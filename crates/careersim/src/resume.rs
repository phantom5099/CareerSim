//! 简历上下文：在线简历内容、可选附件、规则化解析回填与默认简历管理（plan/01 §3.4）。
//!
//! 依赖方向：`http -> ResumeService -> ResumeStore/AttachmentStore/ResumeTextExtractor`；
//! PG 与本地文件系统适配器只通过 [`postgres_resume_service`] 对外暴露。

pub mod content;
pub mod model;
pub mod parse;
pub(crate) mod postgres;
pub mod service;
pub mod storage;
pub mod store;
pub mod structurize;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use sqlx::PgPool;

pub use content::ResumeContent;
pub use model::{
    NewResume, NewResumeAttachment, ParseErrorCode, ParseStatus, Resume, ResumeAttachment,
    ResumeError, ResumeId, ResumePatch,
};
pub use parse::{
    AttachmentKind, CompositeTextExtractor, DocxTextExtractor, ParseError, PdfTextExtractor,
    ResumeTextExtractor,
};
pub use service::{OpenedAttachment, ResumeService};
pub use storage::{
    AttachmentStore, AttachmentStoreError, LocalFsAttachmentStore, S3AttachmentStore,
};
pub use store::{ResumeStore, ResumeStoreError};
pub use structurize::{merge_parsed, structurize};

/// 附件格式统一提取器：PDF + DOCX。
pub fn default_text_extractor() -> Arc<dyn ResumeTextExtractor> {
    Arc::new(
        CompositeTextExtractor::new()
            .push(Arc::new(PdfTextExtractor))
            .push(Arc::new(DocxTextExtractor)),
    )
}

/// 构造基于 PostgreSQL + 本地文件系统的简历服务；存储实现只通过这里对外暴露。
pub fn postgres_resume_service(
    pool: PgPool,
    attachment_store: Arc<dyn AttachmentStore>,
) -> ResumeService {
    ResumeService::new(
        Arc::new(postgres::PgResumeStore::new(pool)),
        attachment_store,
        default_text_extractor(),
    )
}
