//! 账户端口的 PostgreSQL 适配：users 与 login_sessions 两张表。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row, postgres::PgRow};

use super::model::{CredentialUser, NewUser, PasswordHash, ProfilePatch, User, UserId};
use super::session::{LoginSession, NewLoginSession, SessionId, SessionTokenHash};
use super::store::{SessionStore, SessionStoreError, UserStore, UserStoreError};

pub(crate) struct PgUserStore {
    pool: PgPool,
}

impl PgUserStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

pub(crate) struct PgSessionStore {
    pool: PgPool,
}

impl PgSessionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn user_backend(operation: &str, err: sqlx::Error) -> UserStoreError {
    UserStoreError::Backend {
        message: operation.to_owned(),
        source: Some(Box::new(err)),
    }
}

fn session_backend(operation: &str, err: sqlx::Error) -> SessionStoreError {
    SessionStoreError::Backend {
        message: operation.to_owned(),
        source: Some(Box::new(err)),
    }
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(err) if err.is_unique_violation())
}

fn user_from_row(row: &PgRow) -> Result<User, UserStoreError> {
    let graduation_year: Option<i16> = row
        .try_get("graduation_year")
        .map_err(|err| user_backend("read row", err))?;
    // 数据库 CHECK 限定 1980..=2100，越界视为后端数据损坏。
    let graduation_year = match graduation_year {
        None => None,
        Some(v) => Some(u16::try_from(v).map_err(|_| UserStoreError::Backend {
            message: "graduation_year out of u16 range".to_owned(),
            source: None,
        })?),
    };
    Ok(User {
        id: UserId::from_uuid(
            row.try_get("id")
                .map_err(|err| user_backend("read row", err))?,
        ),
        username: row
            .try_get("username")
            .map_err(|err| user_backend("read row", err))?,
        display_name: row
            .try_get("display_name")
            .map_err(|err| user_backend("read row", err))?,
        school: row
            .try_get("school")
            .map_err(|err| user_backend("read row", err))?,
        major: row
            .try_get("major")
            .map_err(|err| user_backend("read row", err))?,
        graduation_year,
        target_roles: row
            .try_get("target_roles")
            .map_err(|err| user_backend("read row", err))?,
        target_cities: row
            .try_get("target_cities")
            .map_err(|err| user_backend("read row", err))?,
        created_at: row
            .try_get("created_at")
            .map_err(|err| user_backend("read row", err))?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|err| user_backend("read row", err))?,
    })
}

fn session_from_row(row: &PgRow) -> Result<LoginSession, SessionStoreError> {
    let map = |err: sqlx::Error| session_backend("read row", err);
    Ok(LoginSession {
        id: SessionId::from_uuid(row.try_get("id").map_err(map)?),
        user_id: UserId::from_uuid(row.try_get("user_id").map_err(map)?),
        created_at: row.try_get("created_at").map_err(map)?,
        last_seen_at: row.try_get("last_seen_at").map_err(map)?,
        idle_expires_at: row.try_get("idle_expires_at").map_err(map)?,
        absolute_expires_at: row.try_get("absolute_expires_at").map_err(map)?,
        revoked_at: row.try_get("revoked_at").map_err(map)?,
    })
}

#[async_trait]
impl UserStore for PgUserStore {
    async fn insert(&self, new: NewUser) -> Result<User, UserStoreError> {
        sqlx::query(
            "INSERT INTO users (id, username, username_key, password_hash, display_name, \
             created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $6) \
             RETURNING id, username, display_name, school, major, graduation_year, \
             target_roles, target_cities, created_at, updated_at",
        )
        .bind(new.id.into_uuid())
        .bind(&new.username)
        .bind(new.username_key.as_str())
        .bind(new.password_hash.as_str())
        .bind(&new.display_name)
        .bind(new.registered_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| {
            if is_unique_violation(&err) {
                UserStoreError::Conflict
            } else {
                user_backend("insert user", err)
            }
        })
        .and_then(|row| user_from_row(&row))
    }

    async fn find_credential(
        &self,
        username_key: &str,
    ) -> Result<Option<CredentialUser>, UserStoreError> {
        let row = sqlx::query("SELECT id, password_hash FROM users WHERE username_key = $1")
            .bind(username_key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| user_backend("find credential", err))?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(CredentialUser {
            id: UserId::from_uuid(
                row.try_get("id")
                    .map_err(|err| user_backend("find credential", err))?,
            ),
            password_hash: PasswordHash::new(
                row.try_get("password_hash")
                    .map_err(|err| user_backend("find credential", err))?,
            ),
        }))
    }

    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, UserStoreError> {
        sqlx::query(
            "SELECT id, username, display_name, school, major, graduation_year, \
             target_roles, target_cities, created_at, updated_at \
             FROM users WHERE id = $1",
        )
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| user_backend("find user", err))?
        .map(|row| user_from_row(&row))
        .transpose()
    }

    async fn update_profile(
        &self,
        id: UserId,
        patch: &ProfilePatch,
        now: DateTime<Utc>,
    ) -> Result<User, UserStoreError> {
        // 外层 Option = 是否修改；内层 Option = 置空或取值。
        // 用固定 SQL 的 CASE WHEN 表达 PATCH 语义，避免拼接动态 SQL。
        sqlx::query(
            "UPDATE users SET \
             display_name = CASE WHEN $3 THEN $4::text ELSE display_name END, \
             school = CASE WHEN $5 THEN $6::text ELSE school END, \
             major = CASE WHEN $7 THEN $8::text ELSE major END, \
             graduation_year = CASE WHEN $9 THEN $10::smallint ELSE graduation_year END, \
             target_roles = CASE WHEN $11 THEN $12::text[] ELSE target_roles END, \
             target_cities = CASE WHEN $13 THEN $14::text[] ELSE target_cities END, \
             updated_at = $2 \
             WHERE id = $1 \
             RETURNING id, username, display_name, school, major, graduation_year, \
             target_roles, target_cities, created_at, updated_at",
        )
        .bind(id.into_uuid())
        .bind(now)
        .bind(patch.display_name.is_some())
        .bind(patch.display_name.clone())
        .bind(patch.school.is_some())
        .bind(patch.school.clone().flatten())
        .bind(patch.major.is_some())
        .bind(patch.major.clone().flatten())
        .bind(patch.graduation_year.is_some())
        .bind(patch.graduation_year.flatten().map(|v| v as i16))
        .bind(patch.target_roles.is_some())
        .bind(patch.target_roles.clone())
        .bind(patch.target_cities.is_some())
        .bind(patch.target_cities.clone())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| user_backend("update profile", err))?
        .map(|row| user_from_row(&row))
        .transpose()
        .and_then(|user| user.ok_or(UserStoreError::NotFound))
    }

    async fn update_password(
        &self,
        id: UserId,
        hash: &PasswordHash,
        now: DateTime<Utc>,
    ) -> Result<(), UserStoreError> {
        let result =
            sqlx::query("UPDATE users SET password_hash = $2, updated_at = $3 WHERE id = $1")
                .bind(id.into_uuid())
                .bind(hash.as_str())
                .bind(now)
                .execute(&self.pool)
                .await
                .map_err(|err| user_backend("update password", err))?;
        if result.rows_affected() == 0 {
            return Err(UserStoreError::NotFound);
        }
        Ok(())
    }
}

#[async_trait]
impl SessionStore for PgSessionStore {
    async fn insert(&self, session: NewLoginSession) -> Result<(), SessionStoreError> {
        sqlx::query(
            "INSERT INTO login_sessions \
             (id, user_id, token_hash, created_at, last_seen_at, idle_expires_at, absolute_expires_at) \
             VALUES ($1, $2, $3, $4, $4, $5, $6)",
        )
        .bind(session.id.into_uuid())
        .bind(session.user_id.into_uuid())
        .bind(session.token_hash.as_bytes())
        .bind(session.created_at)
        .bind(session.idle_expires_at)
        .bind(session.absolute_expires_at)
        .execute(&self.pool)
        .await
        .map_err(|err| {
            if is_unique_violation(&err) {
                // 令牌哈希碰撞概率可忽略；仍按后端错误上报以便排查。
                SessionStoreError::Backend {
                    message: "insert session: token hash conflict".to_owned(),
                    source: Some(Box::new(err)),
                }
            } else {
                session_backend("insert session", err)
            }
        })?;
        Ok(())
    }

    async fn find_by_token_hash(
        &self,
        hash: &SessionTokenHash,
    ) -> Result<Option<LoginSession>, SessionStoreError> {
        sqlx::query(
            "SELECT id, user_id, created_at, last_seen_at, idle_expires_at, absolute_expires_at, \
             revoked_at FROM login_sessions WHERE token_hash = $1",
        )
        .bind(hash.as_bytes())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| session_backend("find session", err))?
        .map(|row| session_from_row(&row))
        .transpose()
    }

    async fn touch(
        &self,
        id: SessionId,
        last_seen_at: DateTime<Utc>,
        idle_expires_at: DateTime<Utc>,
    ) -> Result<(), SessionStoreError> {
        let result = sqlx::query(
            "UPDATE login_sessions SET last_seen_at = $2, idle_expires_at = $3 \
             WHERE id = $1 AND revoked_at IS NULL",
        )
        .bind(id.into_uuid())
        .bind(last_seen_at)
        .bind(idle_expires_at)
        .execute(&self.pool)
        .await
        .map_err(|err| session_backend("touch session", err))?;
        if result.rows_affected() == 0 {
            return Err(SessionStoreError::NotFound);
        }
        Ok(())
    }

    async fn revoke(&self, id: SessionId, now: DateTime<Utc>) -> Result<(), SessionStoreError> {
        // 幂等：不存在或已撤销同样成功。
        sqlx::query(
            "UPDATE login_sessions SET revoked_at = $2 WHERE id = $1 AND revoked_at IS NULL",
        )
        .bind(id.into_uuid())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|err| session_backend("revoke session", err))?;
        Ok(())
    }

    async fn revoke_others(
        &self,
        user: UserId,
        keep: SessionId,
        now: DateTime<Utc>,
    ) -> Result<(), SessionStoreError> {
        sqlx::query(
            "UPDATE login_sessions SET revoked_at = $3 \
             WHERE user_id = $1 AND id <> $2 AND revoked_at IS NULL",
        )
        .bind(user.into_uuid())
        .bind(keep.into_uuid())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|err| session_backend("revoke other sessions", err))?;
        Ok(())
    }

    async fn rotate(
        &self,
        old: SessionId,
        new: NewLoginSession,
        now: DateTime<Utc>,
    ) -> Result<(), SessionStoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| session_backend("rotate begin", err))?;

        let revoked = sqlx::query(
            "UPDATE login_sessions SET revoked_at = $2 WHERE id = $1 AND revoked_at IS NULL",
        )
        .bind(old.into_uuid())
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|err| session_backend("rotate revoke old", err))?;
        if revoked.rows_affected() == 0 {
            return Err(SessionStoreError::NotFound);
        }

        sqlx::query(
            "INSERT INTO login_sessions \
             (id, user_id, token_hash, created_at, last_seen_at, idle_expires_at, absolute_expires_at) \
             VALUES ($1, $2, $3, $4, $4, $5, $6)",
        )
        .bind(new.id.into_uuid())
        .bind(new.user_id.into_uuid())
        .bind(new.token_hash.as_bytes())
        .bind(new.created_at)
        .bind(new.idle_expires_at)
        .bind(new.absolute_expires_at)
        .execute(&mut *tx)
        .await
        .map_err(|err| session_backend("rotate insert new", err))?;

        tx.commit()
            .await
            .map_err(|err| session_backend("rotate commit", err))
    }
}
