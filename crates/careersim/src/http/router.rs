//! 路由装配：各业务模块自带分组限流，顶层只合并与挂中间件（plan/00 §6.3）。

use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::get;
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

use crate::account::service::AccountService;
use crate::catalog::CatalogService;
use crate::resume::service::ResumeService;

use super::auth::origin_check;
use super::{account_routes, catalog_routes, resume_routes};

#[derive(Clone)]
pub struct AppState {
    pub account: Arc<AccountService>,
    pub resume: Arc<ResumeService>,
    /// 公司模板目录服务（plan/01 §4.3）。
    pub catalog: Arc<CatalogService>,
    /// 非 GET/HEAD/OPTIONS 请求的允许 Origin 列表（plan/01 §7.2）。
    pub allowed_origins: Arc<[String]>,
    /// Cookie `Secure` 属性开关；HttpOnly 恒开启。
    pub cookie_secure: bool,
}

/// 组装完整路由；中间件顺序：request-id → trace → origin-check → 各组 body-limit。
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        // 各业务路由自带分组限流，故在顶层合并（B2）。
        .merge(account_routes::router())
        .merge(resume_routes::router())
        .merge(catalog_routes::router())
        .layer(middleware::from_fn_with_state(state.clone(), origin_check))
        .layer(
            // 请求 ID 最外层生成，trace 与响应头透传在内；日志可按请求聚合。
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(TraceLayer::new_for_http())
                .layer(PropagateRequestIdLayer::x_request_id()),
        )
        .with_state(state)
}

async fn healthz() -> StatusCode {
    StatusCode::OK
}
