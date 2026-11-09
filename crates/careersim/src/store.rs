//! 会话与事件存储。事件日志是唯一事实来源。
//!
//! 本模块只负责带版本号的事件日志和会话行缓存，不解释事件内容，也不依赖
//! `agent` 的领域类型。领域校验与投影由 `agent::session` 完成。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

#[cfg(test)]
pub(crate) mod memory;
pub(crate) mod postgres;

#[cfg(test)]
pub(crate) use memory::MemorySessionStore;
pub(crate) use postgres::PostgresSessionStore;

/// 当前事件格式版本。
pub(crate) const EVENT_VERSION: u32 = 1;

/// 会话行及其缓存字段；`stage` 对 store 是不透明字符串。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StoredSession {
    pub id: Uuid,
    pub stage: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seq: u64,
    pub last_turn_id: u64,
}

/// 已落库的事件；`kind` 对 store 是不透明的 JSON。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StoredEvent {
    pub v: u32,
    pub seq: u64,
    pub at: DateTime<Utc>,
    pub kind: Value,
}

/// 追加前期望达到的会话状态，用于比较并交换。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExpectedState<'a> {
    pub stage: &'a str,
    pub last_seq: u64,
}

/// 一次追加请求；`v`、`seq`、`at` 由 store 分配。
#[derive(Debug, Clone, Copy)]
pub(crate) struct AppendBatch<'a> {
    pub expected: ExpectedState<'a>,
    pub events: &'a [Value],
    pub last_turn_id: u64,
    pub next_stage: Option<&'a str>,
}

#[async_trait]
pub(crate) trait SessionStore: Send + Sync {
    /// 创建会话行并写入 seq = 0 的元事件。
    async fn create(
        &self,
        id: Uuid,
        stage: &str,
        meta: Value,
    ) -> Result<(StoredSession, StoredEvent), StoreError>;

    async fn load(&self, id: Uuid) -> Result<(StoredSession, Vec<StoredEvent>), StoreError>;

    async fn list(&self, limit: usize) -> Result<Vec<StoredSession>, StoreError>;

    async fn delete(&self, id: Uuid) -> Result<(), StoreError>;

    /// 以期望状态做 CAS 后追加一批事件；状态已被改写时返回 `Conflict`。
    async fn append(
        &self,
        id: Uuid,
        batch: AppendBatch<'_>,
    ) -> Result<Vec<StoredEvent>, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum StoreError {
    #[error("session not found")]
    NotFound,
    #[error("session state changed concurrently")]
    Conflict,
    #[error("corrupt session: {0}")]
    Corrupt(String),
    #[error("store backend error: {message}")]
    Backend {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

pub(crate) fn to_u64(value: i64, field: &str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::Corrupt(format!("{field} is negative: {value}")))
}

pub(crate) fn backend(
    operation: &str,
    err: impl std::error::Error + Send + Sync + 'static,
) -> StoreError {
    StoreError::Backend {
        message: operation.to_owned(),
        source: Some(Box::new(err)),
    }
}
