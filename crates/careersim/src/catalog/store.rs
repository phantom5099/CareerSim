//! catalog 存储端口；PG 实现见 postgres.rs。
//!
//! 读查询统一施加可见性条件（种子或公开或本人）；写查询统一要求
//! `owner = viewer AND NOT is_seed`，越权与不存在一致返回 `NotFound`。

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::model::{
    CatalogError, Company, CompanyDetail, CompanyId, CompanyPatch, CompanySummary, Job, JobId,
    JobPatch, ListCursor, NewCompany, NewJob, NewPipeline, NewProject, Pipeline, PipelineId,
    ProjectId, ProjectPatch, RecruitmentProject,
};
use super::template::ValidatedTemplate;
use crate::account::UserId;

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
        new: NewCompany,
        now: DateTime<Utc>,
    ) -> Result<Company, CatalogError>;

    async fn update_company(
        &self,
        owner: UserId,
        id: CompanyId,
        patch: &CompanyPatch,
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
        new: NewProject,
        now: DateTime<Utc>,
    ) -> Result<RecruitmentProject, CatalogError>;

    async fn update_project(
        &self,
        owner: UserId,
        id: ProjectId,
        patch: &ProjectPatch,
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
        new: NewPipeline,
        now: DateTime<Utc>,
    ) -> Result<Pipeline, CatalogError>;

    async fn update_pipeline(
        &self,
        owner: UserId,
        id: PipelineId,
        name: &str,
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
        new: NewJob,
        now: DateTime<Utc>,
    ) -> Result<Job, CatalogError>;

    async fn update_job(
        &self,
        owner: UserId,
        id: JobId,
        patch: &JobPatch,
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
