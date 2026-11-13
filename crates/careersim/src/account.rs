//! 账户上下文：身份、密码、会话与资料。
//!
//! 依赖方向：`http -> AccountService -> UserStore/SessionStore/PasswordHasher`；
//! PG 适配器只通过 [`postgres_account_service`] 对外暴露。

pub mod model;
pub mod password;
pub(crate) mod postgres;
pub mod service;
pub mod session;
pub mod store;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use sqlx::PgPool;

pub use model::{
    AccountError, CredentialUser, NewUser, Password, PasswordHash, ProfilePatch, User, UserId,
    Username,
};
pub use password::{Argon2idHasher, PasswordError, PasswordHasher};
pub use service::{AccountService, Authenticated, Clock, SessionGrant, SystemClock};
pub use session::{
    IDLE_TIMEOUT, LoginSession, NewLoginSession, SESSION_COOKIE, SessionId, SessionToken,
    SessionTokenHash, TOUCH_THROTTLE,
};
pub use store::{SessionStore, SessionStoreError, UserStore, UserStoreError};

/// 构造基于 PostgreSQL 的账户服务；存储实现只通过这里对外暴露。
pub fn postgres_account_service(pool: PgPool, hasher: Arc<dyn PasswordHasher>) -> AccountService {
    AccountService::new(
        Arc::new(postgres::PgUserStore::new(pool.clone())),
        Arc::new(postgres::PgSessionStore::new(pool)),
        hasher,
        Arc::new(SystemClock),
    )
}
