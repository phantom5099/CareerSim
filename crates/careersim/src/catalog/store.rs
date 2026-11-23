//! catalog 存储端口；PG 实现见 postgres.rs。
//!
//! 读查询统一施加可见性条件（种子或公开或本人）；写查询统一要求
//! `owner = viewer AND NOT is_seed`，越权与不存在一致返回 `NotFound`。

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};

use super::{
    model::{
        CatalogError, Company, CompanyDetail, CompanyId, CompanySummary, Job, JobId, JobStatus,
        JobType, ListCursor, Pipeline, PipelineId, ProjectId, ProjectStatus, RecruitmentProject,
    },
    template::ValidatedTemplate,
};
use crate::{account::model::UserId, pipeline::model::PipelineDefinition};

/// 手工创建或修改公司时的字段集合；`is_private` 只影响可见性，不影响归属。
#[derive(Debug, Clone, Default)]
pub struct CreateCompanyRecord {
    pub name: String,
    pub english_name: String,
    pub industry: String,
    pub city: String,
    pub size: String,
    pub tagline: String,
    pub description: String,
    pub highlights: Vec<String>,
    pub logo_text: String,
    pub accent: String,
    pub is_private: bool,
}

/// 公司部分更新；`None` 表示不修改。
#[derive(Debug, Clone, Default)]
pub struct UpdateCompanyRecord {
    pub name: Option<String>,
    pub english_name: Option<String>,
    pub industry: Option<String>,
    pub city: Option<String>,
    pub size: Option<String>,
    pub tagline: Option<String>,
    pub description: Option<String>,
    pub highlights: Option<Vec<String>>,
    pub logo_text: Option<String>,
    pub accent: Option<String>,
    pub is_private: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct CreateProjectRecord {
    pub name: String,
    pub season: String,
    pub status: ProjectStatus,
    pub deadline: Option<NaiveDate>,
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateProjectRecord {
    pub name: Option<String>,
    pub season: Option<String>,
    pub status: Option<ProjectStatus>,
    /// 外层 `None` 不修改，内层 `None` 清空截止日期。
    pub deadline: Option<Option<NaiveDate>>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreatePipelineRecord {
    pub name: String,
    pub definition: PipelineDefinition,
}

#[derive(Debug, Clone)]
pub struct CreateJobRecord {
    pub pipeline_id: PipelineId,
    pub title: String,
    pub city: String,
    pub job_type: JobType,
    pub salary_min: Option<u32>,
    pub salary_max: Option<u32>,
    pub salary_months: Option<u8>,
    pub tags: Vec<String>,
    pub description: String,
    pub status: JobStatus,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateJobRecord {
    pub pipeline_id: Option<PipelineId>,
    pub title: Option<String>,
    pub city: Option<String>,
    pub job_type: Option<JobType>,
    pub salary_min: Option<Option<u32>>,
    pub salary_max: Option<Option<u32>>,
    pub salary_months: Option<Option<u8>>,
    pub tags: Option<Vec<String>>,
    pub description: Option<String>,
    pub status: Option<JobStatus>,
}

/// 列表范围（plan/01 §4.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogScope {
    /// 本人创建的全部模板，含私有。
    Mine,
    /// 种子与全部公开模板；不含他人私有。
    Public,
    /// 种子、公开模板与本人模板，按行去重。
    All,
}

#[async_trait]
pub trait CatalogStore: Send + Sync {
    /// 返回一页公司摘要；`cursor` 为上一页末尾的游标。
    async fn list_companies(
        &self,
        viewer: UserId,
        scope: CatalogScope,
        limit: u32,
        cursor: Option<ListCursor>,
    ) -> Result<(Vec<CompanySummary>, Option<ListCursor>), CatalogError>;

    /// 读取公司详情；不可见（不存在、他人私有、已删除）返回 `None`。
    async fn get_company(
        &self,
        viewer: UserId,
        id: CompanyId,
    ) -> Result<Option<CompanyDetail>, CatalogError>;

    async fn create_company(
        &self,
        owner: UserId,
        new: CreateCompanyRecord,
        now: DateTime<Utc>,
    ) -> Result<Company, CatalogError>;

    async fn update_company(
        &self,
        owner: UserId,
        id: CompanyId,
        patch: &UpdateCompanyRecord,
        now: DateTime<Utc>,
    ) -> Result<Company, CatalogError>;

    async fn soft_delete_company(
        &self,
        owner: UserId,
        id: CompanyId,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError>;

    async fn create_project(
        &self,
        owner: UserId,
        company_id: CompanyId,
        new: CreateProjectRecord,
        now: DateTime<Utc>,
    ) -> Result<RecruitmentProject, CatalogError>;

    async fn update_project(
        &self,
        owner: UserId,
        id: ProjectId,
        patch: &UpdateProjectRecord,
        now: DateTime<Utc>,
    ) -> Result<RecruitmentProject, CatalogError>;

    async fn soft_delete_project(
        &self,
        owner: UserId,
        id: ProjectId,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError>;

    async fn create_pipeline(
        &self,
        owner: UserId,
        project_id: ProjectId,
        new: CreatePipelineRecord,
        now: DateTime<Utc>,
    ) -> Result<Pipeline, CatalogError>;

    async fn update_pipeline(
        &self,
        owner: UserId,
        id: PipelineId,
        name: &str,
        now: DateTime<Utc>,
    ) -> Result<Pipeline, CatalogError>;

    /// Replace a pipeline name and its complete stage definition atomically.
    async fn replace_pipeline(
        &self,
        owner: UserId,
        id: PipelineId,
        new: CreatePipelineRecord,
        now: DateTime<Utc>,
    ) -> Result<Pipeline, CatalogError>;

    async fn soft_delete_pipeline(
        &self,
        owner: UserId,
        id: PipelineId,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError>;

    async fn create_job(
        &self,
        owner: UserId,
        project_id: ProjectId,
        new: CreateJobRecord,
        now: DateTime<Utc>,
    ) -> Result<Job, CatalogError>;

    async fn update_job(
        &self,
        owner: UserId,
        id: JobId,
        patch: &UpdateJobRecord,
        now: DateTime<Utc>,
    ) -> Result<Job, CatalogError>;

    async fn soft_delete_job(
        &self,
        owner: UserId,
        id: JobId,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError>;

    /// 单事务写入整份模板；任何冲突整体回滚。
    async fn import_template(
        &self,
        owner: UserId,
        template: &ValidatedTemplate,
        now: DateTime<Utc>,
    ) -> Result<CompanyId, CatalogError>;
}
