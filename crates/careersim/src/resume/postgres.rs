//! 简历端口的 PostgreSQL 适配：resumes 表，行锁保证份数上限与默认唯一。

use async_trait::async_trait;
use sqlx::{PgPool, Row, postgres::PgRow};

use super::model::{
    NewResume, NewResumeAttachment, ParseErrorCode, ParseStatus, RESUME_LIMIT, Resume,
    ResumeAttachment, ResumeId, ResumePatch,
};
use super::store::{ResumeStore, ResumeStoreError};
use crate::account::model::UserId;

/// SELECT 列清单；所有 SQL 均为编译期字面量（sqlx 0.9 `SqlSafeStr` 要求），
/// 查询为固定模板 + 绑定参数，无注入面。
macro_rules! resume_select {
    ($tail:literal) => {
        concat!(
            "SELECT id, user_id, title, is_default, content, attachment_key, ",
            "attachment_name, ",
            "parse_status, parse_error_code, created_at, updated_at ",
            $tail
        )
    };
}

/// INSERT/UPDATE ... RETURNING 列清单；与 SELECT 一致。
macro_rules! resume_update_sql {
    ($head:literal) => {
        concat!(
            $head,
            " RETURNING id, user_id, title, is_default, content, attachment_key, ",
            "attachment_name, ",
            "parse_status, parse_error_code, created_at, updated_at"
        )
    };
}

pub(crate) struct PgResumeStore {
    pool: PgPool,
}

impl PgResumeStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn resume_backend(operation: &str, err: sqlx::Error) -> ResumeStoreError {
    ResumeStoreError::Backend {
        message: operation.to_owned(),
        source: Some(Box::new(err)),
    }
}

fn corrupt(message: impl std::fmt::Display) -> ResumeStoreError {
    ResumeStoreError::Backend {
        message: format!("stored resume row is inconsistent: {message}"),
        source: None,
    }
}

fn resume_from_row(row: &PgRow) -> Result<Resume, ResumeStoreError> {
    let map = |err: sqlx::Error| resume_backend("read row", err);
    let id: uuid::Uuid = row.try_get("id").map_err(map)?;
    let owner: uuid::Uuid = row.try_get("user_id").map_err(map)?;
    let title: String = row.try_get("title").map_err(map)?;
    let is_default: bool = row.try_get("is_default").map_err(map)?;

    let content_value: serde_json::Value = row.try_get("content").map_err(map)?;
    let content: super::content::ResumeContent = serde_json::from_value(content_value)
        .map_err(|err| corrupt(format!("stored content failed to deserialize: {err}")))?;

    let attachment_key: Option<String> = row.try_get("attachment_key").map_err(map)?;
    let attachment_name: Option<String> = row.try_get("attachment_name").map_err(map)?;

    // 附件字段组成组出现：全 NULL ⇔ 无附件；全非空 ⇔ 有附件（与 DB CHECK 一致）。
    // MIME 不落库，由文件名后缀推导（上传时已与内容双重校验）。
    let attachment = match (attachment_key, attachment_name) {
        (None, None) => None,
        (Some(storage_key), Some(file_name)) => {
            let mime = super::model::mime_from_file_name(&file_name)
                .ok_or_else(|| corrupt("unknown attachment file extension"))?;
            Some(ResumeAttachment {
                storage_key,
                file_name,
                mime: mime.to_owned(),
            })
        }
        _ => return Err(corrupt("attachment columns are partially populated")),
    };

    let status_raw: String = row.try_get("parse_status").map_err(map)?;
    let error_code_raw: Option<String> = row.try_get("parse_error_code").map_err(map)?;
    let parse_status = match status_raw.as_str() {
        "not_applicable" => {
            if error_code_raw.is_some() {
                return Err(corrupt("not_applicable resume carries an error code"));
            }
            ParseStatus::NotApplicable
        }
        "pending" => ParseStatus::Pending,
        "ready" => ParseStatus::Ready,
        "failed" => {
            let Some(raw) = error_code_raw.as_deref() else {
                return Err(corrupt("failed resume misses an error code"));
            };
            let Some(code) = ParseErrorCode::from_db(raw) else {
                return Err(corrupt("unknown parse error code"));
            };
            ParseStatus::Failed { code }
        }
        _ => return Err(corrupt("unknown parse status")),
    };

    Ok(Resume {
        id: ResumeId::from_uuid(id),
        owner: UserId::from_uuid(owner),
        title,
        is_default,
        content,
        attachment,
        parse_status,
        created_at: row.try_get("created_at").map_err(map)?,
        updated_at: row.try_get("updated_at").map_err(map)?,
    })
}

#[async_trait]
impl ResumeStore for PgResumeStore {
    async fn list(&self, owner: UserId) -> Result<Vec<Resume>, ResumeStoreError> {
        let rows = sqlx::query(resume_select!(
            "FROM resumes WHERE user_id = $1 AND deleted_at IS NULL \
             ORDER BY is_default DESC, updated_at DESC, id DESC"
        ))
        .bind(owner.into_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|err| resume_backend("list resumes", err))?;
        rows.iter().map(resume_from_row).collect()
    }

    async fn get(&self, owner: UserId, id: ResumeId) -> Result<Resume, ResumeStoreError> {
        sqlx::query(resume_select!(
            "FROM resumes WHERE user_id = $1 AND id = $2 AND deleted_at IS NULL"
        ))
        .bind(owner.into_uuid())
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| resume_backend("get resume", err))?
        .map(|row| resume_from_row(&row))
        .transpose()?
        .ok_or(ResumeStoreError::NotFound)
    }

    async fn insert(&self, owner: UserId, resume: NewResume) -> Result<Resume, ResumeStoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| resume_backend("insert resume begin", err))?;

        // 行锁串行化同一用户的并发插入，保证“首份默认 + 上限 3”判定原子。
        let locked: Option<uuid::Uuid> =
            sqlx::query_scalar("SELECT id FROM users WHERE id = $1 FOR UPDATE")
                .bind(owner.into_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(|err| resume_backend("insert resume lock user", err))?;
        if locked.is_none() {
            return Err(resume_backend(
                "insert resume: owner user does not exist",
                sqlx::Error::RowNotFound,
            ));
        }

        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM resumes WHERE user_id = $1 AND deleted_at IS NULL",
        )
        .bind(owner.into_uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| resume_backend("insert resume count", err))?;
        if count >= RESUME_LIMIT as i64 {
            return Err(ResumeStoreError::LimitReached);
        }
        let is_default = count == 0;

        let (attachment_key, attachment_name) = match &resume.attachment {
            None => (None::<String>, None),
            Some(attachment) => (
                Some(attachment.storage_key.clone()),
                Some(attachment.file_name.clone()),
            ),
        };

        let (parse_status, parse_error_code) = match &resume.parse_status {
            ParseStatus::NotApplicable => ("not_applicable", None),
            ParseStatus::Pending => ("pending", None),
            ParseStatus::Ready => ("ready", None),
            ParseStatus::Failed { code } => ("failed", Some(code.as_str())),
        };

        let row = sqlx::query(resume_update_sql!(
            "INSERT INTO resumes (id, user_id, title, is_default, content, \
             attachment_key, attachment_name, \
             parse_status, parse_error_code, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)"
        ))
        .bind(resume.id.into_uuid())
        .bind(owner.into_uuid())
        .bind(&resume.title)
        .bind(is_default)
        .bind(&resume.content)
        .bind(attachment_key)
        .bind(attachment_name)
        .bind(parse_status)
        .bind(parse_error_code)
        .bind(chrono::Utc::now())
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| {
            if matches!(err, sqlx::Error::Database(ref db_err) if db_err.is_unique_violation()) {
                ResumeStoreError::Backend {
                    message: "insert resume: unique constraint violated".to_owned(),
                    source: Some(Box::new(err)),
                }
            } else {
                resume_backend("insert resume", err)
            }
        })?;

        let inserted = resume_from_row(&row)?;
        tx.commit()
            .await
            .map_err(|err| resume_backend("insert resume commit", err))?;
        Ok(inserted)
    }

    async fn update(
        &self,
        owner: UserId,
        id: ResumeId,
        patch: ResumePatch,
    ) -> Result<Resume, ResumeStoreError> {
        // COALESCE 表达 PATCH 语义；未传字段保持原值。
        let row = sqlx::query(resume_update_sql!(
            "UPDATE resumes SET title = COALESCE($3, title), \
             content = COALESCE($4, content), updated_at = $5 \
             WHERE user_id = $1 AND id = $2 AND deleted_at IS NULL"
        ))
        .bind(owner.into_uuid())
        .bind(id.into_uuid())
        .bind(patch.title)
        .bind(patch.content)
        .bind(chrono::Utc::now())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| resume_backend("update resume", err))?;
        match row {
            Some(row) => Ok(resume_from_row(&row)?),
            None => Err(ResumeStoreError::NotFound),
        }
    }

    async fn set_default(&self, owner: UserId, id: ResumeId) -> Result<Resume, ResumeStoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| resume_backend("set default begin", err))?;

        // 先清旧默认再设新默认；切换默认不更新 updated_at（非内容变更）。
        sqlx::query(
            "UPDATE resumes SET is_default = false \
             WHERE user_id = $1 AND is_default AND deleted_at IS NULL",
        )
        .bind(owner.into_uuid())
        .execute(&mut *tx)
        .await
        .map_err(|err| resume_backend("set default clear old", err))?;

        let row = sqlx::query(resume_update_sql!(
            "UPDATE resumes SET is_default = true \
             WHERE user_id = $1 AND id = $2 AND deleted_at IS NULL"
        ))
        .bind(owner.into_uuid())
        .bind(id.into_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| resume_backend("set default set new", err))?;
        // 出错即回滚（旧默认恢复），语义安全。
        let Some(row) = row else {
            return Err(ResumeStoreError::NotFound);
        };
        let updated = resume_from_row(&row)?;
        tx.commit()
            .await
            .map_err(|err| resume_backend("set default commit", err))?;
        Ok(updated)
    }

    async fn soft_delete(&self, owner: UserId, id: ResumeId) -> Result<(), ResumeStoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| resume_backend("delete resume begin", err))?;

        let was_default: Option<bool> = sqlx::query_scalar(
            "UPDATE resumes SET deleted_at = $3 \
             WHERE user_id = $1 AND id = $2 AND deleted_at IS NULL \
             RETURNING is_default",
        )
        .bind(owner.into_uuid())
        .bind(id.into_uuid())
        .bind(chrono::Utc::now())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| resume_backend("delete resume", err))?;
        let Some(was_default) = was_default else {
            return Err(ResumeStoreError::NotFound);
        };

        if was_default {
            // 事务内提升最新一份（updated_at DESC, id DESC）；没有剩余简历时为空操作。
            sqlx::query(
                "UPDATE resumes SET is_default = true \
                 WHERE id = (SELECT id FROM resumes \
                     WHERE user_id = $1 AND deleted_at IS NULL \
                     ORDER BY updated_at DESC, id DESC LIMIT 1)",
            )
            .bind(owner.into_uuid())
            .execute(&mut *tx)
            .await
            .map_err(|err| resume_backend("delete resume promote", err))?;
        }

        tx.commit()
            .await
            .map_err(|err| resume_backend("delete resume commit", err))
    }

    async fn attach(
        &self,
        owner: UserId,
        id: ResumeId,
        attachment: NewResumeAttachment,
        parse_status: ParseStatus,
        content: serde_json::Value,
    ) -> Result<Resume, ResumeStoreError> {
        let (status, error_code) = match &parse_status {
            ParseStatus::NotApplicable => ("not_applicable", None),
            ParseStatus::Pending => ("pending", None),
            ParseStatus::Ready => ("ready", None),
            ParseStatus::Failed { code } => ("failed", Some(code.as_str())),
        };
        let row = sqlx::query(resume_update_sql!(
            "UPDATE resumes SET attachment_key = $3, attachment_name = $4, \
             parse_status = $5, parse_error_code = $6, content = $7, updated_at = $8 \
             WHERE user_id = $1 AND id = $2 AND deleted_at IS NULL"
        ))
        .bind(owner.into_uuid())
        .bind(id.into_uuid())
        .bind(&attachment.storage_key)
        .bind(&attachment.file_name)
        .bind(status)
        .bind(error_code)
        .bind(&content)
        .bind(chrono::Utc::now())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| resume_backend("attach resume", err))?;
        match row {
            Some(row) => Ok(resume_from_row(&row)?),
            None => Err(ResumeStoreError::NotFound),
        }
    }

    async fn detach(&self, owner: UserId, id: ResumeId) -> Result<Resume, ResumeStoreError> {
        let row = sqlx::query(resume_update_sql!(
            "UPDATE resumes SET attachment_key = NULL, attachment_name = NULL, \
             parse_status = 'not_applicable', parse_error_code = NULL, updated_at = $3 \
             WHERE user_id = $1 AND id = $2 AND deleted_at IS NULL"
        ))
        .bind(owner.into_uuid())
        .bind(id.into_uuid())
        .bind(chrono::Utc::now())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| resume_backend("detach resume", err))?;
        match row {
            Some(row) => Ok(resume_from_row(&row)?),
            None => Err(ResumeStoreError::NotFound),
        }
    }
}
