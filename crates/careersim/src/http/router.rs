//! 路由装配：各业务模块自带分组限流，顶层只合并与挂中间件（plan/00 §6.3）。

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    extract::MatchedPath,
    http::{Request, Response, StatusCode},
    middleware,
    response::IntoResponse,
    routing::get,
};
use tower::ServiceBuilder;
use tower_http::{
    classify::ServerErrorsFailureClass,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::{Span, field};

use super::{
    account_routes, application_routes, auth::origin_check, catalog_routes, resume_routes,
};
use crate::{
    account::service::AccountService, agent::session::StoreSessionService,
    application::service::ApplicationService, catalog::service::CatalogService,
    resume::service::ResumeService,
};

#[derive(Clone)]
pub struct AppState {
    pub account: Arc<AccountService>,
    pub resume: Arc<ResumeService>,
    /// 公司模板目录服务（plan/01 §4.3）。
    pub catalog: Arc<CatalogService>,
    pub application: Arc<ApplicationService>,
    pub sessions: Arc<StoreSessionService>,
    /// 非 GET/HEAD/OPTIONS 请求的允许 Origin 列表（plan/01 §7.2）。
    pub allowed_origins: Arc<[String]>,
    /// Cookie `Secure` 属性开关；HttpOnly 恒开启。
    pub cookie_secure: bool,
}

/// 组装完整路由；中间件顺序：request-id → trace → origin-check → 各组 body-limit。
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/metrics", get(metrics))
        // 各业务路由自带分组限流，故在顶层合并（B2）。
        .merge(account_routes::router())
        .merge(application_routes::router())
        .merge(resume_routes::router())
        .merge(catalog_routes::router())
        .layer(middleware::from_fn_with_state(state.clone(), origin_check))
        .layer(
            // 请求 ID 最外层生成，trace 与响应头透传在内；日志可按请求聚合。
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(|request: &Request<_>| {
                            let route = request
                                .extensions()
                                .get::<MatchedPath>()
                                .map(MatchedPath::as_str)
                                .unwrap_or("unmatched");
                            let request_id = request
                                .extensions()
                                .get::<tower_http::request_id::RequestId>()
                                .and_then(|id| id.header_value().to_str().ok())
                                .unwrap_or("unknown");
                            tracing::info_span!(
                                "http.request",
                                method = %request.method(),
                                route = %route,
                                request_id = %request_id,
                                status_code = field::Empty,
                                latency_ms = field::Empty,
                            )
                        })
                        .on_request(())
                        .on_response(|response: &Response<_>, latency: Duration, span: &Span| {
                            let status = response.status();
                            let latency_ms = latency.as_secs_f64() * 1_000.0;
                            span.record("status_code", field::display(status));
                            span.record("latency_ms", latency_ms);
                            let status_class = format!("{}xx", status.as_u16() / 100);
                            metrics::counter!(
                                "careersim_http_requests_total",
                                "status_class" => status_class
                            )
                            .increment(1);
                            metrics::histogram!("careersim_http_request_duration_seconds")
                                .record(latency.as_secs_f64());
                            tracing::info!(
                                status_code = status.as_u16(),
                                latency_ms,
                                "http request completed"
                            );
                        })
                        .on_body_chunk(())
                        .on_eos(())
                        .on_failure(
                            |failure: ServerErrorsFailureClass, latency: Duration, _span: &Span| {
                                tracing::error!(
                                    error_kind = ?failure,
                                    latency_ms = latency.as_secs_f64() * 1_000.0,
                                    "http request failed"
                                );
                                metrics::counter!(
                                    "careersim_http_failures_total",
                                    "error_kind" => "server_failure"
                                )
                                .increment(1);
                            },
                        ),
                )
                .layer(PropagateRequestIdLayer::x_request_id()),
        )
        .with_state(state)
}

async fn healthz() -> StatusCode {
    StatusCode::OK
}

async fn metrics() -> impl IntoResponse {
    match crate::telemetry::render_metrics() {
        Some(body) => (
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4",
            )],
            body,
        )
            .into_response(),
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
