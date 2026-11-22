//! 投递 PostgreSQL 适配器。

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{PgPool, Row};
use tracing::instrument;

use super::{
    model::{
        Application, ApplicationError, ApplicationId, ApplicationStageView, ApplicationStatus,
        ApplicationView, CurrentApplicationStage, NewApplication,
    },
    store::ApplicationStore,
};
use crate::{
    account::model::UserId,
    agent::session::{SessionId, StageRef},
    catalog::model::{CompanyId, JobId, PipelineId},
    pipeline::model::{StageDefinition, StageKind},
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
        stage_position,
        created_at: row
            .try_get("created_at")
            .map_err(|error| backend("read application", error))?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|error| backend("read application", error))?,
    })
}

const LIST_APPLICATIONS_SQL: &str = "SELECT id, user_id, job_id, resume_id, session_id, status, stage_position, created_at, updated_at \
    FROM applications WHERE user_id = $1 ORDER BY updated_at DESC, id DESC";
const GET_APPLICATION_SQL: &str = "SELECT id, user_id, job_id, resume_id, session_id, status, stage_position, created_at, updated_at \
    FROM applications WHERE id = $1 AND user_id = $2";
const GET_CURRENT_STAGE_SQL: &str = "SELECT a.id, a.user_id, a.job_id, a.resume_id, a.session_id, a.status, a.stage_position, a.created_at, a.updated_at, \
    j.pipeline_id, ps.position, ps.kind, ps.title, ps.rubric, ps.min_rounds, ps.max_rounds \
    FROM applications a \
    JOIN jobs j ON j.id = a.job_id \
    JOIN pipeline_stages ps ON ps.pipeline_id = j.pipeline_id AND ps.position = a.stage_position \
    WHERE a.id = $1 AND a.user_id = $2";
const LIST_APPLICATION_VIEWS_SQL: &str = "SELECT a.id, a.user_id, a.job_id, a.resume_id, a.session_id, a.status, a.stage_position, a.created_at, a.updated_at, \
    c.id AS company_id, \
    j.title AS job_title, r.title AS resume_title, j.pipeline_id \
    FROM applications a \
    JOIN jobs j ON j.id = a.job_id AND j.deleted_at IS NULL \
    JOIN recruitment_projects p ON p.id = j.project_id AND p.deleted_at IS NULL \
    JOIN companies c ON c.id = p.company_id AND c.deleted_at IS NULL \
    LEFT JOIN resumes r ON r.id = a.resume_id \
    WHERE a.user_id = $1 ORDER BY a.updated_at DESC, a.id DESC";
const GET_APPLICATION_VIEW_SQL: &str = "SELECT a.id, a.user_id, a.job_id, a.resume_id, a.session_id, a.status, a.stage_position, a.created_at, a.updated_at, \
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
const INITIAL_STAGE_SQL: &str = "SELECT ps.position, ps.kind FROM jobs j \
    JOIN recruitment_projects p ON p.id = j.project_id AND p.deleted_at IS NULL \
    JOIN companies c ON c.id = p.company_id AND c.deleted_at IS NULL \
    JOIN pipeline_stages ps ON ps.pipeline_id = j.pipeline_id AND ps.position = 0 \
    WHERE j.id = $1 AND j.deleted_at IS NULL AND j.status = 'open' \
      AND (c.is_seed OR NOT c.is_private OR c.owner_user_id = $2)";

fn stage_from_row(row: &sqlx::postgres::PgRow) -> Result<StageDefinition, ApplicationError> {
    let value = json!({
        "position": row
            .try_get::<i16, _>("position")
            .map_err(|error| backend("read application stage", error))?,
        "kind": row
            .try_get::<String, _>("kind")
            .map_err(|error| backend("read application stage", error))?,
        "title": row
            .try_get::<String, _>("title")
            .map_err(|error| backend("read application stage", error))?,
        "minRounds": row
            .try_get::<i16, _>("min_rounds")
            .map_err(|error| backend("read application stage", error))?,
        "maxRounds": row
            .try_get::<i16, _>("max_rounds")
            .map_err(|error| backend("read application stage", error))?,
        "rubric": row
            .try_get::<serde_json::Value, _>("rubric")
            .map_err(|error| backend("read application stage", error))?,
    });
    serde_json::from_value(value).map_err(|error| ApplicationError::InvalidState(error.to_string()))
}

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

#[async_trait]
impl ApplicationStore for PgApplicationStore {
    #[instrument(name = "application.postgres.list", level = "debug", skip(self), fields(owner_id = ?owner))]
    async fn list(&self, owner: UserId) -> Result<Vec<Application>, ApplicationError> {
        let rows = sqlx::query(LIST_APPLICATIONS_SQL)
            .bind(owner.into_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(|error| backend("list applications", error))?;
        rows.iter().map(application_from_row).collect()
    }

    #[instrument(name = "application.postgres.get", level = "debug", skip(self), fields(owner_id = ?owner, application_id = ?id))]
    async fn get(&self, owner: UserId, id: ApplicationId) -> Result<Application, ApplicationError> {
        let row = sqlx::query(GET_APPLICATION_SQL)
            .bind(id.into_uuid())
            .bind(owner.into_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| backend("get application", error))?
            .ok_or(ApplicationError::NotFound)?;
        application_from_row(&row)
    }

    #[instrument(name = "application.postgres.current_stage", level = "debug", skip(self), fields(owner_id = ?owner, application_id = ?id))]
    async fn current_stage(
        &self,
        owner: UserId,
        id: ApplicationId,
    ) -> Result<CurrentApplicationStage, ApplicationError> {
        let row = sqlx::query(GET_CURRENT_STAGE_SQL)
            .bind(id.into_uuid())
            .bind(owner.into_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| backend("get current application stage", error))?
            .ok_or(ApplicationError::NotFound)?;
        let application = application_from_row(&row)?;
        let pipeline_id = PipelineId::from_uuid(
            row.try_get("pipeline_id")
                .map_err(|error| backend("read application pipeline", error))?,
        );
        Ok(CurrentApplicationStage {
            application,
            pipeline_id,
            stage: stage_from_row(&row)?,
        })
    }

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
        new: NewApplication,
        now: DateTime<Utc>,
    ) -> Result<Application, ApplicationError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| backend("begin create application", error))?;
        let job = sqlx::query(
            "SELECT 1 FROM jobs j \
             JOIN recruitment_projects p ON p.id = j.project_id AND p.deleted_at IS NULL \
             JOIN companies c ON c.id = p.company_id AND c.deleted_at IS NULL \
             WHERE j.id = $1 AND j.deleted_at IS NULL AND j.status = 'open' \
               AND (c.is_seed OR NOT c.is_private OR c.owner_user_id = $2)",
        )
        .bind(new.job_id.into_uuid())
        .bind(owner.into_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| backend("validate application job", error))?;
        if job.is_none() {
            return Err(ApplicationError::JobUnavailable);
        }
        let stage =
            sqlx::query(
                "SELECT 1 FROM jobs j \
             JOIN pipeline_stages ps ON ps.pipeline_id = j.pipeline_id \
             WHERE j.id = $1 AND ps.position = $2",
            )
            .bind(new.job_id.into_uuid())
            .bind(i32::try_from(new.stage_position).map_err(|_| {
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
             RETURNING id, user_id, job_id, resume_id, session_id, status, stage_position, created_at, updated_at",
        )
        .bind(new.id.into_uuid())
        .bind(owner.into_uuid())
        .bind(new.job_id.into_uuid())
        .bind(new.resume_id.into_uuid())
        .bind(new.session_id.into_uuid())
        .bind(i32::try_from(new.stage_position).map_err(|_| {
            ApplicationError::InvalidState("stage position overflow".to_owned())
        })?)
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
             RETURNING id, user_id, job_id, resume_id, session_id, status, stage_position, created_at, updated_at",
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
