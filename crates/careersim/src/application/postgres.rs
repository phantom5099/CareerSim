//! 投递 PostgreSQL 适配器。

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use tracing::instrument;

use super::{
    model::{
        Application, ApplicationError, ApplicationId, ApplicationStageView, ApplicationStatus,
        ApplicationView, OfferResponseStatus, OfferView,
    },
    store::{ApplicationStore, CreateApplicationRecord},
};
use crate::{
    account::model::UserId,
    agent::session::{SessionId, StageRef},
    catalog::model::{CompanyId, JobId, PipelineId},
    pipeline::model::{OfferTerms, StageKind},
    resume::model::ResumeId,
};

pub(crate) struct PgApplicationStore {
    pool: PgPool,
}

impl PgApplicationStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn backend(operation: &str, error: sqlx::Error) -> ApplicationError {
    ApplicationError::Backend {
        message: operation.to_owned(),
        source: Some(Box::new(error)),
    }
}

fn unique_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db) if db.is_unique_violation())
}

fn application_from_row(row: &sqlx::postgres::PgRow) -> Result<Application, ApplicationError> {
    let status: String = row
        .try_get("status")
        .map_err(|error| backend("read application", error))?;
    let status = ApplicationStatus::parse(&status)
        .ok_or_else(|| ApplicationError::InvalidState("unknown application status".to_owned()))?;
    let offer_response_status: String = row
        .try_get("offer_response_status")
        .map_err(|error| backend("read application", error))?;
    let offer_response_status =
        OfferResponseStatus::parse(&offer_response_status).ok_or_else(|| {
            ApplicationError::InvalidState("unknown offer response status".to_owned())
        })?;
    let stage_position: i32 = row
        .try_get("stage_position")
        .map_err(|error| backend("read application", error))?;
    let stage_position = u32::try_from(stage_position)
        .map_err(|_| ApplicationError::InvalidState("negative stage position".to_owned()))?;
    Ok(Application {
        id: ApplicationId::from_uuid(
            row.try_get("id")
                .map_err(|error| backend("read application", error))?,
        ),
        user_id: UserId::from_uuid(
            row.try_get("user_id")
                .map_err(|error| backend("read application", error))?,
        ),
        job_id: JobId::from_uuid(
            row.try_get("job_id")
                .map_err(|error| backend("read application", error))?,
        ),
        resume_id: ResumeId::from_uuid(
            row.try_get("resume_id")
                .map_err(|error| backend("read application", error))?,
        ),
        session_id: SessionId::from_uuid(
            row.try_get("session_id")
                .map_err(|error| backend("read application", error))?,
        ),
        status,
        offer_response_status,
        stage_position,
        created_at: row
            .try_get("created_at")
            .map_err(|error| backend("read application", error))?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|error| backend("read application", error))?,
    })
}

const LIST_APPLICATION_VIEWS_SQL: &str = "SELECT a.id, a.user_id, a.job_id, a.resume_id, a.session_id, a.status, a.stage_position, a.created_at, a.updated_at, \
    a.offer_response_status, \
    c.id AS company_id, \
    j.title AS job_title, r.title AS resume_title, j.pipeline_id \
    FROM applications a \
    JOIN jobs j ON j.id = a.job_id AND j.deleted_at IS NULL \
    JOIN recruitment_projects p ON p.id = j.project_id AND p.deleted_at IS NULL \
    JOIN companies c ON c.id = p.company_id AND c.deleted_at IS NULL \
    LEFT JOIN resumes r ON r.id = a.resume_id \
    WHERE a.user_id = $1 ORDER BY a.updated_at DESC, a.id DESC";
const GET_APPLICATION_VIEW_SQL: &str = "SELECT a.id, a.user_id, a.job_id, a.resume_id, a.session_id, a.status, a.stage_position, a.created_at, a.updated_at, \
    a.offer_response_status, \
    c.id AS company_id, \
    j.title AS job_title, r.title AS resume_title, j.pipeline_id \
    FROM applications a \
    JOIN jobs j ON j.id = a.job_id AND j.deleted_at IS NULL \
    JOIN recruitment_projects p ON p.id = j.project_id AND p.deleted_at IS NULL \
    JOIN companies c ON c.id = p.company_id AND c.deleted_at IS NULL \
    LEFT JOIN resumes r ON r.id = a.resume_id \
    WHERE a.id = $1 AND a.user_id = $2";
const LIST_PIPELINE_STAGES_SQL: &str =
    "SELECT position, kind, title FROM pipeline_stages WHERE pipeline_id = $1 ORDER BY position";
const LIST_OFFER_VIEWS_SQL: &str = "SELECT a.id, a.job_id, a.resume_id, a.offer_response_status, \
    c.id AS company_id, j.title AS job_title, r.title AS resume_title, \
    offer_event.offer_terms \
    FROM applications a \
    JOIN jobs j ON j.id = a.job_id AND j.deleted_at IS NULL \
    JOIN recruitment_projects p ON p.id = j.project_id AND p.deleted_at IS NULL \
    JOIN companies c ON c.id = p.company_id AND c.deleted_at IS NULL \
    LEFT JOIN resumes r ON r.id = a.resume_id \
    LEFT JOIN LATERAL ( \
        SELECT e.kind->'outcome'->'offer' AS offer_terms \
        FROM session_events e \
        WHERE e.session_id = a.session_id \
          AND e.kind->>'type' = 'stage_completed' \
          AND e.kind->'outcome'->>'type' = 'offer_pending' \
        ORDER BY e.seq DESC \
        LIMIT 1 \
    ) offer_event ON TRUE \
    WHERE a.user_id = $1 AND a.status = 'offer' \
    ORDER BY a.updated_at DESC, a.id DESC";
const GET_OFFER_VIEW_SQL: &str = "SELECT a.id, a.job_id, a.resume_id, a.offer_response_status, \
    c.id AS company_id, j.title AS job_title, r.title AS resume_title, \
    offer_event.offer_terms \
    FROM applications a \
    JOIN jobs j ON j.id = a.job_id AND j.deleted_at IS NULL \
    JOIN recruitment_projects p ON p.id = j.project_id AND p.deleted_at IS NULL \
    JOIN companies c ON c.id = p.company_id AND c.deleted_at IS NULL \
    LEFT JOIN resumes r ON r.id = a.resume_id \
    LEFT JOIN LATERAL ( \
        SELECT e.kind->'outcome'->'offer' AS offer_terms \
        FROM session_events e \
        WHERE e.session_id = a.session_id \
          AND e.kind->>'type' = 'stage_completed' \
          AND e.kind->'outcome'->>'type' = 'offer_pending' \
        ORDER BY e.seq DESC \
        LIMIT 1 \
    ) offer_event ON TRUE \
    WHERE a.id = $1 AND a.user_id = $2 AND a.status = 'offer'";
const INITIAL_STAGE_SQL: &str = "SELECT ps.position, ps.kind FROM jobs j \
    JOIN recruitment_projects p ON p.id = j.project_id AND p.deleted_at IS NULL \
    JOIN companies c ON c.id = p.company_id AND c.deleted_at IS NULL \
    JOIN pipeline_stages ps ON ps.pipeline_id = j.pipeline_id \
    WHERE j.id = $1 AND j.deleted_at IS NULL AND j.status = 'open' \
      AND (c.is_seed OR NOT c.is_private OR c.owner_user_id = $2) \
    ORDER BY ps.position LIMIT 1 FOR SHARE";

fn stage_view_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ApplicationStageView, ApplicationError> {
    let position: i16 = row
        .try_get("position")
        .map_err(|error| backend("read application stage", error))?;
    let position = u16::try_from(position)
        .map_err(|_| ApplicationError::InvalidState("negative stage position".to_owned()))?;
    let kind: String = row
        .try_get("kind")
        .map_err(|error| backend("read application stage", error))?;
    let kind = serde_json::from_value::<StageKind>(json!(kind))
        .map_err(|error| ApplicationError::InvalidState(error.to_string()))?;
    Ok(ApplicationStageView {
        position,
        kind,
        title: row
            .try_get("title")
            .map_err(|error| backend("read application stage", error))?,
    })
}

async fn load_pipeline_stages(
    pool: &PgPool,
    pipeline_id: PipelineId,
) -> Result<Vec<ApplicationStageView>, ApplicationError> {
    let rows = sqlx::query(LIST_PIPELINE_STAGES_SQL)
        .bind(pipeline_id.into_uuid())
        .fetch_all(pool)
        .await
        .map_err(|error| backend("list application stages", error))?;
    rows.iter().map(stage_view_from_row).collect()
}

fn application_view_from_row(
    row: &sqlx::postgres::PgRow,
    stages: Vec<ApplicationStageView>,
) -> Result<ApplicationView, ApplicationError> {
    Ok(ApplicationView {
        application: application_from_row(row)?,
        company_id: CompanyId::from_uuid(
            row.try_get("company_id")
                .map_err(|error| backend("read application company", error))?,
        ),
        job_title: row
            .try_get("job_title")
            .map_err(|error| backend("read application job", error))?,
        resume_title: row
            .try_get("resume_title")
            .map_err(|error| backend("read application resume", error))?,
        stages,
    })
}

fn offer_view_from_row(row: &sqlx::postgres::PgRow) -> Result<OfferView, ApplicationError> {
    let response_status: String = row
        .try_get("offer_response_status")
        .map_err(|error| backend("read offer response status", error))?;
    let response_status = OfferResponseStatus::parse(&response_status).ok_or_else(|| {
        ApplicationError::InvalidState("unknown offer response status".to_owned())
    })?;
    let offer_json: Option<Value> = row
        .try_get("offer_terms")
        .map_err(|error| backend("read offer terms", error))?;
    let offer_json = offer_json.ok_or_else(|| {
        ApplicationError::InvalidState("offer application has no offer terms".to_owned())
    })?;
    let offer: OfferTerms = serde_json::from_value(offer_json)
        .map_err(|error| ApplicationError::InvalidState(format!("invalid offer terms: {error}")))?;
    Ok(OfferView {
        application_id: ApplicationId::from_uuid(
            row.try_get("id")
                .map_err(|error| backend("read offer application", error))?,
        ),
        company_id: CompanyId::from_uuid(
            row.try_get("company_id")
                .map_err(|error| backend("read offer company", error))?,
        ),
        job_id: JobId::from_uuid(
            row.try_get("job_id")
                .map_err(|error| backend("read offer job", error))?,
        ),
        job_title: row
            .try_get("job_title")
            .map_err(|error| backend("read offer job title", error))?,
        resume_id: ResumeId::from_uuid(
            row.try_get("resume_id")
                .map_err(|error| backend("read offer resume", error))?,
        ),
        resume_title: row
            .try_get("resume_title")
            .map_err(|error| backend("read offer resume title", error))?,
        offer,
        response_status,
    })
}

#[async_trait]
impl ApplicationStore for PgApplicationStore {
    #[instrument(name = "application.postgres.list_views", level = "debug", skip(self), fields(owner_id = ?owner))]
    async fn list_views(&self, owner: UserId) -> Result<Vec<ApplicationView>, ApplicationError> {
        let rows = sqlx::query(LIST_APPLICATION_VIEWS_SQL)
            .bind(owner.into_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(|error| backend("list application views", error))?;
        let mut stages_by_pipeline: HashMap<PipelineId, Vec<ApplicationStageView>> = HashMap::new();
        let mut views = Vec::with_capacity(rows.len());
        for row in &rows {
            let pipeline_id = PipelineId::from_uuid(
                row.try_get("pipeline_id")
                    .map_err(|error| backend("read application pipeline", error))?,
            );
            let stages = if let Some(stages) = stages_by_pipeline.get(&pipeline_id) {
                stages.clone()
            } else {
                let stages = load_pipeline_stages(&self.pool, pipeline_id).await?;
                stages_by_pipeline.insert(pipeline_id, stages.clone());
                stages
            };
            views.push(application_view_from_row(row, stages)?);
        }
        Ok(views)
    }

    #[instrument(name = "application.postgres.get_view", level = "debug", skip(self), fields(owner_id = ?owner, application_id = ?id))]
    async fn get_view(
        &self,
        owner: UserId,
        id: ApplicationId,
    ) -> Result<ApplicationView, ApplicationError> {
        let row = sqlx::query(GET_APPLICATION_VIEW_SQL)
            .bind(id.into_uuid())
            .bind(owner.into_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| backend("get application view", error))?
            .ok_or(ApplicationError::NotFound)?;
        let pipeline_id = PipelineId::from_uuid(
            row.try_get("pipeline_id")
                .map_err(|error| backend("read application pipeline", error))?,
        );
        let stages = load_pipeline_stages(&self.pool, pipeline_id).await?;
        application_view_from_row(&row, stages)
    }

    #[instrument(name = "application.postgres.list_offer_views", level = "debug", skip(self), fields(owner_id = ?owner))]
    async fn list_offer_views(&self, owner: UserId) -> Result<Vec<OfferView>, ApplicationError> {
        let rows = sqlx::query(LIST_OFFER_VIEWS_SQL)
            .bind(owner.into_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(|error| backend("list offer views", error))?;
        rows.iter().map(offer_view_from_row).collect()
    }

    #[instrument(name = "application.postgres.update_offer_response", level = "debug", skip(self), fields(owner_id = ?owner, application_id = ?id, response_status = status.as_str()))]
    async fn update_offer_response(
        &self,
        owner: UserId,
        id: ApplicationId,
        status: OfferResponseStatus,
        now: DateTime<Utc>,
    ) -> Result<OfferView, ApplicationError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| backend("begin update offer response", error))?;
        let row = sqlx::query(
            "UPDATE applications SET offer_response_status = $3, updated_at = $4 \
             WHERE id = $1 AND user_id = $2 AND status = 'offer' \
             RETURNING id",
        )
        .bind(id.into_uuid())
        .bind(owner.into_uuid())
        .bind(status.as_str())
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| backend("update offer response", error))?;
        if row.is_none() {
            return Err(ApplicationError::NotFound);
        }
        tx.commit()
            .await
            .map_err(|error| backend("commit offer response", error))?;
        let row = sqlx::query(GET_OFFER_VIEW_SQL)
            .bind(id.into_uuid())
            .bind(owner.into_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| backend("get updated offer view", error))?
            .ok_or(ApplicationError::NotFound)?;
        offer_view_from_row(&row)
    }

    #[instrument(name = "application.postgres.initial_stage", level = "debug", skip(self), fields(owner_id = ?owner, job_id = ?job_id))]
    async fn initial_stage(
        &self,
        owner: UserId,
        job_id: JobId,
    ) -> Result<StageRef, ApplicationError> {
        let row = sqlx::query(INITIAL_STAGE_SQL)
            .bind(job_id.into_uuid())
            .bind(owner.into_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| backend("get application initial stage", error))?
            .ok_or(ApplicationError::JobUnavailable)?;
        let position: i16 = row
            .try_get("position")
            .map_err(|error| backend("read application initial stage", error))?;
        let kind: String = row
            .try_get("kind")
            .map_err(|error| backend("read application initial stage", error))?;
        Ok(StageRef {
            position: u16::try_from(position).map_err(|_| {
                ApplicationError::InvalidState("negative stage position".to_owned())
            })?,
            kind: serde_json::from_value::<StageKind>(json!(kind))
                .map_err(|error| ApplicationError::InvalidState(error.to_string()))?,
        })
    }

    #[instrument(name = "application.postgres.create", level = "debug", skip(self), fields(owner_id = ?owner, application_id = ?new.id, job_id = ?new.job_id, session_id = ?new.session_id))]
    async fn create(
        &self,
        owner: UserId,
        new: CreateApplicationRecord,
        initial_stage: StageRef,
        now: DateTime<Utc>,
    ) -> Result<Application, ApplicationError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| backend("begin create application", error))?;
        let stage = sqlx::query(INITIAL_STAGE_SQL)
            .bind(new.job_id.into_uuid())
            .bind(owner.into_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| backend("validate application initial stage", error))?
            .ok_or(ApplicationError::JobUnavailable)?;
        let stage_position: i16 = stage
            .try_get("position")
            .map_err(|error| backend("read application initial stage", error))?;
        let stage_position = u16::try_from(stage_position).map_err(|_| {
            ApplicationError::InvalidState("negative initial stage position".to_owned())
        })?;
        let stage_kind: String = stage
            .try_get("kind")
            .map_err(|error| backend("read application initial stage", error))?;
        if stage_position != initial_stage.position || stage_kind != initial_stage.kind.as_str() {
            return Err(ApplicationError::InvalidState(
                "initial stage changed while creating application".to_owned(),
            ));
        }
        let resume = sqlx::query(
            "SELECT 1 FROM resumes WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
        )
        .bind(new.resume_id.into_uuid())
        .bind(owner.into_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| backend("validate application resume", error))?;
        if resume.is_none() {
            return Err(ApplicationError::ResumeNotFound);
        }
        let session = sqlx::query(
            "SELECT 1 FROM sessions s \
             JOIN session_events e ON e.session_id = s.id AND e.seq = 0 \
             WHERE s.id = $1 \
               AND e.kind->>'type' = 'session_meta' \
               AND e.kind->'binding'->>'type' = 'application_stage' \
               AND e.kind->'binding'->>'application_id' = $2::text",
        )
        .bind(new.session_id.into_uuid())
        .bind(new.id.into_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| backend("validate application session", error))?;
        if session.is_none() {
            return Err(ApplicationError::SessionNotFound);
        }
        let row = sqlx::query(
            "INSERT INTO applications \
             (id, user_id, job_id, resume_id, session_id, status, stage_position, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, 'active', $6, $7, $7) \
             RETURNING id, user_id, job_id, resume_id, session_id, status, offer_response_status, stage_position, created_at, updated_at",
        )
        .bind(new.id.into_uuid())
        .bind(owner.into_uuid())
        .bind(new.job_id.into_uuid())
        .bind(new.resume_id.into_uuid())
        .bind(new.session_id.into_uuid())
        .bind(i32::from(initial_stage.position))
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            if unique_violation(&error) {
                ApplicationError::Conflict
            } else {
                backend("create application", error)
            }
        })?;
        tx.commit()
            .await
            .map_err(|error| backend("commit create application", error))?;
        application_from_row(&row)
    }

    #[instrument(name = "application.postgres.update_progress", level = "debug", skip(self), fields(owner_id = ?owner, application_id = ?id, status = status.as_str()))]
    async fn update_progress(
        &self,
        owner: UserId,
        id: ApplicationId,
        status: ApplicationStatus,
        stage_position: u32,
        now: DateTime<Utc>,
    ) -> Result<Application, ApplicationError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| backend("begin update application", error))?;
        let exists = sqlx::query("SELECT 1 FROM applications WHERE id = $1 AND user_id = $2")
            .bind(id.into_uuid())
            .bind(owner.into_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| backend("check application", error))?;
        if exists.is_none() {
            return Err(ApplicationError::NotFound);
        }
        let stage =
            sqlx::query(
                "SELECT 1 FROM applications a \
             JOIN jobs j ON j.id = a.job_id \
             JOIN pipeline_stages ps ON ps.pipeline_id = j.pipeline_id \
             WHERE a.id = $1 AND a.user_id = $2 AND ps.position = $3",
            )
            .bind(id.into_uuid())
            .bind(owner.into_uuid())
            .bind(i32::try_from(stage_position).map_err(|_| {
                ApplicationError::InvalidState("stage position overflow".to_owned())
            })?)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| backend("validate application stage position", error))?;
        if stage.is_none() {
            return Err(ApplicationError::InvalidState(
                "stage position is not present in the job pipeline".to_owned(),
            ));
        }
        let row = sqlx::query(
            "UPDATE applications SET status = $3, stage_position = $4, updated_at = $5 \
             WHERE id = $1 AND user_id = $2 \
             RETURNING id, user_id, job_id, resume_id, session_id, status, offer_response_status, stage_position, created_at, updated_at",
        )
        .bind(id.into_uuid())
        .bind(owner.into_uuid())
        .bind(status.as_str())
        .bind(i32::try_from(stage_position).map_err(|_| {
            ApplicationError::InvalidState("stage position overflow".to_owned())
        })?)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| backend("update application", error))?
        .ok_or(ApplicationError::NotFound)?;
        tx.commit()
            .await
            .map_err(|error| backend("commit update application", error))?;
        application_from_row(&row)
    }
}
