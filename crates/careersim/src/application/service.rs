//! 投递应用服务；只负责投递记录持久化，不编排阶段推进。

use std::sync::Arc;

use chrono::{DateTime, Utc};

use super::{
    model::{
        Application, ApplicationError, ApplicationId, ApplicationStatus, CurrentApplicationStage,
        NewApplication,
    },
    store::ApplicationStore,
};
use crate::account::model::UserId;

pub struct ApplicationService {
    store: Arc<dyn ApplicationStore>,
}

impl ApplicationService {
    pub fn new(store: Arc<dyn ApplicationStore>) -> Self {
        Self { store }
    }

    pub async fn list(&self, owner: UserId) -> Result<Vec<Application>, ApplicationError> {
        self.store.list(owner).await
    }

    pub async fn get(
        &self,
        owner: UserId,
        id: ApplicationId,
    ) -> Result<Application, ApplicationError> {
        self.store.get(owner, id).await
    }

    pub async fn current_stage(
        &self,
        owner: UserId,
        id: ApplicationId,
    ) -> Result<CurrentApplicationStage, ApplicationError> {
        self.store.current_stage(owner, id).await
    }

    pub async fn create(
        &self,
        owner: UserId,
        new: NewApplication,
        now: DateTime<Utc>,
    ) -> Result<Application, ApplicationError> {
        self.store.create(owner, new, now).await
    }

    pub async fn update_progress(
        &self,
        owner: UserId,
        id: ApplicationId,
        status: ApplicationStatus,
        stage_position: u32,
        now: DateTime<Utc>,
    ) -> Result<Application, ApplicationError> {
        self.store
            .update_progress(owner, id, status, stage_position, now)
            .await
    }
}
