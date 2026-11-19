//! CareerSim 求职模拟器的 AgentLoop 实现。

use std::sync::Arc;

use sqlx::PgPool;

pub mod account;
pub mod agent;
pub mod app;
pub mod catalog;
pub mod config;
pub mod error;
pub mod http;
pub mod llm;
pub mod pipeline;
pub mod resume;
pub(crate) mod store;
pub mod telemetry;

#[cfg(test)]
pub(crate) mod testing;

/// 构造基于 PostgreSQL 的会话服务；存储实现只通过这里对外暴露。
pub fn postgres_session_service(pool: PgPool) -> agent::session::StoreSessionService {
    agent::session::StoreSessionService::new(Arc::new(store::PostgresSessionStore::new(pool)))
}
