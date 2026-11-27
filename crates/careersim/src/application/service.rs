//! 投递应用服务；只负责投递记录持久化，不编排阶段推进。

use std::sync::Arc;

use chrono::{DateTime, Utc};

use super::{
    model::{
        Application, ApplicationError, ApplicationId, ApplicationStatus, ApplicationView,
        CreateApplicationInput, OfferResponseStatus, OfferView,
    },
    store::{ApplicationStore, CreateApplicationRecord},
};
use crate::{account::model::UserId, agent::session::StageRef};

pub struct ApplicationService {
    store: Arc<dyn ApplicationStore>,
}

impl ApplicationService {
    pub fn new(store: Arc<dyn ApplicationStore>) -> Self {
        Self { store }
    }

    pub async fn list_views(
        &self,
        owner: UserId,
    ) -> Result<Vec<ApplicationView>, ApplicationError> {
        self.store.list_views(owner).await
    }

    pub async fn get_view(
        &self,
        owner: UserId,
        id: ApplicationId,
    ) -> Result<ApplicationView, ApplicationError> {
        self.store.get_view(owner, id).await
    }

    pub async fn list_offer_views(
        &self,
        owner: UserId,
    ) -> Result<Vec<OfferView>, ApplicationError> {
        self.store.list_offer_views(owner).await
    }

    pub async fn update_offer_response(
        &self,
        owner: UserId,
        id: ApplicationId,
        status: OfferResponseStatus,
        now: DateTime<Utc>,
    ) -> Result<OfferView, ApplicationError> {
        self.store
            .update_offer_response(owner, id, status, now)
            .await
    }

    pub async fn initial_stage(
        &self,
        owner: UserId,
        job_id: crate::catalog::model::JobId,
    ) -> Result<crate::agent::session::StageRef, ApplicationError> {
        self.store.initial_stage(owner, job_id).await
    }

    pub async fn create(
        &self,
        owner: UserId,
        new: CreateApplicationInput,
        initial_stage: StageRef,
        now: DateTime<Utc>,
    ) -> Result<Application, ApplicationError> {
        let record = CreateApplicationRecord {
            id: new.id,
            job_id: new.job_id,
            resume_id: new.resume_id,
            session_id: new.session_id,
        };
        self.store.create(owner, record, initial_stage, now).await
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
