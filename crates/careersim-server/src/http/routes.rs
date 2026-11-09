//! `/api/v1` 下的业务路由。

use axum::Router;
use axum::routing::get;
use axum::routing::post;

use crate::state::AppState;

pub mod auth;
pub mod companies;
pub mod health;

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/me", get(auth::me))
        .route("/auth/logout", post(auth::logout))
        .route("/companies/industries", get(companies::industries))
}
