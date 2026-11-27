//! 投递持久化端口。

use async_trait::async_trait;
use chrono::DateTime;

use super::model::{
    Application, ApplicationError, ApplicationId, ApplicationStatus, ApplicationView,
    OfferResponseStatus, OfferView,
};
use crate::{account::model::UserId, agent::session::StageRef};

#[derive(Debug, Clone)]
pub struct CreateApplicationRecord {
    pub id: ApplicationId,
    pub job_id: crate::catalog::model::JobId,
    pub resume_id: crate::resume::model::ResumeId,
    pub session_id: crate::agent::session::SessionId,
}

#[async_trait]
pub trait ApplicationStore: Send + Sync {
    async fn list_views(&self, owner: UserId) -> Result<Vec<ApplicationView>, ApplicationError>;

    async fn get_view(
        &self,
        owner: UserId,
        id: ApplicationId,
    ) -> Result<ApplicationView, ApplicationError>;

    async fn list_offer_views(&self, owner: UserId) -> Result<Vec<OfferView>, ApplicationError>;

    async fn update_offer_response(
        &self,
        owner: UserId,
        id: ApplicationId,
        status: OfferResponseStatus,
        now: DateTime<chrono::Utc>,
    ) -> Result<OfferView, ApplicationError>;

    async fn initial_stage(
        &self,
        owner: UserId,
        job_id: crate::catalog::model::JobId,
    ) -> Result<crate::agent::session::StageRef, ApplicationError>;

    async fn create(
        &self,
        owner: UserId,
        new: CreateApplicationRecord,
        initial_stage: StageRef,
        now: DateTime<chrono::Utc>,
    ) -> Result<Application, ApplicationError>;

    async fn update_progress(
        &self,
        owner: UserId,
        id: ApplicationId,
        status: ApplicationStatus,
        stage_position: u32,
        now: DateTime<chrono::Utc>,
    ) -> Result<Application, ApplicationError>;
}
