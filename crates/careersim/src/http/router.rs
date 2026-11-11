//! 路由装配：公开/私有分组与中间件顺序（plan/00 §6.3）。

use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{get, patch, post};
use tower::ServiceBuilder;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

use crate::account::service::AccountService;
use crate::resume::service::ResumeService;

use super::account_routes;
use super::auth::origin_check;
use super::resume_routes;

/// JSON 请求体上限：1 MiB。
const BODY_LIMIT_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub account: Arc<AccountService>,
    pub resume: Arc<ResumeService>,
    /// 非 GET/HEAD/OPTIONS 请求的允许 Origin 列表（plan/01 §7.2）。
    pub allowed_origins: Arc<[String]>,
    /// Cookie `Secure` 属性开关；HttpOnly 恒开启。
    pub cookie_secure: bool,
}

/// 组装完整路由；中间件顺序：request-id → trace → origin-check → body-limit。
pub fn router(state: AppState) -> Router {
    // 公开路由：注册、登录、健康检查。
    let public = Router::new()
        .route("/api/v1/auth/register", post(account_routes::register))
        .route("/api/v1/auth/login", post(account_routes::login))
        .route("/healthz", get(healthz))
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT_BYTES));

    // 私有路由：认证由 AuthContext 提取器强制。
    let private = Router::new()
        .route("/api/v1/auth/logout", post(account_routes::logout))
        .route("/api/v1/auth/me", get(account_routes::me))
        .route("/api/v1/me", patch(account_routes::update_profile))
        .route("/api/v1/me/password", post(account_routes::change_password))
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT_BYTES));

    Router::new()
        .merge(public)
        .merge(private)
        // 简历路由自带 1 MiB/12 MiB 分组限流，故在顶层合并（B2）。
        .merge(resume_routes::router())
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
