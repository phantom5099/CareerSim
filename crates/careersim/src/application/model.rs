//! 投递领域模型。

use chrono::{DateTime, Utc};

pub use crate::agent::session::ApplicationId;
use crate::{
    account::model::UserId,
    agent::session::SessionId,
    catalog::model::{JobId, PipelineId},
    pipeline::model::StageDefinition,
    resume::model::ResumeId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationStatus {
    Active,
    Rejected,
    Offer,
    Withdrawn,
}

impl ApplicationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Rejected => "rejected",
            Self::Offer => "offer",
            Self::Withdrawn => "withdrawn",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "rejected" => Some(Self::Rejected),
            "offer" => Some(Self::Offer),
            "withdrawn" => Some(Self::Withdrawn),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application {
    pub id: ApplicationId,
    pub user_id: UserId,
    pub job_id: JobId,
    pub resume_id: ResumeId,
    pub session_id: SessionId,
    pub status: ApplicationStatus,
    /// 当前流程位置，从零开始，唯一的当前阶段定位依据。
    pub stage_position: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewApplication {
    pub id: ApplicationId,
    pub job_id: JobId,
    pub resume_id: ResumeId,
    pub session_id: SessionId,
    pub stage_position: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentApplicationStage {
    pub application: Application,
    pub pipeline_id: PipelineId,
    pub stage: StageDefinition,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("application not found")]
    NotFound,
    #[error("job is not available for application")]
    JobUnavailable,
    #[error("resume not found")]
    ResumeNotFound,
    #[error("session not found")]
    SessionNotFound,
    #[error("application already exists for this job")]
    Conflict,
    #[error("invalid application state: {0}")]
    InvalidState(String),
    #[error("application store backend error: {message}")]
    Backend {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}
