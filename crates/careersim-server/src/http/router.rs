//! 路由装配：CORS、请求编号、追踪与统一 404。

use axum::Router;
use axum::http::HeaderValue;
use axum::http::Method;
use axum::http::header;
use axum::middleware;
use axum::routing::get;
use tower_http::cors::AllowOrigin;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use super::extractors::RequestId;
use super::extractors::attach_request_id;
use super::routes;
use super::routes::health;
use crate::error::ApiError;
use crate::state::AppState;

pub fn build_router(state: AppState, cors_origins: &[String]) -> Router {
    let origins = cors_origins
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok())
        .collect::<Vec<_>>();

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
        .nest("/api/v1", routes::api_routes())
        .fallback(not_found)
        .layer(middleware::from_fn(attach_request_id))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

async fn not_found(request_id: RequestId) -> ApiError {
    ApiError::not_found(request_id.as_str())
}
