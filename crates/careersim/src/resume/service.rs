//! 简历应用服务：编排存储端口、附件存储与文本提取（plan/01 §3.4、§4.4、§8）。

use std::sync::Arc;

use tokio::io::AsyncRead;

use super::content::ResumeContent;
use super::model::{
    DOCX_MIME, MAX_ATTACHMENT_BYTES, NewResume, PDF_MIME, ParseStatus, Resume, ResumeAttachment,
    ResumeError, ResumeId, ResumePatch,
};
use super::parse::{
    AttachmentKind, DocxTextExtractor, EXTRACT_TIMEOUT, ParseError, ResumeTextExtractor,
    truncate_utf8,
};
use super::storage::{AttachmentStore, AttachmentStoreError};
use super::store::{ResumeStore, ResumeStoreError};
use super::structurize::{merge_parsed, structurize};
use crate::account::model::UserId;

/// 打开附件下载的结果；流由调用方读完（≤10 MiB）。
pub struct OpenedAttachment {
    pub file_name: String,
    pub mime: String,
    pub stream: Box<dyn AsyncRead + Send + Unpin>,
}

pub struct ResumeService {
    store: Arc<dyn ResumeStore>,
    attachments: Arc<dyn AttachmentStore>,
    extractor: Arc<dyn ResumeTextExtractor>,
}

impl ResumeService {
    pub fn new(
        store: Arc<dyn ResumeStore>,
        attachments: Arc<dyn AttachmentStore>,
        extractor: Arc<dyn ResumeTextExtractor>,
    ) -> Self {
        Self {
            store,
            attachments,
            extractor,
        }
    }

    pub async fn list(&self, owner: UserId) -> Result<Vec<Resume>, ResumeError> {
        self.store.list(owner).await.map_err(map_store_error)
    }

    pub async fn get(&self, owner: UserId, id: ResumeId) -> Result<Resume, ResumeError> {
        self.store.get(owner, id).await.map_err(map_store_error)
    }

    /// 创建在线手填简历。
    pub async fn create_online(
        &self,
        owner: UserId,
        title: &str,
        content: ResumeContent,
    ) -> Result<Resume, ResumeError> {
        let title = super::model::validate_title(title)?;
        // 字段级边界与 DB 字节兜底分属两层；入库前必须先过字段校验。
        content.validate()?;
        let json = content.to_json()?;
        let resume = NewResume {
            id: ResumeId::generate(),
            title,
            content: json,
            attachment: None,
            parse_status: ParseStatus::NotApplicable,
        };
        self.store
            .insert(owner, resume)
            .await
            .map_err(map_store_error)
    }

    /// 上传附件简历：双重格式校验 → 落盘 → 提取文本 → 生成草稿。
    ///
    /// 解析失败时仍保留附件并落库 `failed` 状态与固定错误码；数据库写入失败时
    /// 尽力删除已落盘文件，避免孤儿文件。
    pub async fn upload_attachment(
        &self,
        owner: UserId,
        title: &str,
        raw_file_name: &str,
        bytes: &[u8],
    ) -> Result<Resume, ResumeError> {
        let title = super::model::validate_title(title)?;
        if bytes.is_empty() {
            return Err(ResumeError::UnsupportedAttachment);
        }
        if bytes.len() > MAX_ATTACHMENT_BYTES {
            return Err(ResumeError::AttachmentTooLarge);
        }
        let kind = detect_attachment_kind(raw_file_name, bytes)?;
        let file_name = sanitize_file_name(raw_file_name)?;

        let storage_key = uuid::Uuid::new_v4().to_string();
        self.attachments
            .put(&storage_key, bytes)
            .await
            .map_err(map_attachment_error)?;

        // 文件已落盘：此后任何失败都必须清理附件并记录日志，避免孤儿文件
        // （plan/01 §8.1 第 11 步）。
        let outcome = self
            .parse_and_insert(owner, title, file_name, kind, &storage_key, bytes.to_vec())
            .await;
        if let Err(err) = &outcome {
            tracing::error!(
                storage_key = %storage_key,
                error = %err,
                "attachment upload failed after writing file; removing stored attachment"
            );
            let _ = self.attachments.delete(&storage_key).await;
        }
        outcome
    }

    /// 上传附件到**已有**简历：格式校验 → 落盘 → 提取 → 结构化 → 回填合并。
    ///
    /// 只填空字段、列表追加去重，用户已填内容不被覆盖；替换旧附件时旧文件
    /// 在数据库更新成功后清理。解析失败仍保留附件并落库 `failed` 状态。
    pub async fn attach_to_resume(
        &self,
        owner: UserId,
        id: ResumeId,
        raw_file_name: &str,
        bytes: &[u8],
    ) -> Result<Resume, ResumeError> {
        let (file_name, kind, storage_key) =
            self.validate_and_store(raw_file_name, bytes).await?;

        let outcome = self
            .parse_and_attach(owner, id, file_name, kind, &storage_key, bytes.to_vec())
            .await;
        if let Err(err) = &outcome {
            tracing::error!(
                storage_key = %storage_key,
                error = %err,
                "attachment attach failed after writing file; removing stored attachment"
            );
            let _ = self.attachments.delete(&storage_key).await;
        }
        outcome
    }

    /// 移除简历附件：清空附件列并重置解析状态；内容保持不变。
    pub async fn remove_attachment(
        &self,
        owner: UserId,
        id: ResumeId,
    ) -> Result<Resume, ResumeError> {
        let resume = self.store.get(owner, id).await.map_err(map_store_error)?;
        let old_key = resume.attachment.as_ref().map(|att| att.storage_key.clone());
        let updated = self
            .store
            .detach(owner, id)
            .await
            .map_err(map_store_error)?;
        if let Some(key) = old_key {
            let _ = self.attachments.delete(&key).await;
        }
        Ok(updated)
    }

    /// 校验并落盘附件，返回 (清洗后文件名, 格式, 存储键)。
    async fn validate_and_store(
        &self,
        raw_file_name: &str,
        bytes: &[u8],
    ) -> Result<(String, AttachmentKind, String), ResumeError> {
        if bytes.is_empty() {
            return Err(ResumeError::UnsupportedAttachment);
        }
        if bytes.len() > MAX_ATTACHMENT_BYTES {
            return Err(ResumeError::AttachmentTooLarge);
        }
        let kind = detect_attachment_kind(raw_file_name, bytes)?;
        let file_name = sanitize_file_name(raw_file_name)?;

        let storage_key = uuid::Uuid::new_v4().to_string();
        self.attachments
            .put(&storage_key, bytes)
            .await
            .map_err(map_attachment_error)?;
        Ok((file_name, kind, storage_key))
    }

    /// 落盘之后的解析与回填；失败由调用方清理附件。
    async fn parse_and_attach(
        &self,
        owner: UserId,
        id: ResumeId,
        file_name: String,
        kind: AttachmentKind,
        storage_key: &str,
        bytes: Vec<u8>,
    ) -> Result<Resume, ResumeError> {
        let extracted = self.extract_with_timeout(storage_key, kind, bytes).await?;

        let (parse_status, parsed) = match extracted {
            Err(parse_err) => (
                ParseStatus::Failed {
                    code: parse_err.code(),
                },
                ResumeContent::default(),
            ),
            Ok(text) if text.trim().is_empty() => (
                ParseStatus::Failed {
                    code: super::model::ParseErrorCode::EmptyText,
                },
                ResumeContent::default(),
            ),
            Ok(text) => {
                let truncated = truncate_utf8(text.trim(), super::parse::MAX_EXTRACTED_TEXT_BYTES);
                (ParseStatus::Ready, structurize(&truncated))
            }
        };

        // 回填合并：以现有内容为基底，只补空字段、列表追加去重。
        let current = self.store.get(owner, id).await.map_err(map_store_error)?;
        let merged = merge_parsed(&current.content, &parsed);
        merged.validate()?;
        let content_json = merged.to_json()?;

        let updated = self
            .store
            .attach(
                owner,
                id,
                super::model::NewResumeAttachment {
                    storage_key: storage_key.to_owned(),
                    file_name: file_name.clone(),
                },
                parse_status,
                content_json,
            )
            .await
            .map_err(map_store_error)?;

        // 数据库已指向新附件：旧附件文件此时才清理。
        if let Some(old) = &current.attachment {
            if old.storage_key != storage_key {
                let _ = self.attachments.delete(&old.storage_key).await;
            }
        }
        Ok(updated)
    }

    /// 提取文本：阻塞线程池执行，带超时；失败日志只携带解析器消息。
    async fn extract_with_timeout(
        &self,
        storage_key: &str,
        kind: AttachmentKind,
        bytes: Vec<u8>,
    ) -> Result<Result<String, ParseError>, ResumeError> {
        let extractor = self.extractor.clone();
        let handle = tokio::task::spawn_blocking(move || extractor.extract(kind, &bytes));
        match tokio::time::timeout(EXTRACT_TIMEOUT, handle).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(join_err)) => {
                // 解析任务 panic / 提取器内部错误：日志在此处记录，
                // 整体按后端错误失败；已落盘附件由调用方清理。
                tracing::error!(
                    storage_key = %storage_key,
                    error = %join_err,
                    "text extraction task failed"
                );
                Err(ResumeError::Backend {
                    message: "text extraction task failed".to_owned(),
                    source: Some(Box::new(join_err)),
                })
            }
            Err(_elapsed) => Ok(Err(ParseError::Timeout)),
        }
    }

    /// 落盘之后的解析与入库；失败由调用方负责清理附件。
    async fn parse_and_insert(
        &self,
        owner: UserId,
        title: String,
        file_name: String,
        kind: AttachmentKind,
        storage_key: &str,
        bytes: Vec<u8>,
    ) -> Result<Resume, ResumeError> {
        // 提取在阻塞线程池执行，带超时；纯文本只用于生成草稿，不持久化、不进日志。
        let extracted = self.extract_with_timeout(storage_key, kind, bytes).await?;

        let (parse_status, content_json) = match extracted {
            Err(parse_err) => (
                ParseStatus::Failed {
                    code: parse_err.code(),
                },
                None,
            ),
            Ok(text) if text.trim().is_empty() => (
                ParseStatus::Failed {
                    code: super::model::ParseErrorCode::EmptyText,
                },
                None,
            ),
            Ok(text) => {
                let truncated = truncate_utf8(text.trim(), super::parse::MAX_EXTRACTED_TEXT_BYTES);
                // 规则化结构化：分段 + 正则 + 词典，识别不出的字段留空。
                let draft = structurize(&truncated);
                let json = draft.to_json()?;
                (ParseStatus::Ready, Some(json))
            }
        };

        let content = match &content_json {
            Some(json) => json.clone(),
            // 解析失败也必须落库：默认草稿序列化失败属于后端错误，
            // 但仍先尝试入库以保留 `failed` 状态与附件引用。
            None => ResumeContent::default().to_json()?,
        };

        let resume = NewResume {
            id: ResumeId::generate(),
            title,
            content,
            attachment: Some(super::model::NewResumeAttachment {
                storage_key: storage_key.to_owned(),
                file_name,
            }),
            parse_status,
        };

        self.store
            .insert(owner, resume)
            .await
            .map_err(map_store_error)
    }

    /// 编辑标题/内容（PATCH 语义）。
    pub async fn update(
        &self,
        owner: UserId,
        id: ResumeId,
        title: Option<&str>,
        content: Option<ResumeContent>,
    ) -> Result<Resume, ResumeError> {
        let title = match title {
            Some(raw) => Some(super::model::validate_title(raw)?),
            None => None,
        };
        let content_json = match content {
            Some(value) => {
                value.validate()?;
                Some(value.to_json()?)
            }
            None => None,
        };
        let patch = ResumePatch {
            title,
            content: content_json,
        };
        if patch.title.is_none() && patch.content.is_none() {
            // 空补丁等价于读取当前状态；直接返回，避免无谓的 updated_at 跳动。
            return self.get(owner, id).await;
        }
        self.store
            .update(owner, id, patch)
            .await
            .map_err(map_store_error)
    }

    pub async fn set_default(&self, owner: UserId, id: ResumeId) -> Result<Resume, ResumeError> {
        self.store
            .set_default(owner, id)
            .await
            .map_err(map_store_error)
    }

    /// 软删除简历；删除默认简历时由存储层在事务内提升最新一份。附件文件尽力清理。
    pub async fn delete(&self, owner: UserId, id: ResumeId) -> Result<(), ResumeError> {
        let resume = self.store.get(owner, id).await.map_err(map_store_error)?;
        self.store
            .soft_delete(owner, id)
            .await
            .map_err(map_store_error)?;
        if let Some(attachment) = &resume.attachment {
            let _ = self.attachments.delete(&attachment.storage_key).await;
        }
        Ok(())
    }

    /// 打开附件下载流；不向调用方暴露存储键。
    pub async fn open_attachment(
        &self,
        owner: UserId,
        id: ResumeId,
    ) -> Result<OpenedAttachment, ResumeError> {
        let resume = self.store.get(owner, id).await.map_err(map_store_error)?;
        let attachment: &ResumeAttachment =
            resume.attachment.as_ref().ok_or(ResumeError::NotFound)?;
        let stream = self
            .attachments
            .open(&attachment.storage_key)
            .await
            .map_err(map_attachment_error)?;
        Ok(OpenedAttachment {
            file_name: attachment.file_name.clone(),
            mime: attachment.mime.clone(),
            stream,
        })
    }
}

/// 扩展名 + 内容双重校验。PDF 走 magic bytes；DOCX 本质是 zip 容器，
/// 非 Word 直出的 OOXML 常被 infer 仅认作 `application/zip`，此时用
/// 真实 DOCX 解析器做最终裁决——能解析即接受。
pub fn detect_attachment_kind(
    file_name: &str,
    bytes: &[u8],
) -> Result<AttachmentKind, ResumeError> {
    let extension_ok = |expected: AttachmentKind| -> bool {
        let ext = std::path::Path::new(file_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase);
        ext.as_deref() == Some(expected.extension())
    };

    let detected = infer::get(bytes).and_then(|kind| match kind.mime_type() {
        PDF_MIME => Some(AttachmentKind::Pdf),
        DOCX_MIME => Some(AttachmentKind::Docx),
        _ => None,
    });

    match detected {
        Some(kind) => {
            if extension_ok(kind) {
                Ok(kind)
            } else {
                Err(ResumeError::UnsupportedAttachment)
            }
        }
        None => {
            if extension_ok(AttachmentKind::Docx)
                && DocxTextExtractor::default()
                    .extract(AttachmentKind::Docx, bytes)
                    .is_ok()
            {
                Ok(AttachmentKind::Docx)
            } else {
                Err(ResumeError::UnsupportedAttachment)
            }
        }
    }
}

/// 清洗原始文件名：trim、拒绝空值/控制字符/路径分隔符/`..`/超长（≤255 字节）。
pub fn sanitize_file_name(raw: &str) -> Result<String, ResumeError> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > 255
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.chars().any(char::is_control)
    {
        return Err(ResumeError::InvalidFileName);
    }
    Ok(trimmed.to_owned())
}

fn map_store_error(err: ResumeStoreError) -> ResumeError {
    match err {
        ResumeStoreError::NotFound => ResumeError::NotFound,
        ResumeStoreError::LimitReached => ResumeError::LimitReached,
        ResumeStoreError::Backend { message, source } => ResumeError::Backend { message, source },
    }
}

fn map_attachment_error(err: AttachmentStoreError) -> ResumeError {
    match err {
        AttachmentStoreError::NotFound => ResumeError::NotFound,
        AttachmentStoreError::InvalidKey => ResumeError::Backend {
            message: "attachment storage rejected the key".to_owned(),
            source: None,
        },
        AttachmentStoreError::Backend { message, source } => {
            ResumeError::Backend { message, source }
        }
    }
}

impl From<AttachmentStoreError> for ResumeError {
    fn from(err: AttachmentStoreError) -> Self {
        map_attachment_error(err)
    }
}
