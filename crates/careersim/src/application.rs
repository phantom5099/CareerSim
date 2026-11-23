//! 投递持久化：岗位投递记录与完整流程会话的一对一关系。

pub mod model;
pub(crate) mod postgres;
pub mod service;
pub mod store;

pub use model::{
    Application, ApplicationError, ApplicationId, ApplicationStageView, ApplicationStatus,
    ApplicationView, CreateApplicationInput, OfferResponseStatus, OfferView,
};
pub use service::ApplicationService;
pub use store::CreateApplicationRecord;

/// 组装 PostgreSQL 投递服务；组合根负责注入数据库连接池。
pub fn postgres_application_service(pool: sqlx::PgPool) -> ApplicationService {
    ApplicationService::new(std::sync::Arc::new(postgres::PgApplicationStore::new(pool)))
}
