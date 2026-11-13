//! catalog 路由处理器：目录浏览、模板 CRUD 与 CompanyTemplateV1 导入
//! （plan/01 §6.2）。所有路由都需要登录；可见性与写权限由存储层保证。

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::catalog::model::{Company, CompanySummary};
use crate::catalog::service::{
    CatalogService, CompanyInput, CompanyPatchInput, JobInput, JobPatchInput, PipelineInput,
    ProjectInput, ProjectPatchInput,
};
use crate::catalog::store::CatalogScope;
use crate::catalog::template::TemplateError;
use crate::catalog::{
    CatalogError, CompanyId, JobId, JobStatus, JobType, ListCursor, PipelineId, ProjectId,
    ProjectStatus,
};
use crate::pipeline::StageDefinition;

use super::auth::AuthContext;
use super::error::ApiError;
use super::router::AppState;

#[cfg(test)]
mod tests;

/// catalog 路由组；state 由外层 `router()` 统一注入。
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/companies",
            get(list_companies).post(create_company),
        )
        .route(
            "/api/v1/companies/{companyId}",
            get(get_company)
                .patch(update_company)
                .delete(delete_company),
        )
        .route(
            "/api/v1/companies/{companyId}/projects",
            post(create_project),
        )
        .route(
            "/api/v1/projects/{projectId}",
            patch(update_project).delete(delete_project),
        )
        .route(
            "/api/v1/projects/{projectId}/pipelines",
            post(create_pipeline),
        )
        .route(
            "/api/v1/pipelines/{pipelineId}",
            patch(update_pipeline).delete(delete_pipeline),
        )
        .route("/api/v1/projects/{projectId}/jobs", post(create_job))
        .route("/api/v1/jobs/{jobId}", patch(update_job).delete(delete_job))
        .route("/api/v1/companies/import/validate", post(validate_template))
        .route("/api/v1/companies/import", post(import_template))
}

// ---------- 错误映射 ----------

impl From<CatalogError> for ApiError {
    fn from(err: CatalogError) -> Self {
        match err {
            CatalogError::InvalidInput { field, reason } => ApiError::validation_fields(
                format!("invalid {field}: {reason}"),
                vec![field.to_owned()],
            ),
            CatalogError::InvalidTemplate(errors) => ApiError::validation_fields(
                "template validation failed",
                errors.into_iter().map(render_template_issue).collect(),
            ),
            CatalogError::InvalidReference { field } => ApiError::validation_fields(
                format!("referenced {field} does not exist"),
                vec![field.to_owned()],
            ),
            CatalogError::NotFound => ApiError::not_found("resource not found"),
            CatalogError::Conflict(field) => {
                ApiError::conflict(format!("`{field}` already exists"))
            }
            // 后端错误细节只进日志；不透出 SQL 与连接信息。
            CatalogError::Backend { message, source } => {
                match source.as_deref() {
                    Some(inner) => tracing::error!(
                        operation = %message,
                        error = %crate::error::error_chain(inner),
                        "catalog backend error"
                    ),
                    None => tracing::error!(operation = %message, "catalog backend error"),
                }
                ApiError::internal_error()
            }
        }
    }
}

fn render_template_issue(issue: TemplateError) -> String {
    if issue.path.is_empty() {
        issue.message
    } else {
        format!("{}: {}", issue.path, issue.message)
    }
}

// ---------- 输入 DTO ----------

fn parse_status<T>(
    raw: &str,
    field: &'static str,
    allowed: &str,
    parse: fn(&str) -> Option<T>,
) -> Result<T, ApiError> {
    parse(raw.trim()).ok_or_else(|| {
        ApiError::validation_fields(
            format!("invalid {field}: must be one of {allowed}"),
            vec![field.to_owned()],
        )
    })
}

fn parse_date(raw: &str, field: &'static str) -> Result<NaiveDate, ApiError> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").map_err(|_| {
        ApiError::validation_fields(
            format!("invalid {field}: must be a YYYY-MM-DD date"),
            vec![field.to_owned()],
        )
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompanyRequest {
    name: String,
    #[serde(default)]
    english_name: String,
    industry: String,
    #[serde(default)]
    city: String,
    #[serde(default)]
    size: String,
    #[serde(default)]
    tagline: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    highlights: Vec<String>,
    #[serde(default)]
    logo_text: String,
    #[serde(default)]
    accent: String,
    #[serde(default)]
    is_private: bool,
}

impl From<CompanyRequest> for CompanyInput {
    fn from(dto: CompanyRequest) -> Self {
        Self {
            name: dto.name,
            english_name: dto.english_name,
            industry: dto.industry,
            city: dto.city,
            size: dto.size,
            tagline: dto.tagline,
            description: dto.description,
            highlights: dto.highlights,
            logo_text: dto.logo_text,
            accent: dto.accent,
            is_private: dto.is_private,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompanyPatchRequest {
    name: Option<String>,
    english_name: Option<String>,
    industry: Option<String>,
    city: Option<String>,
    size: Option<String>,
    tagline: Option<String>,
    description: Option<String>,
    highlights: Option<Vec<String>>,
    logo_text: Option<String>,
    accent: Option<String>,
    is_private: Option<bool>,
}

impl From<CompanyPatchRequest> for CompanyPatchInput {
    fn from(dto: CompanyPatchRequest) -> Self {
        Self {
            name: dto.name,
            english_name: dto.english_name,
            industry: dto.industry,
            city: dto.city,
            size: dto.size,
            tagline: dto.tagline,
            description: dto.description,
            highlights: dto.highlights,
            logo_text: dto.logo_text,
            accent: dto.accent,
            is_private: dto.is_private,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRequest {
    name: String,
    #[serde(default)]
    season: String,
    status: Option<String>,
    deadline: Option<String>,
    #[serde(default)]
    description: String,
}

impl ProjectRequest {
    fn into_input(self) -> Result<ProjectInput, ApiError> {
        Ok(ProjectInput {
            name: self.name,
            season: self.season,
            status: self
                .status
                .map(|raw| {
                    parse_status(
                        &raw,
                        "status",
                        "open / closing / closed",
                        ProjectStatus::parse,
                    )
                })
                .transpose()?,
            deadline: self
                .deadline
                .map(|raw| parse_date(&raw, "deadline"))
                .transpose()?,
            description: self.description,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectPatchRequest {
    name: Option<String>,
    season: Option<String>,
    status: Option<String>,
    /// 外层 `None` 不修改，内层 `None` 清空截止日期。
    deadline: Option<Option<String>>,
    description: Option<String>,
}

impl ProjectPatchRequest {
    fn into_input(self) -> Result<ProjectPatchInput, ApiError> {
        Ok(ProjectPatchInput {
            name: self.name,
            season: self.season,
            status: self
                .status
                .map(|raw| {
                    parse_status(
                        &raw,
                        "status",
                        "open / closing / closed",
                        ProjectStatus::parse,
                    )
                })
                .transpose()?,
            deadline: self
                .deadline
                .map(|inner| inner.map(|raw| parse_date(&raw, "deadline")).transpose())
                .transpose()?,
            description: self.description,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PipelineRequest {
    name: String,
    /// 阶段定义必填；缺省或为空会被服务层拒绝。
    stages: Option<Vec<StageDefinition>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobRequest {
    pipeline_id: Uuid,
    title: String,
    #[serde(default)]
    city: String,
    job_type: Option<String>,
    salary_min: Option<u32>,
    salary_max: Option<u32>,
    salary_months: Option<u8>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    description: String,
    status: Option<String>,
}

impl JobRequest {
    fn into_input(self) -> Result<JobInput, ApiError> {
        Ok(JobInput {
            pipeline_id: Some(PipelineId::from_uuid(self.pipeline_id)),
            title: self.title,
            city: self.city,
            job_type: self
                .job_type
                .map(|raw| {
                    parse_status(
                        &raw,
                        "jobType",
                        "campus / internship / experienced",
                        JobType::parse,
                    )
                })
                .transpose()?,
            salary_min: self.salary_min,
            salary_max: self.salary_max,
            salary_months: self.salary_months,
            tags: self.tags,
            description: self.description,
            status: self
                .status
                .map(|raw| parse_status(&raw, "status", "draft / open / closed", JobStatus::parse))
                .transpose()?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobPatchRequest {
    pipeline_id: Option<Uuid>,
    title: Option<String>,
    city: Option<String>,
    job_type: Option<String>,
    salary_min: Option<Option<u32>>,
    salary_max: Option<Option<u32>>,
    salary_months: Option<Option<u8>>,
    tags: Option<Vec<String>>,
    description: Option<String>,
    status: Option<String>,
}

impl JobPatchRequest {
    fn into_input(self) -> Result<JobPatchInput, ApiError> {
        Ok(JobPatchInput {
            pipeline_id: self.pipeline_id.map(PipelineId::from_uuid),
            title: self.title,
            city: self.city,
            job_type: self
                .job_type
                .map(|raw| {
                    parse_status(
                        &raw,
                        "jobType",
                        "campus / internship / experienced",
                        JobType::parse,
                    )
                })
                .transpose()?,
            salary_min: self.salary_min,
            salary_max: self.salary_max,
            salary_months: self.salary_months,
            tags: self.tags,
            description: self.description,
            status: self
                .status
                .map(|raw| parse_status(&raw, "status", "draft / open / closed", JobStatus::parse))
                .transpose()?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListQuery {
    scope: Option<String>,
    limit: Option<u32>,
    cursor: Option<String>,
}

fn parse_cursor(raw: &str) -> Result<ListCursor, ApiError> {
    let invalid = || ApiError::validation_failed("invalid cursor");
    let (millis, id) = raw.split_once(':').ok_or_else(invalid)?;
    let millis: i64 = millis.parse().map_err(|_| invalid())?;
    let id = Uuid::parse_str(id).map_err(|_| invalid())?;
    let updated_at = DateTime::from_timestamp_millis(millis).ok_or_else(invalid)?;
    Ok(ListCursor { updated_at, id })
}

// ---------- 响应 DTO ----------

fn company_dto(company: &Company, viewer: crate::account::UserId) -> CompanyDto {
    CompanyDto {
        id: company.id.into_uuid().to_string(),
        source: company.source.as_str(),
        owner_id: company.owner.map(|user| user.into_uuid().to_string()),
        is_private: company.is_private,
        is_owner: company.is_owned_by(viewer),
        name: company.name.clone(),
        english_name: company.english_name.clone(),
        industry: company.industry.clone(),
        city: company.city.clone(),
        size: company.size.clone(),
        tagline: company.tagline.clone(),
        description: company.description.clone(),
        highlights: company.highlights.clone(),
        logo_text: company.logo_text.clone(),
        accent: company.accent.clone(),
        created_at: company.created_at.to_rfc3339(),
        updated_at: company.updated_at.to_rfc3339(),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanyDto {
    id: String,
    source: &'static str,
    owner_id: Option<String>,
    is_private: bool,
    is_owner: bool,
    name: String,
    english_name: String,
    industry: String,
    city: String,
    size: String,
    tagline: String,
    description: String,
    highlights: Vec<String>,
    logo_text: String,
    accent: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectDto {
    id: String,
    company_id: String,
    name: String,
    season: String,
    status: String,
    deadline: Option<String>,
    description: String,
    created_at: String,
    updated_at: String,
}

impl From<crate::catalog::RecruitmentProject> for ProjectDto {
    fn from(project: crate::catalog::RecruitmentProject) -> Self {
        Self {
            id: project.id.into_uuid().to_string(),
            company_id: project.company_id.into_uuid().to_string(),
            name: project.name,
            season: project.season,
            status: project.status.as_str().to_owned(),
            deadline: project.deadline.map(|date| date.to_string()),
            description: project.description,
            created_at: project.created_at.to_rfc3339(),
            updated_at: project.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PipelineDto {
    id: String,
    project_id: String,
    name: String,
    stages: Vec<StageDefinition>,
    created_at: String,
    updated_at: String,
}

impl From<crate::catalog::Pipeline> for PipelineDto {
    fn from(pipeline: crate::catalog::Pipeline) -> Self {
        Self {
            id: pipeline.id.into_uuid().to_string(),
            project_id: pipeline.project_id.into_uuid().to_string(),
            name: pipeline.name,
            stages: pipeline.definition.stages().to_vec(),
            created_at: pipeline.created_at.to_rfc3339(),
            updated_at: pipeline.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JobDto {
    id: String,
    project_id: String,
    pipeline_id: String,
    title: String,
    city: String,
    job_type: String,
    salary_min: Option<u32>,
    salary_max: Option<u32>,
    salary_months: Option<u8>,
    tags: Vec<String>,
    description: String,
    status: String,
    created_at: String,
    updated_at: String,
}

impl From<crate::catalog::Job> for JobDto {
    fn from(job: crate::catalog::Job) -> Self {
        Self {
            id: job.id.into_uuid().to_string(),
            project_id: job.project_id.into_uuid().to_string(),
            pipeline_id: job.pipeline_id.into_uuid().to_string(),
            title: job.title,
            city: job.city,
            job_type: job.job_type.as_str().to_owned(),
            salary_min: job.salary_min,
            salary_max: job.salary_max,
            salary_months: job.salary_months,
            tags: job.tags,
            description: job.description,
            status: job.status.as_str().to_owned(),
            created_at: job.created_at.to_rfc3339(),
            updated_at: job.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
struct CompanyDetailDto {
    #[serde(flatten)]
    company: CompanyDto,
    projects: Vec<ProjectDto>,
    pipelines: Vec<PipelineDto>,
    jobs: Vec<JobDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanyListItemDto {
    #[serde(flatten)]
    company: CompanyDto,
    project_count: u32,
    visible_job_count: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanyListDto {
    items: Vec<CompanyListItemDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TemplateIssueDto {
    path: String,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TemplatePreviewDto {
    name: String,
    industry: String,
    project_count: usize,
    pipeline_count: usize,
    job_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TemplateValidateDto {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<TemplatePreviewDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<Vec<TemplateIssueDto>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportCreatedDto {
    company_id: String,
}

// ---------- 处理器 ----------

async fn list_companies(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(query): Query<ListQuery>,
) -> Result<Json<CompanyListDto>, ApiError> {
    let scope = match query.scope.as_deref().unwrap_or("all") {
        "mine" => CatalogScope::Mine,
        "public" => CatalogScope::Public,
        "all" => CatalogScope::All,
        other => {
            return Err(ApiError::validation_fields(
                format!("invalid scope: {other}"),
                vec!["scope".to_owned()],
            ));
        }
    };
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let cursor = match &query.cursor {
        Some(raw) => Some(parse_cursor(raw)?),
        None => None,
    };
    let (summaries, next_cursor) = state
        .catalog
        .list_companies(auth.user_id, scope, limit, cursor)
        .await?;
    let items = summaries
        .iter()
        .map(|summary: &CompanySummary| CompanyListItemDto {
            company: company_dto(&summary.company, auth.user_id),
            project_count: summary.project_count,
            visible_job_count: summary.visible_job_count,
        })
        .collect();
    Ok(Json(CompanyListDto {
        items,
        next_cursor: next_cursor.map(|cursor| cursor.to_string()),
    }))
}

async fn get_company(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(company_id): Path<Uuid>,
) -> Result<Json<CompanyDetailDto>, ApiError> {
    let detail = state
        .catalog
        .get_company(auth.user_id, CompanyId::from_uuid(company_id))
        .await?
        .ok_or_else(|| ApiError::not_found("resource not found"))?;
    Ok(Json(CompanyDetailDto {
        company: company_dto(&detail.company, auth.user_id),
        projects: detail.projects.into_iter().map(ProjectDto::from).collect(),
        pipelines: detail
            .pipelines
            .into_iter()
            .map(PipelineDto::from)
            .collect(),
        jobs: detail.jobs.into_iter().map(JobDto::from).collect(),
    }))
}

async fn create_company(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<CompanyRequest>,
) -> Result<(StatusCode, Json<CompanyDto>), ApiError> {
    let company = state
        .catalog
        .create_company(auth.user_id, request.into(), Utc::now())
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(company_dto(&company, auth.user_id)),
    ))
}

async fn update_company(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(company_id): Path<Uuid>,
    Json(request): Json<CompanyPatchRequest>,
) -> Result<Json<CompanyDto>, ApiError> {
    let company = state
        .catalog
        .update_company(
            auth.user_id,
            CompanyId::from_uuid(company_id),
            request.into(),
            Utc::now(),
        )
        .await?;
    Ok(Json(company_dto(&company, auth.user_id)))
}

async fn delete_company(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(company_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .delete_company(auth.user_id, CompanyId::from_uuid(company_id), Utc::now())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_project(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(company_id): Path<Uuid>,
    Json(request): Json<ProjectRequest>,
) -> Result<(StatusCode, Json<ProjectDto>), ApiError> {
    let project = state
        .catalog
        .create_project(
            auth.user_id,
            CompanyId::from_uuid(company_id),
            request.into_input()?,
            Utc::now(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(ProjectDto::from(project))))
}

async fn update_project(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(project_id): Path<Uuid>,
    Json(request): Json<ProjectPatchRequest>,
) -> Result<Json<ProjectDto>, ApiError> {
    let project = state
        .catalog
        .update_project(
            auth.user_id,
            ProjectId::from_uuid(project_id),
            request.into_input()?,
            Utc::now(),
        )
        .await?;
    Ok(Json(ProjectDto::from(project)))
}

async fn delete_project(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(project_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .delete_project(auth.user_id, ProjectId::from_uuid(project_id), Utc::now())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_pipeline(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(project_id): Path<Uuid>,
    Json(request): Json<PipelineRequest>,
) -> Result<(StatusCode, Json<PipelineDto>), ApiError> {
    let pipeline = state
        .catalog
        .create_pipeline(
            auth.user_id,
            ProjectId::from_uuid(project_id),
            PipelineInput {
                name: request.name,
                stages: request.stages,
            },
            Utc::now(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(PipelineDto::from(pipeline))))
}

async fn update_pipeline(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(pipeline_id): Path<Uuid>,
    Json(request): Json<PipelineRequest>,
) -> Result<Json<PipelineDto>, ApiError> {
    // 阶段修改未纳入本期语义；显式拒绝而非静默丢弃。
    if request.stages.is_some() {
        return Err(ApiError::validation_fields(
            "pipeline stages cannot be modified",
            vec!["stages".to_owned()],
        ));
    }
    let pipeline = state
        .catalog
        .update_pipeline(
            auth.user_id,
            PipelineId::from_uuid(pipeline_id),
            &request.name,
            Utc::now(),
        )
        .await?;
    Ok(Json(PipelineDto::from(pipeline)))
}

async fn delete_pipeline(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(pipeline_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .delete_pipeline(auth.user_id, PipelineId::from_uuid(pipeline_id), Utc::now())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_job(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(project_id): Path<Uuid>,
    Json(request): Json<JobRequest>,
) -> Result<(StatusCode, Json<JobDto>), ApiError> {
    let job = state
        .catalog
        .create_job(
            auth.user_id,
            ProjectId::from_uuid(project_id),
            request.into_input()?,
            Utc::now(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(JobDto::from(job))))
}

async fn update_job(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(job_id): Path<Uuid>,
    Json(request): Json<JobPatchRequest>,
) -> Result<Json<JobDto>, ApiError> {
    let job = state
        .catalog
        .update_job(
            auth.user_id,
            JobId::from_uuid(job_id),
            request.into_input()?,
            Utc::now(),
        )
        .await?;
    Ok(Json(JobDto::from(job)))
}

async fn delete_job(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(job_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .delete_job(auth.user_id, JobId::from_uuid(job_id), Utc::now())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 导入 dry-run：只校验并返回全部错误；与创建共用同一校验函数。
/// 校验本身是纯函数，但登录态仍是本路由组的统一前提（plan/01 §7.3）。
async fn validate_template(
    _auth: AuthContext,
    Json(body): Json<Value>,
) -> Result<Json<TemplateValidateDto>, ApiError> {
    match CatalogService::validate_template(&body) {
        Ok(validated) => Ok(Json(TemplateValidateDto {
            ok: true,
            preview: Some(TemplatePreviewDto {
                name: validated.company.name,
                industry: validated.company.industry,
                project_count: validated.projects.len(),
                pipeline_count: validated.pipelines.len(),
                job_count: validated.jobs.len(),
            }),
            errors: None,
        })),
        Err(errors) => Ok(Json(TemplateValidateDto {
            ok: false,
            preview: None,
            errors: Some(
                errors
                    .into_iter()
                    .map(|issue| TemplateIssueDto {
                        path: issue.path,
                        message: issue.message,
                    })
                    .collect(),
            ),
        })),
    }
}

async fn import_template(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<ImportCreatedDto>), ApiError> {
    let company_id = state
        .catalog
        .import_template(auth.user_id, &body, Utc::now())
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ImportCreatedDto {
            company_id: company_id.into_uuid().to_string(),
        }),
    ))
}
