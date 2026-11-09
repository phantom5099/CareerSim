//! 存活与就绪探针。

use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct Health {
    status: &'static str,
}

pub async fn healthz() -> Json<Health> {
    Json(Health { status: "ok" })
}

pub async fn readyz() -> Json<Health> {
    Json(Health { status: "ok" })
}
