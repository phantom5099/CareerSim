//! catalog 端口的 PostgreSQL 适配：companies、recruitment_projects、
//! pipelines、pipeline_stages、jobs 五张表（plan/01 §5.2）。
//!
//! 读路径统一施加可见性条件（种子、公开或本人）；写路径统一要求
//! `owner_user_id = viewer`（种子行 owner 为 NULL，天然被排除）。
//! 不可见与不存在一致返回 `NotFound`，不泄露存在性。

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{Executor, PgPool, Postgres, Row, postgres::PgRow};

use super::model::{
    CatalogError, Company, CompanyDetail, CompanyId, CompanyPatch, CompanySource, CompanySummary,
    Job, JobId, JobPatch, JobStatus, JobType, ListCursor, NewCompany, NewJob, NewPipeline,
    NewProject, Pipeline, PipelineId, ProjectId, ProjectPatch, ProjectStatus, RecruitmentProject,
    normalize_key,
};
use super::store::{CatalogScope, CatalogStore};
use super::template::ValidatedTemplate;
use crate::account::UserId;
use crate::pipeline::{PipelineDefinition, StageDefinition};

pub(crate) struct PgCatalogStore {
    pool: PgPool,
}

impl PgCatalogStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// 动态查询一律编译期定稿：下列 SQL 只拼接静态文本与绑定参数，
/// 满足 sqlx 0.9 的 `SqlSafeStr` 审计要求。
///
/// 公司列表；三个可见性谓词分别对应 Mine / Public / All。
/// 岗位计数按可见性过滤：归属者见全部状态，其他人仅见 open。
/// keyset 分页：`(updated_at, id)` 行比较 + 严格小于，与 DESC 排序配套。
const LIST_COMPANIES_MINE_SQL: &str = "SELECT c.id, c.owner_user_id, c.is_seed, c.is_private, \
     c.name, c.english_name, c.industry, c.city, c.size, c.tagline, c.description, c.highlights, \
     c.logo_text, c.accent, c.created_at, c.updated_at, \
     COUNT(DISTINCT p.id) AS project_count, COUNT(DISTINCT j.id) AS visible_job_count \
     FROM companies c \
     LEFT JOIN recruitment_projects p ON p.company_id = c.id AND p.deleted_at IS NULL \
     LEFT JOIN jobs j ON j.project_id = p.id AND j.deleted_at IS NULL \
       AND (c.owner_user_id = $1 OR j.status = 'open') \
     WHERE c.deleted_at IS NULL AND c.owner_user_id = $1 \
       AND ($2::timestamptz IS NULL OR (c.updated_at, c.id) < ($2::timestamptz, $3::uuid)) \
     GROUP BY c.id ORDER BY c.updated_at DESC, c.id DESC LIMIT $4";
const LIST_COMPANIES_PUBLIC_SQL: &str = "SELECT c.id, c.owner_user_id, c.is_seed, c.is_private, \
     c.name, c.english_name, c.industry, c.city, c.size, c.tagline, c.description, c.highlights, \
     c.logo_text, c.accent, c.created_at, c.updated_at, \
     COUNT(DISTINCT p.id) AS project_count, COUNT(DISTINCT j.id) AS visible_job_count \
     FROM companies c \
     LEFT JOIN recruitment_projects p ON p.company_id = c.id AND p.deleted_at IS NULL \
     LEFT JOIN jobs j ON j.project_id = p.id AND j.deleted_at IS NULL \
       AND (c.owner_user_id = $1 OR j.status = 'open') \
     WHERE c.deleted_at IS NULL AND (c.is_seed OR NOT c.is_private) \
       AND ($2::timestamptz IS NULL OR (c.updated_at, c.id) < ($2::timestamptz, $3::uuid)) \
     GROUP BY c.id ORDER BY c.updated_at DESC, c.id DESC LIMIT $4";
const LIST_COMPANIES_ALL_SQL: &str = "SELECT c.id, c.owner_user_id, c.is_seed, c.is_private, \
     c.name, c.english_name, c.industry, c.city, c.size, c.tagline, c.description, c.highlights, \
     c.logo_text, c.accent, c.created_at, c.updated_at, \
     COUNT(DISTINCT p.id) AS project_count, COUNT(DISTINCT j.id) AS visible_job_count \
     FROM companies c \
     LEFT JOIN recruitment_projects p ON p.company_id = c.id AND p.deleted_at IS NULL \
     LEFT JOIN jobs j ON j.project_id = p.id AND j.deleted_at IS NULL \
       AND (c.owner_user_id = $1 OR j.status = 'open') \
     WHERE c.deleted_at IS NULL AND (c.is_seed OR NOT c.is_private OR c.owner_user_id = $1) \
       AND ($2::timestamptz IS NULL OR (c.updated_at, c.id) < ($2::timestamptz, $3::uuid)) \
     GROUP BY c.id ORDER BY c.updated_at DESC, c.id DESC LIMIT $4";

/// 公司详情：可见性（种子、公开或本人）统一施加在公司行上。
const DETAIL_COMPANY_SQL: &str = "SELECT c.id, c.owner_user_id, c.is_seed, c.is_private, c.name, \
     c.english_name, c.industry, c.city, c.size, c.tagline, c.description, c.highlights, \
     c.logo_text, c.accent, c.created_at, c.updated_at \
     FROM companies c \
     WHERE c.id = $1 AND c.deleted_at IS NULL \
       AND (c.is_seed OR NOT c.is_private OR c.owner_user_id = $2)";
const DETAIL_PROJECTS_SQL: &str = "SELECT p.id, p.company_id, p.name, p.season, p.status, \
     p.deadline, p.description, p.created_at, p.updated_at \
     FROM recruitment_projects p \
     WHERE p.company_id = $1 AND p.deleted_at IS NULL ORDER BY p.created_at, p.id";
const DETAIL_PIPELINES_SQL: &str = "SELECT pl.id, pl.project_id, pl.name, pl.created_at, \
     pl.updated_at \
     FROM pipelines pl \
     JOIN recruitment_projects p ON p.id = pl.project_id AND p.deleted_at IS NULL \
     WHERE p.company_id = $1 AND pl.deleted_at IS NULL ORDER BY pl.created_at, pl.id";
/// 岗位可见性：归属者见全部状态，其他人仅见 open。
const DETAIL_JOBS_OWNER_SQL: &str = "SELECT j.id, j.project_id, j.pipeline_id, j.title, j.city, \
     j.job_type, j.salary_min, j.salary_max, j.salary_months, j.tags, j.description, j.status, \
     j.created_at, j.updated_at \
     FROM jobs j \
     JOIN recruitment_projects p ON p.id = j.project_id AND p.deleted_at IS NULL \
     WHERE p.company_id = $1 AND j.deleted_at IS NULL ORDER BY j.created_at, j.id";
const DETAIL_JOBS_PUBLIC_SQL: &str = "SELECT j.id, j.project_id, j.pipeline_id, j.title, j.city, \
     j.job_type, j.salary_min, j.salary_max, j.salary_months, j.tags, j.description, j.status, \
     j.created_at, j.updated_at \
     FROM jobs j \
     JOIN recruitment_projects p ON p.id = j.project_id AND p.deleted_at IS NULL \
     WHERE p.company_id = $1 AND j.deleted_at IS NULL AND j.status = 'open' \
     ORDER BY j.created_at, j.id";

/// 公司部分更新；`COALESCE` 实现未提交字段保持原值。
const UPDATE_COMPANY_SQL: &str = "UPDATE companies SET \
     name = COALESCE($3, name), \
     name_key = COALESCE($4, name_key), \
     english_name = COALESCE($5, english_name), \
     industry = COALESCE($6, industry), \
     city = COALESCE($7, city), \
     size = COALESCE($8, size), \
     tagline = COALESCE($9, tagline), \
     description = COALESCE($10, description), \
     highlights = COALESCE($11, highlights), \
     logo_text = COALESCE($12, logo_text), \
     accent = COALESCE($13, accent), \
     is_private = COALESCE($14, is_private), \
     updated_at = $2 \
     WHERE id = $1 AND owner_user_id = $15 AND deleted_at IS NULL \
     RETURNING id, owner_user_id, is_seed, is_private, name, english_name, industry, city, \
     size, tagline, description, highlights, logo_text, accent, created_at, updated_at";

/// 项目部分更新；截止日期用 `value + clear` 参数对表达「不修改 / 清空 / 置值」。
const UPDATE_PROJECT_SQL: &str = "UPDATE recruitment_projects p SET \
     name = COALESCE($3, p.name), \
     name_key = COALESCE($4, p.name_key), \
     season = COALESCE($5, p.season), \
     status = COALESCE($6::text, p.status), \
     deadline = CASE WHEN $7::bool THEN NULL ELSE COALESCE($8, p.deadline) END, \
     description = COALESCE($9, p.description), \
     updated_at = $2 \
     FROM companies c \
     WHERE p.id = $1 AND p.deleted_at IS NULL AND c.id = p.company_id \
       AND c.owner_user_id = $10 AND c.deleted_at IS NULL \
     RETURNING p.id, p.company_id, p.name, p.season, p.status, p.deadline, p.description, \
     p.created_at, p.updated_at";

const UPDATE_PIPELINE_SQL: &str = "UPDATE pipelines pl SET name = $3, name_key = $4, \
     updated_at = $2 \
     FROM recruitment_projects p \
     JOIN companies c ON c.id = p.company_id \
     WHERE pl.id = $1 AND pl.project_id = p.id AND pl.deleted_at IS NULL \
       AND c.owner_user_id = $5 AND c.deleted_at IS NULL \
     RETURNING pl.id, pl.project_id, pl.name, pl.created_at, pl.updated_at";

/// 岗位部分更新；三个薪资字段都用 `value + clear` 参数对。
const UPDATE_JOB_SQL: &str = "UPDATE jobs j SET \
     pipeline_id = COALESCE($3, j.pipeline_id), \
     title = COALESCE($4, j.title), \
     title_key = COALESCE($5, j.title_key), \
     city = COALESCE($6, j.city), \
     job_type = COALESCE($7::text, j.job_type), \
     salary_min = CASE WHEN $8::bool THEN NULL ELSE COALESCE($9, j.salary_min) END, \
     salary_max = CASE WHEN $10::bool THEN NULL ELSE COALESCE($11, j.salary_max) END, \
     salary_months = CASE WHEN $12::bool THEN NULL ELSE COALESCE($13, j.salary_months) END, \
     tags = COALESCE($14, j.tags), \
     description = COALESCE($15, j.description), \
     status = COALESCE($16::text, j.status), \
     updated_at = $2 \
     FROM recruitment_projects p \
     JOIN companies c ON c.id = p.company_id AND c.deleted_at IS NULL \
     WHERE j.id = $1 AND j.deleted_at IS NULL AND j.project_id = p.id \
       AND c.owner_user_id = $17 \
     RETURNING j.id, j.project_id, j.pipeline_id, j.title, j.city, j.job_type, j.salary_min, \
     j.salary_max, j.salary_months, j.tags, j.description, j.status, j.created_at, j.updated_at";

fn backend_error(operation: &str, err: sqlx::Error) -> CatalogError {
    CatalogError::Backend {
        message: operation.to_owned(),
        source: Some(Box::new(err)),
    }
}

/// 数据库行违反领域不变量（CHECK 约束兜底应拦住，出现即为数据损坏）。
fn corrupted_row(what: &str) -> CatalogError {
    CatalogError::Backend {
        message: format!("stored {what} violates catalog invariants"),
        source: None,
    }
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db) if db.is_unique_violation())
}

fn is_foreign_key_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db) if db.is_foreign_key_violation())
}

/// CHECK 约束（23514）兜底被触发：PATCH 允许单侧更新字段，
/// 新值与库内旧值组合可能越过 CHECK；应作为 422 返回而非 500。
fn check_violation_constraint(err: &sqlx::Error) -> Option<&str> {
    match err {
        sqlx::Error::Database(db) if db.is_check_violation() => db.constraint(),
        _ => None,
    }
}

/// 插入/更新错误映射：唯一冲突转 `Conflict`，其余视为后端错误。
fn write_error(err: sqlx::Error, conflict: Option<&'static str>, operation: &str) -> CatalogError {
    match (is_unique_violation(&err), conflict) {
        (true, Some(field)) => CatalogError::Conflict(field),
        _ => backend_error(operation, err),
    }
}

/// 岗位写入错误映射：标题唯一冲突与流水线复合外键单独识别。
fn job_write_error(err: sqlx::Error, operation: &str) -> CatalogError {
    if is_unique_violation(&err) {
        return CatalogError::Conflict("title");
    }
    if is_foreign_key_violation(&err) {
        return CatalogError::InvalidReference {
            field: "pipelineId",
        };
    }
    if let Some(constraint) = check_violation_constraint(&err) {
        // 约束名形如 jobs_salary_nonneg / jobs_salary_months / jobs_tags_len；
        // 服务层已校验过其余字段，这里实际可触达的是 PATCH 单侧薪资组合。
        let field = if constraint.contains("salary") {
            "salaryMax"
        } else {
            "payload"
        };
        return CatalogError::InvalidInput {
            field,
            reason: format!("value violates database constraint `{constraint}`"),
        };
    }
    backend_error(operation, err)
}

fn column<T>(row: &PgRow, name: &str) -> Result<T, CatalogError>
where
    T: for<'c> sqlx::Decode<'c, Postgres> + sqlx::Type<Postgres>,
{
    row.try_get::<T, _>(name)
        .map_err(|err| backend_error("read row", err))
}

fn count_column(row: &PgRow, name: &str) -> Result<u32, CatalogError> {
    let raw: i64 = column(row, name)?;
    u32::try_from(raw).map_err(|_| corrupted_row(name))
}

fn salary_i32(raw: Option<u32>) -> Result<Option<i32>, CatalogError> {
    raw.map(|v| i32::try_from(v).map_err(|_| corrupted_row("salary")))
        .transpose()
}

fn company_from_row(row: &PgRow) -> Result<Company, CatalogError> {
    let id: uuid::Uuid = column(row, "id")?;
    let owner: Option<uuid::Uuid> = column(row, "owner_user_id")?;
    let is_seed: bool = column(row, "is_seed")?;
    let highlights: Vec<String> = column(row, "highlights")?;
    Ok(Company {
        id: CompanyId::from_uuid(id),
        source: if is_seed {
            CompanySource::Seed
        } else {
            CompanySource::User
        },
        owner: owner.map(UserId::from_uuid),
        is_private: column(row, "is_private")?,
        name: column(row, "name")?,
        english_name: column(row, "english_name")?,
        industry: column(row, "industry")?,
        city: column(row, "city")?,
        size: column(row, "size")?,
        tagline: column(row, "tagline")?,
        description: column(row, "description")?,
        highlights,
        logo_text: column(row, "logo_text")?,
        accent: column(row, "accent")?,
        created_at: column(row, "created_at")?,
        updated_at: column(row, "updated_at")?,
    })
}

fn project_from_row(row: &PgRow) -> Result<RecruitmentProject, CatalogError> {
    let id: uuid::Uuid = column(row, "id")?;
    let company_id: uuid::Uuid = column(row, "company_id")?;
    let status_raw: String = column(row, "status")?;
    let status =
        ProjectStatus::parse(&status_raw).ok_or_else(|| corrupted_row("project status"))?;
    Ok(RecruitmentProject {
        id: ProjectId::from_uuid(id),
        company_id: CompanyId::from_uuid(company_id),
        name: column(row, "name")?,
        season: column(row, "season")?,
        status,
        deadline: column(row, "deadline")?,
        description: column(row, "description")?,
        created_at: column(row, "created_at")?,
        updated_at: column(row, "updated_at")?,
    })
}

fn job_from_row(row: &PgRow) -> Result<Job, CatalogError> {
    let id: uuid::Uuid = column(row, "id")?;
    let project_id: uuid::Uuid = column(row, "project_id")?;
    let pipeline_id: uuid::Uuid = column(row, "pipeline_id")?;
    let job_type_raw: String = column(row, "job_type")?;
    let status_raw: String = column(row, "status")?;
    let job_type = JobType::parse(&job_type_raw).ok_or_else(|| corrupted_row("job type"))?;
    let status = JobStatus::parse(&status_raw).ok_or_else(|| corrupted_row("job status"))?;
    let salary_min: Option<i32> = column(row, "salary_min")?;
    let salary_max: Option<i32> = column(row, "salary_max")?;
    let salary_months: Option<i16> = column(row, "salary_months")?;
    let tags: Vec<String> = column(row, "tags")?;
    Ok(Job {
        id: JobId::from_uuid(id),
        project_id: ProjectId::from_uuid(project_id),
        pipeline_id: PipelineId::from_uuid(pipeline_id),
        title: column(row, "title")?,
        city: column(row, "city")?,
        job_type,
        salary_min: salary_min
            .map(u32::try_from)
            .transpose()
            .map_err(|_| corrupted_row("salary_min"))?,
        salary_max: salary_max
            .map(u32::try_from)
            .transpose()
            .map_err(|_| corrupted_row("salary_max"))?,
        salary_months: salary_months
            .map(u8::try_from)
            .transpose()
            .map_err(|_| corrupted_row("salary_months"))?,
        tags,
        description: column(row, "description")?,
        status,
        created_at: column(row, "created_at")?,
        updated_at: column(row, "updated_at")?,
    })
}

/// 从阶段行重建 [`StageDefinition`]：kind/rubric 的 serde 映射是唯一事实来源。
fn stage_from_row(row: &PgRow) -> Result<StageDefinition, CatalogError> {
    let stage_key: String = column(row, "stage_key")?;
    let kind: String = column(row, "kind")?;
    let title: String = column(row, "title")?;
    let rubric: serde_json::Value = column(row, "rubric")?;
    let min_rounds: i16 = column(row, "min_rounds")?;
    let max_rounds: i16 = column(row, "max_rounds")?;
    let value = serde_json::json!({
        "key": stage_key,
        "kind": kind,
        "title": title,
        "minRounds": min_rounds,
        "maxRounds": max_rounds,
        "rubric": rubric,
    });
    serde_json::from_value(value).map_err(|err| CatalogError::Backend {
        message: "stored stage violates invariants".to_owned(),
        source: Some(Box::new(err)),
    })
}

#[allow(clippy::too_many_arguments)]
async fn insert_company_row<'e, E>(
    executor: E,
    id: CompanyId,
    owner: Option<UserId>,
    is_private: bool,
    name: &str,
    english_name: &str,
    industry: &str,
    city: &str,
    size: &str,
    tagline: &str,
    description: &str,
    highlights: &[String],
    logo_text: &str,
    accent: &str,
    now: DateTime<Utc>,
) -> Result<(), CatalogError>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "INSERT INTO companies (id, owner_user_id, is_seed, is_private, name, name_key, \
         english_name, industry, city, size, tagline, description, highlights, logo_text, \
         accent, created_at, updated_at) \
         VALUES ($1, $2, false, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $15)",
    )
    .bind(id.into_uuid())
    .bind(owner.map(UserId::into_uuid))
    .bind(is_private)
    .bind(name)
    .bind(normalize_key(name))
    .bind(english_name)
    .bind(industry)
    .bind(city)
    .bind(size)
    .bind(tagline)
    .bind(description)
    .bind(highlights.to_vec())
    .bind(logo_text)
    .bind(accent)
    .bind(now)
    .execute(executor)
    .await
    .map_err(|err| write_error(err, Some("name"), "insert company"))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_project_row<'e, E>(
    executor: E,
    id: ProjectId,
    company_id: CompanyId,
    name: &str,
    season: &str,
    status: ProjectStatus,
    deadline: Option<NaiveDate>,
    description: &str,
    now: DateTime<Utc>,
) -> Result<(), CatalogError>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "INSERT INTO recruitment_projects \
         (id, company_id, name, name_key, season, status, deadline, description, \
         created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)",
    )
    .bind(id.into_uuid())
    .bind(company_id.into_uuid())
    .bind(name)
    .bind(normalize_key(name))
    .bind(season)
    .bind(status.as_str())
    .bind(deadline)
    .bind(description)
    .bind(now)
    .execute(executor)
    .await
    .map_err(|err| write_error(err, Some("name"), "insert project"))?;
    Ok(())
}

async fn insert_pipeline_row<'e, E>(
    executor: E,
    id: PipelineId,
    project_id: ProjectId,
    name: &str,
    now: DateTime<Utc>,
) -> Result<(), CatalogError>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "INSERT INTO pipelines (id, project_id, name, name_key, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $5)",
    )
    .bind(id.into_uuid())
    .bind(project_id.into_uuid())
    .bind(name)
    .bind(normalize_key(name))
    .bind(now)
    .execute(executor)
    .await
    .map_err(|err| write_error(err, Some("name"), "insert pipeline"))?;
    Ok(())
}

/// 批量写阶段：单条 UNNEST 语句，字段与 `pipeline_stages` 列一一对应。
async fn insert_stages<'e, E>(
    executor: E,
    pipeline_id: PipelineId,
    definition: &PipelineDefinition,
) -> Result<(), CatalogError>
where
    E: Executor<'e, Database = Postgres>,
{
    let stages = definition.stages();
    let mut keys = Vec::with_capacity(stages.len());
    let mut positions = Vec::with_capacity(stages.len());
    let mut kinds = Vec::with_capacity(stages.len());
    let mut titles = Vec::with_capacity(stages.len());
    let mut rubrics = Vec::with_capacity(stages.len());
    let mut min_rounds = Vec::with_capacity(stages.len());
    let mut max_rounds = Vec::with_capacity(stages.len());
    for (index, stage) in stages.iter().enumerate() {
        keys.push(stage.key.as_str().to_owned());
        positions.push(i16::try_from(index).map_err(|_| corrupted_row("stage position"))?);
        kinds.push(stage.kind.as_str().to_owned());
        titles.push(stage.title.clone());
        rubrics.push(
            serde_json::to_value(&stage.rubric).map_err(|err| CatalogError::Backend {
                message: "serialize rubric".to_owned(),
                source: Some(Box::new(err)),
            })?,
        );
        min_rounds.push(i16::from(stage.min_rounds));
        max_rounds.push(i16::from(stage.max_rounds));
    }
    sqlx::query(
        "INSERT INTO pipeline_stages \
         (pipeline_id, stage_key, position, kind, title, rubric, min_rounds, max_rounds) \
         SELECT $1, key, position, kind, title, rubric, min_rounds, max_rounds \
         FROM UNNEST($2::text[], $3::int2[], $4::text[], $5::text[], $6::jsonb[], $7::int2[], \
                      $8::int2[]) \
              AS t(key, position, kind, title, rubric, min_rounds, max_rounds)",
    )
    .bind(pipeline_id.into_uuid())
    .bind(&keys)
    .bind(&positions)
    .bind(&kinds)
    .bind(&titles)
    .bind(&rubrics)
    .bind(&min_rounds)
    .bind(&max_rounds)
    .execute(executor)
    .await
    .map_err(|err| backend_error("insert stages", err))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_job_row<'e, E>(
    executor: E,
    id: JobId,
    project_id: ProjectId,
    pipeline_id: PipelineId,
    title: &str,
    city: &str,
    job_type: JobType,
    salary_min: Option<u32>,
    salary_max: Option<u32>,
    salary_months: Option<u8>,
    tags: &[String],
    description: &str,
    status: JobStatus,
    now: DateTime<Utc>,
) -> Result<(), CatalogError>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "INSERT INTO jobs \
         (id, project_id, pipeline_id, title, title_key, city, job_type, salary_min, \
         salary_max, salary_months, tags, description, status, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $14)",
    )
    .bind(id.into_uuid())
    .bind(project_id.into_uuid())
    .bind(pipeline_id.into_uuid())
    .bind(title)
    .bind(normalize_key(title))
    .bind(city)
    .bind(job_type.as_str())
    .bind(salary_i32(salary_min)?)
    .bind(salary_i32(salary_max)?)
    .bind(salary_months.map(i16::from))
    .bind(tags.to_vec())
    .bind(description)
    .bind(status.as_str())
    .bind(now)
    .execute(executor)
    .await
    .map_err(|err| job_write_error(err, "insert job"))?;
    Ok(())
}

impl PgCatalogStore {
    /// 读取指定流水线的完整定义；存储行必须能重建出通过 `parse` 的定义。
    async fn load_definition(
        &self,
        pipeline_id: PipelineId,
    ) -> Result<PipelineDefinition, CatalogError> {
        let rows = sqlx::query(
            "SELECT stage_key, position, kind, title, rubric, min_rounds, max_rounds \
             FROM pipeline_stages WHERE pipeline_id = $1 ORDER BY position",
        )
        .bind(pipeline_id.into_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|err| backend_error("load stages", err))?;
        let stages: Vec<StageDefinition> =
            rows.iter().map(stage_from_row).collect::<Result<_, _>>()?;
        PipelineDefinition::parse(stages).map_err(|err| CatalogError::Backend {
            message: format!("stored pipeline violates invariants: {err}"),
            source: None,
        })
    }

    /// 校验公司归属：不存在、他人所有或已删除一律 `NotFound`。
    async fn ensure_company_owned(
        &self,
        owner: UserId,
        company_id: CompanyId,
    ) -> Result<(), CatalogError> {
        let found = sqlx::query(
            "SELECT 1 AS ok FROM companies \
             WHERE id = $1 AND owner_user_id = $2 AND deleted_at IS NULL",
        )
        .bind(company_id.into_uuid())
        .bind(owner.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| backend_error("check company ownership", err))?;
        if found.is_none() {
            return Err(CatalogError::NotFound);
        }
        Ok(())
    }

    /// 校验项目归属：经由所属公司的 owner 判断。
    async fn ensure_project_owned(
        &self,
        owner: UserId,
        project_id: ProjectId,
    ) -> Result<(), CatalogError> {
        let found = sqlx::query(
            "SELECT 1 AS ok FROM recruitment_projects p \
             JOIN companies c ON c.id = p.company_id \
             WHERE p.id = $1 AND c.owner_user_id = $2 \
               AND p.deleted_at IS NULL AND c.deleted_at IS NULL",
        )
        .bind(project_id.into_uuid())
        .bind(owner.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| backend_error("check project ownership", err))?;
        if found.is_none() {
            return Err(CatalogError::NotFound);
        }
        Ok(())
    }

    /// 校验流水线：存在、未删除且属于 `project_id`；否则引用无效。
    async fn ensure_pipeline_in_project(
        &self,
        owner: UserId,
        project_id: ProjectId,
        pipeline_id: PipelineId,
    ) -> Result<(), CatalogError> {
        let found = sqlx::query(
            "SELECT 1 AS ok FROM pipelines pl \
             JOIN recruitment_projects p ON p.id = pl.project_id \
             JOIN companies c ON c.id = p.company_id \
             WHERE pl.id = $1 AND p.id = $2 AND c.owner_user_id = $3 \
               AND pl.deleted_at IS NULL AND p.deleted_at IS NULL AND c.deleted_at IS NULL",
        )
        .bind(pipeline_id.into_uuid())
        .bind(project_id.into_uuid())
        .bind(owner.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| backend_error("check pipeline", err))?;
        if found.is_none() {
            return Err(CatalogError::InvalidReference {
                field: "pipelineId",
            });
        }
        Ok(())
    }
}

#[async_trait]
impl CatalogStore for PgCatalogStore {
    async fn list_companies(
        &self,
        viewer: UserId,
        scope: CatalogScope,
        limit: u32,
        cursor: Option<ListCursor>,
    ) -> Result<(Vec<CompanySummary>, Option<ListCursor>), CatalogError> {
        let sql = match scope {
            CatalogScope::Mine => LIST_COMPANIES_MINE_SQL,
            CatalogScope::Public => LIST_COMPANIES_PUBLIC_SQL,
            CatalogScope::All => LIST_COMPANIES_ALL_SQL,
        };
        let limit_usize = usize::try_from(limit).map_err(|_| corrupted_row("list limit"))?;
        let page_size = i64::from(limit) + 1;
        let rows = sqlx::query(sql)
            .bind(viewer.into_uuid())
            .bind(cursor.map(|c| c.updated_at))
            .bind(cursor.map(|c| c.id))
            .bind(page_size)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| backend_error("list companies", err))?;

        let has_more = rows.len() > limit_usize;
        let mut summaries = Vec::with_capacity(rows.len().min(limit_usize));
        for row in rows.iter().take(limit_usize) {
            summaries.push(CompanySummary {
                company: company_from_row(row)?,
                project_count: count_column(row, "project_count")?,
                visible_job_count: count_column(row, "visible_job_count")?,
            });
        }
        let next_cursor = if has_more {
            summaries.last().map(|summary| ListCursor {
                updated_at: summary.company.updated_at,
                id: summary.company.id.into_uuid(),
            })
        } else {
            None
        };
        Ok((summaries, next_cursor))
    }

    async fn get_company(
        &self,
        viewer: UserId,
        id: CompanyId,
    ) -> Result<Option<CompanyDetail>, CatalogError> {
        let row = sqlx::query(DETAIL_COMPANY_SQL)
            .bind(id.into_uuid())
            .bind(viewer.into_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| backend_error("get company", err))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let company = company_from_row(&row)?;

        let project_rows = sqlx::query(DETAIL_PROJECTS_SQL)
            .bind(id.into_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(|err| backend_error("list projects", err))?;
        let mut projects = Vec::with_capacity(project_rows.len());
        for row in &project_rows {
            projects.push(project_from_row(row)?);
        }

        let pipeline_rows = sqlx::query(DETAIL_PIPELINES_SQL)
            .bind(id.into_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(|err| backend_error("list pipelines", err))?;
        let mut pipelines = Vec::with_capacity(pipeline_rows.len());
        for row in &pipeline_rows {
            let pipeline_id: uuid::Uuid = column(row, "id")?;
            let definition = self
                .load_definition(PipelineId::from_uuid(pipeline_id))
                .await?;
            pipelines.push(Pipeline {
                id: PipelineId::from_uuid(pipeline_id),
                project_id: ProjectId::from_uuid(column(row, "project_id")?),
                name: column(row, "name")?,
                definition,
                created_at: column(row, "created_at")?,
                updated_at: column(row, "updated_at")?,
            });
        }

        // 岗位可见性：归属者见全部状态，其他人仅见 open。
        let job_sql = if company.owner == Some(viewer) {
            DETAIL_JOBS_OWNER_SQL
        } else {
            DETAIL_JOBS_PUBLIC_SQL
        };
        let job_rows = sqlx::query(job_sql)
            .bind(id.into_uuid())
            .fetch_all(&self.pool)
            .await
            .map_err(|err| backend_error("list jobs", err))?;
        let mut jobs = Vec::with_capacity(job_rows.len());
        for row in &job_rows {
            jobs.push(job_from_row(row)?);
        }

        Ok(Some(CompanyDetail {
            company,
            projects,
            pipelines,
            jobs,
        }))
    }

    async fn create_company(
        &self,
        owner: UserId,
        new: NewCompany,
        now: DateTime<Utc>,
    ) -> Result<Company, CatalogError> {
        let id = CompanyId::generate();
        insert_company_row(
            &self.pool,
            id,
            Some(owner),
            new.is_private,
            &new.name,
            &new.english_name,
            &new.industry,
            &new.city,
            &new.size,
            &new.tagline,
            &new.description,
            &new.highlights,
            &new.logo_text,
            &new.accent,
            now,
        )
        .await?;
        Ok(Company {
            id,
            source: CompanySource::User,
            owner: Some(owner),
            is_private: new.is_private,
            name: new.name,
            english_name: new.english_name,
            industry: new.industry,
            city: new.city,
            size: new.size,
            tagline: new.tagline,
            description: new.description,
            highlights: new.highlights,
            logo_text: new.logo_text,
            accent: new.accent,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_company(
        &self,
        owner: UserId,
        id: CompanyId,
        patch: &CompanyPatch,
        now: DateTime<Utc>,
    ) -> Result<Company, CatalogError> {
        let name_key = patch.name.as_deref().map(normalize_key);
        let row = sqlx::query(UPDATE_COMPANY_SQL)
            .bind(id.into_uuid())
            .bind(now)
            .bind(patch.name.clone())
            .bind(name_key)
            .bind(patch.english_name.clone())
            .bind(patch.industry.clone())
            .bind(patch.city.clone())
            .bind(patch.size.clone())
            .bind(patch.tagline.clone())
            .bind(patch.description.clone())
            .bind(patch.highlights.clone())
            .bind(patch.logo_text.clone())
            .bind(patch.accent.clone())
            .bind(patch.is_private)
            .bind(owner.into_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| write_error(err, Some("name"), "update company"))?;
        let Some(row) = row else {
            return Err(CatalogError::NotFound);
        };
        company_from_row(&row)
    }

    async fn soft_delete_company(
        &self,
        owner: UserId,
        id: CompanyId,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError> {
        let result = sqlx::query(
            "UPDATE companies SET deleted_at = $3, updated_at = $3 \
             WHERE id = $1 AND owner_user_id = $2 AND deleted_at IS NULL",
        )
        .bind(id.into_uuid())
        .bind(owner.into_uuid())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|err| backend_error("delete company", err))?;
        if result.rows_affected() == 0 {
            return Err(CatalogError::NotFound);
        }
        Ok(())
    }

    async fn create_project(
        &self,
        owner: UserId,
        company_id: CompanyId,
        new: NewProject,
        now: DateTime<Utc>,
    ) -> Result<RecruitmentProject, CatalogError> {
        self.ensure_company_owned(owner, company_id).await?;
        let id = ProjectId::generate();
        insert_project_row(
            &self.pool,
            id,
            company_id,
            &new.name,
            &new.season,
            new.status,
            new.deadline,
            &new.description,
            now,
        )
        .await?;
        Ok(RecruitmentProject {
            id,
            company_id,
            name: new.name,
            season: new.season,
            status: new.status,
            deadline: new.deadline,
            description: new.description,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_project(
        &self,
        owner: UserId,
        id: ProjectId,
        patch: &ProjectPatch,
        now: DateTime<Utc>,
    ) -> Result<RecruitmentProject, CatalogError> {
        let name_key = patch.name.as_deref().map(normalize_key);
        let clear_deadline = matches!(patch.deadline, Some(None));
        let deadline_value = patch.deadline.unwrap_or(None);
        let row = sqlx::query(UPDATE_PROJECT_SQL)
            .bind(id.into_uuid())
            .bind(now)
            .bind(patch.name.clone())
            .bind(name_key)
            .bind(patch.season.clone())
            .bind(patch.status.map(ProjectStatus::as_str))
            .bind(clear_deadline)
            .bind(deadline_value)
            .bind(patch.description.clone())
            .bind(owner.into_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| write_error(err, Some("name"), "update project"))?;
        let Some(row) = row else {
            return Err(CatalogError::NotFound);
        };
        project_from_row(&row)
    }

    async fn soft_delete_project(
        &self,
        owner: UserId,
        id: ProjectId,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError> {
        let result = sqlx::query(
            "UPDATE recruitment_projects p SET deleted_at = $3, updated_at = $3 \
             FROM companies c \
             WHERE p.id = $1 AND p.deleted_at IS NULL AND c.id = p.company_id \
               AND c.owner_user_id = $2 AND c.deleted_at IS NULL",
        )
        .bind(id.into_uuid())
        .bind(owner.into_uuid())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|err| backend_error("delete project", err))?;
        if result.rows_affected() == 0 {
            return Err(CatalogError::NotFound);
        }
        Ok(())
    }

    async fn create_pipeline(
        &self,
        owner: UserId,
        project_id: ProjectId,
        new: NewPipeline,
        now: DateTime<Utc>,
    ) -> Result<Pipeline, CatalogError> {
        self.ensure_project_owned(owner, project_id).await?;
        let id = PipelineId::generate();
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| backend_error("begin create pipeline", err))?;
        insert_pipeline_row(&mut *tx, id, project_id, &new.name, now).await?;
        insert_stages(&mut *tx, id, &new.definition).await?;
        tx.commit()
            .await
            .map_err(|err| backend_error("commit create pipeline", err))?;
        Ok(Pipeline {
            id,
            project_id,
            name: new.name,
            definition: new.definition,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_pipeline(
        &self,
        owner: UserId,
        id: PipelineId,
        name: &str,
        now: DateTime<Utc>,
    ) -> Result<Pipeline, CatalogError> {
        let row = sqlx::query(UPDATE_PIPELINE_SQL)
            .bind(id.into_uuid())
            .bind(now)
            .bind(name)
            .bind(normalize_key(name))
            .bind(owner.into_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| write_error(err, Some("name"), "update pipeline"))?;
        let Some(row) = row else {
            return Err(CatalogError::NotFound);
        };
        let pipeline_id: uuid::Uuid = column(&row, "id")?;
        let definition = self
            .load_definition(PipelineId::from_uuid(pipeline_id))
            .await?;
        Ok(Pipeline {
            id: PipelineId::from_uuid(pipeline_id),
            project_id: ProjectId::from_uuid(column(&row, "project_id")?),
            name: column(&row, "name")?,
            definition,
            created_at: column(&row, "created_at")?,
            updated_at: column(&row, "updated_at")?,
        })
    }

    async fn soft_delete_pipeline(
        &self,
        owner: UserId,
        id: PipelineId,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError> {
        let result = sqlx::query(
            "UPDATE pipelines pl SET deleted_at = $3, updated_at = $3 \
             FROM recruitment_projects p \
             JOIN companies c ON c.id = p.company_id \
             WHERE pl.id = $1 AND pl.project_id = p.id AND pl.deleted_at IS NULL \
               AND c.owner_user_id = $2 AND c.deleted_at IS NULL",
        )
        .bind(id.into_uuid())
        .bind(owner.into_uuid())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|err| backend_error("delete pipeline", err))?;
        if result.rows_affected() == 0 {
            return Err(CatalogError::NotFound);
        }
        Ok(())
    }

    async fn create_job(
        &self,
        owner: UserId,
        project_id: ProjectId,
        new: NewJob,
        now: DateTime<Utc>,
    ) -> Result<Job, CatalogError> {
        self.ensure_project_owned(owner, project_id).await?;
        self.ensure_pipeline_in_project(owner, project_id, new.pipeline_id)
            .await?;
        let id = JobId::generate();
        insert_job_row(
            &self.pool,
            id,
            project_id,
            new.pipeline_id,
            &new.title,
            &new.city,
            new.job_type,
            new.salary_min,
            new.salary_max,
            new.salary_months,
            &new.tags,
            &new.description,
            new.status,
            now,
        )
        .await?;
        Ok(Job {
            id,
            project_id,
            pipeline_id: new.pipeline_id,
            title: new.title,
            city: new.city,
            job_type: new.job_type,
            salary_min: new.salary_min,
            salary_max: new.salary_max,
            salary_months: new.salary_months,
            tags: new.tags,
            description: new.description,
            status: new.status,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_job(
        &self,
        owner: UserId,
        id: JobId,
        patch: &JobPatch,
        now: DateTime<Utc>,
    ) -> Result<Job, CatalogError> {
        let project_row = sqlx::query(
            "SELECT j.project_id FROM jobs j \
             JOIN recruitment_projects p ON p.id = j.project_id AND p.deleted_at IS NULL \
             JOIN companies c ON c.id = p.company_id AND c.deleted_at IS NULL \
             WHERE j.id = $1 AND j.deleted_at IS NULL AND c.owner_user_id = $2",
        )
        .bind(id.into_uuid())
        .bind(owner.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| backend_error("check job ownership", err))?;
        let Some(project_row) = project_row else {
            return Err(CatalogError::NotFound);
        };
        let project_id: uuid::Uuid = column(&project_row, "project_id")?;
        if let Some(pipeline_id) = patch.pipeline_id {
            self.ensure_pipeline_in_project(owner, ProjectId::from_uuid(project_id), pipeline_id)
                .await?;
        }

        let title_key = patch.title.as_deref().map(normalize_key);
        let salary_min_clear = matches!(patch.salary_min, Some(None));
        let salary_min_value = salary_i32(patch.salary_min.unwrap_or(None))?;
        let salary_max_clear = matches!(patch.salary_max, Some(None));
        let salary_max_value = salary_i32(patch.salary_max.unwrap_or(None))?;
        let salary_months_clear = matches!(patch.salary_months, Some(None));
        let salary_months_value = patch.salary_months.unwrap_or(None).map(i16::from);
        let row = sqlx::query(UPDATE_JOB_SQL)
            .bind(id.into_uuid())
            .bind(now)
            .bind(patch.pipeline_id.map(PipelineId::into_uuid))
            .bind(patch.title.clone())
            .bind(title_key)
            .bind(patch.city.clone())
            .bind(patch.job_type.map(JobType::as_str))
            .bind(salary_min_clear)
            .bind(salary_min_value)
            .bind(salary_max_clear)
            .bind(salary_max_value)
            .bind(salary_months_clear)
            .bind(salary_months_value)
            .bind(patch.tags.clone())
            .bind(patch.description.clone())
            .bind(patch.status.map(JobStatus::as_str))
            .bind(owner.into_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| job_write_error(err, "update job"))?;
        let Some(row) = row else {
            return Err(CatalogError::NotFound);
        };
        job_from_row(&row)
    }

    async fn soft_delete_job(
        &self,
        owner: UserId,
        id: JobId,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError> {
        let result = sqlx::query(
            "UPDATE jobs j SET deleted_at = $3, updated_at = $3 \
             FROM recruitment_projects p \
             JOIN companies c ON c.id = p.company_id \
             WHERE j.id = $1 AND j.project_id = p.id AND j.deleted_at IS NULL \
               AND c.owner_user_id = $2 AND c.deleted_at IS NULL",
        )
        .bind(id.into_uuid())
        .bind(owner.into_uuid())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|err| backend_error("delete job", err))?;
        if result.rows_affected() == 0 {
            return Err(CatalogError::NotFound);
        }
        Ok(())
    }

    async fn import_template(
        &self,
        owner: UserId,
        template: &ValidatedTemplate,
        now: DateTime<Utc>,
    ) -> Result<CompanyId, CatalogError> {
        // 单事务整体写入：任何一步失败即回滚，不产生半份模板。
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| backend_error("begin import", err))?;

        let company_id = CompanyId::generate();
        let company = &template.company;
        insert_company_row(
            &mut *tx,
            company_id,
            Some(owner),
            company.is_private,
            &company.name,
            &company.english_name,
            &company.industry,
            &company.city,
            &company.size,
            &company.tagline,
            &company.description,
            &company.highlights,
            &company.logo_text,
            &company.accent,
            now,
        )
        .await?;

        let mut project_ids = HashMap::with_capacity(template.projects.len());
        for project in &template.projects {
            let id = ProjectId::generate();
            insert_project_row(
                &mut *tx,
                id,
                company_id,
                &project.name,
                &project.season,
                project.status,
                project.deadline,
                &project.description,
                now,
            )
            .await?;
            project_ids.insert(project.key.clone(), id);
        }

        let mut pipeline_ids = HashMap::with_capacity(template.pipelines.len());
        for pipeline in &template.pipelines {
            let project_id = project_ids
                .get(&pipeline.project_key)
                .copied()
                .ok_or_else(|| corrupted_row("template pipeline project key"))?;
            let id = PipelineId::generate();
            insert_pipeline_row(&mut *tx, id, project_id, &pipeline.name, now).await?;
            insert_stages(&mut *tx, id, &pipeline.definition).await?;
            pipeline_ids.insert(pipeline.key.clone(), id);
        }

        for job in &template.jobs {
            let project_id = project_ids
                .get(&job.project_key)
                .copied()
                .ok_or_else(|| corrupted_row("template job project key"))?;
            let pipeline_id = pipeline_ids
                .get(&job.pipeline_key)
                .copied()
                .ok_or_else(|| corrupted_row("template job pipeline key"))?;
            insert_job_row(
                &mut *tx,
                JobId::generate(),
                project_id,
                pipeline_id,
                &job.title,
                &job.city,
                job.job_type,
                job.salary_min,
                job.salary_max,
                job.salary_months,
                &job.tags,
                &job.description,
                job.status,
                now,
            )
            .await?;
        }

        tx.commit()
            .await
            .map_err(|err| backend_error("commit import", err))?;
        Ok(company_id)
    }
}
