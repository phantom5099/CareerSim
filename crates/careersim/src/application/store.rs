//! 投递持久化端口。

use async_trait::async_trait;
use chrono::DateTime;

use super::model::{
    Application, ApplicationError, ApplicationId, ApplicationStatus, CurrentApplicationStage,
    NewApplication,
};
use crate::account::model::UserId;

#[async_trait]
pub trait ApplicationStore: Send + Sync {
    async fn list(&self, owner: UserId) -> Result<Vec<Application>, ApplicationError>;

    async fn get(&self, owner: UserId, id: ApplicationId) -> Result<Application, ApplicationError>;

    async fn current_stage(
        &self,
        owner: UserId,
        id: ApplicationId,
    ) -> Result<CurrentApplicationStage, ApplicationError>;

    async fn create(
        &self,
        owner: UserId,
        new: NewApplication,
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
