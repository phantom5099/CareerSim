//! 账户存储端口；PG 实现见 postgres.rs，测试替身见 tests.rs。

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::{
    model::{CredentialUser, PasswordHash, UpdateProfileInput, User, UserId, Username},
    session::{CreateLoginSessionRecord, LoginSession, SessionId, SessionTokenHash},
};

/// 已完成账户校验、可交给 users 表写入的记录。
pub struct CreateUserRecord {
    pub id: UserId,
    pub username: String,
    pub username_key: Username,
    pub password_hash: PasswordHash,
    pub display_name: String,
    pub registered_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum UserStoreError {
    #[error("username already taken")]
    Conflict,
    #[error("user not found")]
    NotFound,
    #[error("user store backend error: {message}")]
    Backend {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum SessionStoreError {
    #[error("session not found")]
    NotFound,
    #[error("session store backend error: {message}")]
    Backend {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

#[async_trait]
pub trait UserStore: Send + Sync {
    async fn insert(&self, new: CreateUserRecord) -> Result<User, UserStoreError>;
    async fn find_credential(
        &self,
        username_key: &str,
    ) -> Result<Option<CredentialUser>, UserStoreError>;
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, UserStoreError>;
    /// 只应用 `patch` 中显式给出的字段；`now` 写入 `updated_at`。
    async fn update_profile(
        &self,
        id: UserId,
        patch: &UpdateProfileInput,
        now: DateTime<Utc>,
    ) -> Result<User, UserStoreError>;
    async fn update_password(
        &self,
        id: UserId,
        hash: &PasswordHash,
        now: DateTime<Utc>,
    ) -> Result<(), UserStoreError>;
}

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn insert(&self, session: CreateLoginSessionRecord) -> Result<(), SessionStoreError>;
    async fn find_by_token_hash(
        &self,
        hash: &SessionTokenHash,
    ) -> Result<Option<LoginSession>, SessionStoreError>;
    /// 顺延 `last_seen_at` 并把 `idle_expires_at` 重算为 min(顺延值, 绝对过期)；
    /// 调用方已按节流窗口判定，两个时间都由服务层计算以保证测试确定性。
    async fn touch(
        &self,
        id: SessionId,
        last_seen_at: DateTime<Utc>,
        idle_expires_at: DateTime<Utc>,
    ) -> Result<(), SessionStoreError>;
    /// 幂等：会话不存在或已撤销时同样成功。
    async fn revoke(&self, id: SessionId, now: DateTime<Utc>) -> Result<(), SessionStoreError>;
    /// 撤销该用户除 `keep` 外的所有有效会话。
    async fn revoke_others(
        &self,
        user: UserId,
        keep: SessionId,
        now: DateTime<Utc>,
    ) -> Result<(), SessionStoreError>;
    /// 同一事务内撤销旧会话并写入新会话；旧会话缺失时返回 `NotFound`。
    async fn rotate(
        &self,
        old: SessionId,
        new: CreateLoginSessionRecord,
        now: DateTime<Utc>,
    ) -> Result<(), SessionStoreError>;
}
