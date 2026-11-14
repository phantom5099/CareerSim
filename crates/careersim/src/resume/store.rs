//! Resume persistence port.

use async_trait::async_trait;
use thiserror::Error;

use super::model::{NewResume, NewResumeAttachment, ParseStatus, Resume, ResumeId, ResumePatch};
use crate::account::model::UserId;

/// Errors raised by resume persistence backends.
#[derive(Debug, Error)]
pub enum ResumeStoreError {
    /// The resume does not exist for this owner (or is soft-deleted).
    #[error("resume not found")]
    NotFound,
    /// The owner already holds the maximum number of resumes.
    #[error("resume limit reached")]
    LimitReached,
    /// The backend failed unexpectedly.
    #[error("resume store backend failure: {message}")]
    Backend {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl ResumeStoreError {
    pub fn backend(message: impl std::fmt::Display) -> Self {
        ResumeStoreError::Backend {
            message: message.to_string(),
            source: None,
        }
    }
}

/// Persistence port for resumes. All operations are scoped to a single owner.
#[async_trait]
pub trait ResumeStore: Send + Sync {
    /// Lists the owner's active resumes, default first, then newest first.
    async fn list(&self, owner: UserId) -> Result<Vec<Resume>, ResumeStoreError>;

    /// Fetches one active resume owned by `owner`.
    async fn get(&self, owner: UserId, id: ResumeId) -> Result<Resume, ResumeStoreError>;

    /// Inserts a new resume. Enforces the per-owner limit and the "first
    /// resume becomes default" rule inside a transaction.
    async fn insert(&self, owner: UserId, resume: NewResume) -> Result<Resume, ResumeStoreError>;

    /// Applies a partial update (title/content) and returns the fresh row.
    async fn update(
        &self,
        owner: UserId,
        id: ResumeId,
        patch: ResumePatch,
    ) -> Result<Resume, ResumeStoreError>;

    /// Makes `id` the owner's single default resume.
    async fn set_default(&self, owner: UserId, id: ResumeId) -> Result<Resume, ResumeStoreError>;

    /// Soft-deletes the resume; if it was the default, promotes the most
    /// recently updated remaining resume (all within one transaction).
    async fn soft_delete(&self, owner: UserId, id: ResumeId) -> Result<(), ResumeStoreError>;

    /// Attaches (or replaces) the attachment of an existing resume and stores
    /// the parse outcome plus the merged content in one update. The caller is
    /// responsible for cleaning up any replaced file on disk.
    async fn attach(
        &self,
        owner: UserId,
        id: ResumeId,
        attachment: NewResumeAttachment,
        parse_status: ParseStatus,
        content: serde_json::Value,
    ) -> Result<Resume, ResumeStoreError>;

    /// Removes the attachment (if any) and resets `parse_status` to
    /// `not_applicable`; content is left untouched.
    async fn detach(&self, owner: UserId, id: ResumeId) -> Result<Resume, ResumeStoreError>;
}
