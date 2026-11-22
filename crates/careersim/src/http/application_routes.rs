//! 投递 HTTP 路由：列表、详情、当前阶段、创建与客户端阶段推进。

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    auth::AuthContext,
    error::{ApiError, render_backend_chain},
    router::AppState,
};
use crate::{
    agent::session::{ApplicationId, SessionBinding, SessionService},
    application::{ApplicationError, ApplicationStatus, NewApplication},
    catalog::model::JobId,
    pipeline::model::StageKind,
    resume::model::ResumeId,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/applications",
            get(list_applications).post(create_application),
        )
        .route("/api/v1/applications/{applicationId}", get(get_application))
        .route(
            "/api/v1/applications/{applicationId}/current",
            get(get_current_application),
        )
        .route(
            "/api/v1/applications/{applicationId}/advance",
            post(advance_application),
        )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplicationStageDto {
    position: u16,
    kind: StageKind,
    title: String,
    state: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplicationDto {
    id: String,
    company_id: String,
    job_id: String,
    job_title: String,
    resume_id: String,
    resume_title: String,
    applied_at: chrono::DateTime<Utc>,
    status: ApplicationStatus,
    current_stage: StageKind,
    stages: Vec<ApplicationStageDto>,
    next_action: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateApplicationRequest {
    job_id: Uuid,
    resume_id: Uuid,
}

impl From<ApplicationError> for ApiError {
    fn from(error: ApplicationError) -> Self {
        match error {
            ApplicationError::JobUnavailable
            | ApplicationError::ResumeNotFound
            | ApplicationError::SessionNotFound
            | ApplicationError::NotFound => ApiError::not_found("application resource not found"),
            ApplicationError::Conflict => ApiError::conflict("application already exists"),
            ApplicationError::InvalidState(reason) => {
                ApiError::validation_fields(reason, vec!["application".to_owned()])
            }
            ApplicationError::Backend { message, source } => {
                tracing::error!(
                    error_kind = "backend",
                    error = %render_backend_chain(&message, source.as_deref()),
                    "application backend error"
                );
                ApiError::internal_error()
            }
        }
    }
}

fn session_error(error: crate::agent::session::SessionError) -> ApiError {
    tracing::error!(error_kind = "session", error = %error, "application session error");
    ApiError::internal_error()
}

fn stage_state(status: ApplicationStatus, position: u16, current: u32) -> &'static str {
    if status == ApplicationStatus::Rejected && u32::from(position) == current {
        "failed"
    } else if u32::from(position) < current {
        "done"
    } else if u32::from(position) == current {
        "current"
    } else {
        "pending"
    }
}

fn next_action(kind: StageKind, title: &str, status: ApplicationStatus) -> String {
    match status {
        ApplicationStatus::Offer => "Offer 已发出 · 请在截止日前答复".to_owned(),
        ApplicationStatus::Rejected => "流程结束 · 可查看面试复盘".to_owned(),
        ApplicationStatus::Withdrawn => "投递已撤回".to_owned(),
        ApplicationStatus::Active => match kind {
            StageKind::HrScreen => "等待 HR 初筛结果".to_owned(),
            StageKind::Exam => "等待在线笔试安排".to_owned(),
            _ => format!("进入{title} · 等待安排"),
        },
    }
}

fn to_dto(view: crate::application::ApplicationView) -> Result<ApplicationDto, ApiError> {
    let current = view
        .stages
        .iter()
        .find(|stage| u32::from(stage.position) == view.application.stage_position)
        .ok_or_else(ApiError::internal_error)?;
    let stages = view
        .stages
        .iter()
        .map(|stage| ApplicationStageDto {
            position: stage.position,
            kind: stage.kind,
            title: stage.title.clone(),
            state: stage_state(
                view.application.status,
                stage.position,
                view.application.stage_position,
            ),
        })
        .collect();
    Ok(ApplicationDto {
        id: view.application.id.into_uuid().to_string(),
        company_id: view.company_id.into_uuid().to_string(),
        job_id: view.application.job_id.into_uuid().to_string(),
        job_title: view.job_title,
        resume_id: view.application.resume_id.into_uuid().to_string(),
        resume_title: view
            .resume_title
            .unwrap_or_else(|| "已删除的简历".to_owned()),
        applied_at: view.application.created_at,
        status: view.application.status,
        current_stage: current.kind,
        stages,
        next_action: next_action(current.kind, &current.title, view.application.status),
    })
}

async fn list_applications(
    State(state): State<AppState>,
    auth: AuthContext,
) -> Result<Json<Vec<ApplicationDto>>, ApiError> {
    let views = state.application.list_views(auth.user_id).await?;
    views
        .into_iter()
        .map(to_dto)
        .collect::<Result<Vec<_>, _>>()
        .map(Json)
}

async fn get_application(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApplicationDto>, ApiError> {
    let view = state
        .application
        .get_view(auth.user_id, ApplicationId::from_uuid(application_id))
        .await?;
    Ok(Json(to_dto(view)?))
}

async fn get_current_application(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApplicationDto>, ApiError> {
    get_application(State(state), auth, Path(application_id)).await
}

async fn create_application(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<CreateApplicationRequest>,
) -> Result<(StatusCode, Json<ApplicationDto>), ApiError> {
    let application_id = ApplicationId::generate();
    let stage = state
        .application
        .initial_stage(auth.user_id, JobId::from_uuid(request.job_id))
        .await?;
    let session = state
        .sessions
        .create(
            SessionBinding::ApplicationStage { application_id },
            stage.clone(),
        )
        .await
        .map_err(session_error)?;
    let created = state
        .application
        .create(
            auth.user_id,
            NewApplication {
                id: application_id,
                job_id: JobId::from_uuid(request.job_id),
                resume_id: ResumeId::from_uuid(request.resume_id),
                session_id: session.id,
                stage_position: u32::from(stage.position),
            },
            Utc::now(),
        )
        .await;
    if let Err(error) = created {
        let _ = state.sessions.delete(session.id).await;
        return Err(error.into());
    }
    let view = state
        .application
        .get_view(auth.user_id, application_id)
        .await?;
    Ok((StatusCode::CREATED, Json(to_dto(view)?)))
}

async fn advance_application(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApplicationDto>, ApiError> {
    let view = state
        .application
        .get_view(auth.user_id, ApplicationId::from_uuid(application_id))
        .await?;
    if view.application.status != ApplicationStatus::Active {
        return Ok(Json(to_dto(view)?));
    }
    let current_index = view
        .stages
        .iter()
        .position(|stage| u32::from(stage.position) == view.application.stage_position)
        .ok_or_else(ApiError::internal_error)?;
    let (status, next_position) = match view.stages.get(current_index + 1) {
        Some(next) => (ApplicationStatus::Active, u32::from(next.position)),
        None => (ApplicationStatus::Offer, view.application.stage_position),
    };
    state
        .application
        .update_progress(
            auth.user_id,
            ApplicationId::from_uuid(application_id),
            status,
            next_position,
            Utc::now(),
        )
        .await?;
    let updated = state
        .application
        .get_view(auth.user_id, ApplicationId::from_uuid(application_id))
        .await?;
    Ok(Json(to_dto(updated)?))
}
