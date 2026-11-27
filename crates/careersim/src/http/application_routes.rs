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
    format_timestamp,
    router::AppState,
};
use crate::{
    agent::session::{ApplicationId, SessionBinding, SessionEventKind, SessionService},
    application::{
        ApplicationError, ApplicationStatus, CreateApplicationInput, OfferResponseStatus, OfferView,
    },
    catalog::model::JobId,
    pipeline::model::{StageKind, StageOutcome},
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
        .route("/api/v1/offers", get(list_offers))
        .route(
            "/api/v1/offers/{applicationId}/respond",
            post(respond_to_offer),
        )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplicationStageResponse {
    position: u16,
    kind: String,
    title: String,
    state: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplicationResponse {
    id: String,
    company_id: String,
    job_id: String,
    job_title: String,
    resume_id: String,
    resume_title: String,
    applied_at: String,
    status: String,
    current_stage: String,
    stages: Vec<ApplicationStageResponse>,
    next_action: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OfferResponse {
    id: String,
    company_id: String,
    job_id: String,
    job_title: String,
    resume_id: String,
    resume_title: String,
    base_salary: u32,
    months: u8,
    bonus: String,
    city: String,
    issued_at: String,
    deadline: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OfferResponseRequest {
    status: OfferResponseStatus,
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

fn session_has_offer_terms(session: &crate::agent::session::Session) -> bool {
    session.events.iter().any(|event| {
        matches!(
            event.kind,
            SessionEventKind::StageCompleted {
                outcome: StageOutcome::OfferPending { .. },
                ..
            }
        )
    })
}

fn to_response(view: crate::application::ApplicationView) -> Result<ApplicationResponse, ApiError> {
    let current = view
        .stages
        .iter()
        .find(|stage| u32::from(stage.position) == view.application.stage_position)
        .ok_or_else(ApiError::internal_error)?;
    let stages = view
        .stages
        .iter()
        .map(|stage| ApplicationStageResponse {
            position: stage.position,
            kind: stage.kind.as_str().to_owned(),
            title: stage.title.clone(),
            state: stage_state(
                view.application.status,
                stage.position,
                view.application.stage_position,
            ),
        })
        .collect();
    Ok(ApplicationResponse {
        id: view.application.id.into_uuid().to_string(),
        company_id: view.company_id.into_uuid().to_string(),
        job_id: view.application.job_id.into_uuid().to_string(),
        job_title: view.job_title,
        resume_id: view.application.resume_id.into_uuid().to_string(),
        resume_title: view
            .resume_title
            .unwrap_or_else(|| "已删除的简历".to_owned()),
        applied_at: format_timestamp(view.application.created_at),
        status: view.application.status.as_str().to_owned(),
        current_stage: current.kind.as_str().to_owned(),
        stages,
        next_action: next_action(current.kind, &current.title, view.application.status),
    })
}

fn to_offer_response(view: OfferView) -> OfferResponse {
    OfferResponse {
        id: view.application_id.into_uuid().to_string(),
        company_id: view.company_id.into_uuid().to_string(),
        job_id: view.job_id.into_uuid().to_string(),
        job_title: view.job_title,
        resume_id: view.resume_id.into_uuid().to_string(),
        resume_title: view
            .resume_title
            .unwrap_or_else(|| "已删除的简历".to_owned()),
        base_salary: view.offer.base_salary,
        months: view.offer.months,
        bonus: view.offer.bonus,
        city: view.offer.city,
        issued_at: format_timestamp(view.offer.issued_at),
        deadline: view.offer.deadline.to_string(),
        status: view.response_status.as_str().to_owned(),
    }
}

async fn list_applications(
    State(state): State<AppState>,
    auth: AuthContext,
) -> Result<Json<Vec<ApplicationResponse>>, ApiError> {
    let views = state.application.list_views(auth.user_id).await?;
    views
        .into_iter()
        .map(to_response)
        .collect::<Result<Vec<_>, _>>()
        .map(Json)
}

async fn list_offers(
    State(state): State<AppState>,
    auth: AuthContext,
) -> Result<Json<Vec<OfferResponse>>, ApiError> {
    let views = state.application.list_offer_views(auth.user_id).await?;
    Ok(Json(views.into_iter().map(to_offer_response).collect()))
}

async fn respond_to_offer(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(application_id): Path<Uuid>,
    Json(request): Json<OfferResponseRequest>,
) -> Result<Json<OfferResponse>, ApiError> {
    let view = state
        .application
        .update_offer_response(
            auth.user_id,
            ApplicationId::from_uuid(application_id),
            request.status,
            Utc::now(),
        )
        .await?;
    Ok(Json(to_offer_response(view)))
}

async fn get_application(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApplicationResponse>, ApiError> {
    let view = state
        .application
        .get_view(auth.user_id, ApplicationId::from_uuid(application_id))
        .await?;
    Ok(Json(to_response(view)?))
}

async fn get_current_application(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApplicationResponse>, ApiError> {
    get_application(State(state), auth, Path(application_id)).await
}

async fn create_application(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<CreateApplicationRequest>,
) -> Result<(StatusCode, Json<ApplicationResponse>), ApiError> {
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
            CreateApplicationInput {
                id: application_id,
                job_id: JobId::from_uuid(request.job_id),
                resume_id: ResumeId::from_uuid(request.resume_id),
                session_id: session.id,
            },
            stage,
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
    Ok((StatusCode::CREATED, Json(to_response(view)?)))
}

async fn advance_application(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApplicationResponse>, ApiError> {
    let view = state
        .application
        .get_view(auth.user_id, ApplicationId::from_uuid(application_id))
        .await?;
    if view.application.status != ApplicationStatus::Active {
        return Ok(Json(to_response(view)?));
    }
    let current_index = view
        .stages
        .iter()
        .position(|stage| u32::from(stage.position) == view.application.stage_position)
        .ok_or_else(ApiError::internal_error)?;
    let (status, next_position) = match view.stages.get(current_index + 1) {
        Some(next) => (ApplicationStatus::Active, u32::from(next.position)),
        None => {
            let session = state
                .sessions
                .load(view.application.session_id)
                .await
                .map_err(session_error)?;
            if !session_has_offer_terms(&session) {
                return Err(ApiError::validation_fields(
                    "offer terms are not persisted for this application",
                    vec!["application".to_owned()],
                ));
            }
            (ApplicationStatus::Offer, view.application.stage_position)
        }
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
    Ok(Json(to_response(updated)?))
}
