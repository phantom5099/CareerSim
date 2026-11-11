//! 简历应用服务：编排存储端口、附件存储与文本提取（plan/01 §3.4、§4.4、§8）。

use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::io::AsyncRead;

use super::content::ResumeContent;
use super::model::{
    DOCX_MIME, MAX_ATTACHMENT_BYTES, NewResume, PDF_MIME, ParseStatus, Resume, ResumeAttachment,
    ResumeError, ResumeId, ResumePatch,
};
use super::parse::{
    AttachmentKind, EXTRACT_TIMEOUT, ParseError, ResumeTextExtractor, draft_from_text,
    truncate_utf8,
};
use super::storage::{AttachmentStore, AttachmentStoreError};
use super::store::{ResumeStore, ResumeStoreError};
use crate::account::model::UserId;

/// 打开附件下载的结果；流由调用方读完（≤10 MiB）。
pub struct OpenedAttachment {
    pub file_name: String,
    pub mime: String,
    pub size: u64,
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

        let mut sha256 = [0u8; 32];
        sha256.copy_from_slice(&Sha256::digest(bytes));

        let storage_key = uuid::Uuid::new_v4().to_string();
        self.attachments
            .put(&storage_key, bytes)
            .await
            .map_err(map_attachment_error)?;

        // 提取在阻塞线程池执行，带超时；纯文本只用于生成草稿，不持久化、不进日志。
        let extractor = self.extractor.clone();
        let bytes_for_task = bytes.to_vec();
        let handle = tokio::task::spawn_blocking(move || extractor.extract(kind, &bytes_for_task));
        let extracted = match tokio::time::timeout(EXTRACT_TIMEOUT, handle).await {
            Ok(Ok(result)) => result,
            Ok(Err(join_err)) => {
                return Err(ResumeError::Backend {
                    message: "text extraction task failed".to_owned(),
                    source: Some(Box::new(join_err)),
                });
            }
            Err(_elapsed) => Err(ParseError::Timeout),
        };

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
                let draft = draft_from_text(&truncated);
                let json = draft.to_json()?;
                (ParseStatus::Ready, Some(json))
            }
        };

        let content = match &content_json {
            Some(json) => json.clone(),
            None => ResumeContent::default().to_json()?,
        };

        let resume = NewResume {
            id: ResumeId::generate(),
            title,
            content,
            attachment: Some(super::model::NewResumeAttachment {
                storage_key: storage_key.clone(),
                file_name,
                mime: kind.mime(),
                size: bytes.len() as u64,
                sha256,
            }),
            parse_status,
        };

        match self.store.insert(owner, resume).await {
            Ok(inserted) => Ok(inserted),
            Err(store_err) => {
                let _ = self.attachments.delete(&storage_key).await;
                Err(map_store_error(store_err))
            }
        }
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
            Some(value) => Some(value.to_json()?),
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
            size: attachment.size,
            stream,
        })
    }
}

/// 扩展名 + magic bytes 双重校验；两者必须一致地指向同一支持格式。
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
        Some(kind) if extension_ok(kind) => Ok(kind),
        _ => Err(ResumeError::UnsupportedAttachment),
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
