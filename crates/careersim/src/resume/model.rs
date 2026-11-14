//! 简历领域模型：归属、附件与解析状态边界（plan/01 §3.4）。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::content::ResumeContent;

/// 附件 MIME 固定值；仅 PDF/DOCX 两种（plan/01 §5.3）。
pub const PDF_MIME: &str = "application/pdf";
pub const DOCX_MIME: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

/// 由文件名后缀推导附件 MIME（大小写不敏感）。
///
/// MIME 不单独落库：上传时扩展名已与文件内容双重校验匹配，
/// 仅 PDF/DOCX 两种，从 `attachment_name` 后缀推导即足够。
pub fn mime_from_file_name(file_name: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)?;
    match ext.as_str() {
        "pdf" => Some(PDF_MIME),
        "docx" => Some(DOCX_MIME),
        _ => None,
    }
}

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
    /// 无附件；解析概念不适用。
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
    /// 校验后的固定 MIME（PDF/DOCX 之一），由文件名后缀推导。
    pub mime: String,
}

/// 待写入的附件元数据；MIME 由 `file_name` 后缀推导，不单独落库。
#[derive(Debug, Clone)]
pub struct NewResumeAttachment {
    pub storage_key: String,
    pub file_name: String,
}

/// 简历聚合：结构化在线内容（必有、可编辑）+ 可选附件（原始 PDF/DOCX）。
#[derive(Debug, Clone)]
pub struct Resume {
    pub id: ResumeId,
    pub owner: crate::account::model::UserId,
    /// 展示标题，1..=80 字符。
    pub title: String,
    /// 默认简历；每用户最多 1 份，由部分唯一索引兜底。
    pub is_default: bool,
    pub content: ResumeContent,
    pub attachment: Option<ResumeAttachment>,
    /// 无附件时为 `NotApplicable`；有附件时为解析状态。
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
