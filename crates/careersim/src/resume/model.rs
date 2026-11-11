//! 简历领域模型：归属、类型、解析状态与边界（plan/01 §3.4）。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::content::ResumeContent;

/// 附件 MIME 固定值；仅 PDF/DOCX 两种（plan/01 §5.3）。
pub const PDF_MIME: &str = "application/pdf";
pub const DOCX_MIME: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

/// 每个用户最多持有的有效（未删除）简历份数。
pub const RESUME_LIMIT: usize = 3;

/// 附件大小上限：10 MiB（plan/01 §3.4、§8.1；与 0004 迁移的 CHECK 一致）。
pub const MAX_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;

/// 简历唯一标识；投递按它引用，不复制内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResumeId(Uuid);

impl ResumeId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn into_uuid(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumeKind {
    /// 在线手填简历，无附件。
    Online,
    /// 附件简历：解析草稿同样保存在 `content`，可编辑。
    Attachment,
}

impl ResumeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ResumeKind::Online => "online",
            ResumeKind::Attachment => "attachment",
        }
    }
}

/// 解析失败的固定错误码；对外只暴露该枚举，不携带底层错误（plan/01 §5.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseErrorCode {
    UnsupportedFormat,
    Timeout,
    EmptyText,
    Corrupt,
}

impl ParseErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ParseErrorCode::UnsupportedFormat => "unsupported_format",
            ParseErrorCode::Timeout => "timeout",
            ParseErrorCode::EmptyText => "empty_text",
            ParseErrorCode::Corrupt => "corrupt",
        }
    }

    /// 数据库读取用；未知字符串视为存储损坏，由调用方返回后端错误。
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "unsupported_format" => Some(Self::UnsupportedFormat),
            "timeout" => Some(Self::Timeout),
            "empty_text" => Some(Self::EmptyText),
            "corrupt" => Some(Self::Corrupt),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseStatus {
    /// 在线简历，无解析概念。
    NotApplicable,
    /// 附件已上传待解析；Phase 1 为同步解析，通常不出现。
    Pending,
    Ready,
    Failed {
        code: ParseErrorCode,
    },
}

impl ParseStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParseStatus::NotApplicable => "not_applicable",
            ParseStatus::Pending => "pending",
            ParseStatus::Ready => "ready",
            ParseStatus::Failed { .. } => "failed",
        }
    }
}

/// 附件元数据；`storage_key` 由服务端生成，不来自用户，也不对外返回。
#[derive(Debug, Clone)]
pub struct ResumeAttachment {
    pub storage_key: String,
    /// 原始文件名，仅用于展示与下载头。
    pub file_name: String,
    /// 校验后的固定 MIME（PDF/DOCX 之一）。
    pub mime: String,
    pub size: u64,
    /// 文件内容摘要；不对外返回。
    pub sha256: [u8; 32],
}

/// 待写入的附件元数据；`mime` 为校验后的固定值。
#[derive(Debug, Clone)]
pub struct NewResumeAttachment {
    pub storage_key: String,
    pub file_name: String,
    pub mime: &'static str,
    pub size: u64,
    pub sha256: [u8; 32],
}

/// 简历聚合。
#[derive(Debug, Clone)]
pub struct Resume {
    pub id: ResumeId,
    pub owner: crate::account::model::UserId,
    /// 展示标题，1..=80 字符。
    pub title: String,
    pub kind: ResumeKind,
    /// 默认简历；每用户最多 1 份，由部分唯一索引兜底。
    pub is_default: bool,
    pub content: ResumeContent,
    pub attachment: Option<ResumeAttachment>,
    pub parse_status: ParseStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 插入简历的存储输入；`content` 已完成边界校验并序列化。
#[derive(Debug, Clone)]
pub struct NewResume {
    pub id: ResumeId,
    pub title: String,
    pub content: serde_json::Value,
    pub attachment: Option<NewResumeAttachment>,
    pub parse_status: ParseStatus,
}

/// 编辑输入；PATCH 语义，字段为空表示不修改。
#[derive(Debug, Clone, Default)]
pub struct ResumePatch {
    pub title: Option<String>,
    pub content: Option<serde_json::Value>,
}

/// 标题边界：trim 后 1..=80 字符（plan/01 §5.3 `resumes_title_len`）。
pub fn validate_title(raw: &str) -> Result<String, ResumeError> {
    let title = raw.trim();
    let chars = title.chars().count();
    if !(1..=80).contains(&chars) {
        return Err(ResumeError::InvalidTitle {
            reason: "must be 1..=80 characters",
        });
    }
    Ok(title.to_owned())
}

/// resume 模块错误；HTTP 层映射为统一错误体。
#[derive(Debug, thiserror::Error)]
pub enum ResumeError {
    #[error("invalid resume title: {reason}")]
    InvalidTitle { reason: &'static str },
    #[error("invalid resume content field `{field}`: {reason}")]
    InvalidContent { field: String, reason: &'static str },
    #[error("resume content too large")]
    ContentTooLarge,
    #[error("resume not found")]
    NotFound,
    #[error("resume limit reached")]
    LimitReached,
    #[error("attachment exceeds the 10 MiB size limit")]
    AttachmentTooLarge,
    #[error("attachment is not a valid PDF or DOCX file")]
    UnsupportedAttachment,
    #[error("attachment file name is invalid")]
    InvalidFileName,
    #[error("resume backend error: {message}")]
    Backend {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}
