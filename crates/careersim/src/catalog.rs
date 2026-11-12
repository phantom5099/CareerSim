//! 公司模板目录（catalog）有界上下文：领域模型、`CompanyTemplateV1` 校验、
//! 存储端口与 PostgreSQL 适配（plan/01 §3.3、§4.3、§6.2）。

pub mod model;
pub(crate) mod postgres;
pub mod service;
pub mod store;
pub mod template;

pub use model::{
    CatalogError, Company, CompanyDetail, CompanyId, CompanyPatch, CompanySource, CompanySummary,
    Job, JobId, JobPatch, JobStatus, JobType, ListCursor, NewCompany, NewJob, NewPipeline,
    NewProject, Pipeline, PipelineId, ProjectId, ProjectPatch, ProjectStatus, RecruitmentProject,
};
pub use service::CatalogService;
pub use store::CatalogScope;
pub use template::{TemplateError, ValidatedTemplate};

/// 组装 PostgreSQL 实现的 catalog 服务；组合根（app.rs）调用。
pub fn postgres_catalog_service(pool: sqlx::PgPool) -> CatalogService {
    CatalogService::new(std::sync::Arc::new(postgres::PgCatalogStore::new(pool)))
}

#[cfg(test)]
mod tests;
