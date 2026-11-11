//! 账户用例服务：注册、登录、注销、资料、改密与会话认证。

use std::fmt;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::task;

use super::model::{
    AccountError, NewUser, Password, PasswordHash, ProfilePatch, User, UserId, Username,
};
use super::password::PasswordHasher;
use super::session::{SessionId, SessionToken, new_login_session};
use super::store::{SessionStore, SessionStoreError, UserStore, UserStoreError};

/// 时钟端口；测试用 `FakeClock`，生产用 [`SystemClock`]。
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// 注册或登录成功的返回：用户与会话令牌原文（仅此一次返回）。
pub struct SessionGrant {
    pub user: User,
    pub token: String,
}

impl fmt::Debug for SessionGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionGrant")
            .field("user", &self.user)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// 认证成功：当前用户与会话标识；HTTP 层由此构造 `AuthContext`。
pub struct Authenticated {
    pub user: User,
    pub session_id: SessionId,
}

impl fmt::Debug for Authenticated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Authenticated")
            .field("user", &self.user)
            .field("session_id", &self.session_id)
            .finish()
    }
}

pub struct AccountService {
    users: Arc<dyn UserStore>,
    sessions: Arc<dyn SessionStore>,
    hasher: Arc<dyn PasswordHasher>,
    clock: Arc<dyn Clock>,
}

impl AccountService {
    pub fn new(
        users: Arc<dyn UserStore>,
        sessions: Arc<dyn SessionStore>,
        hasher: Arc<dyn PasswordHasher>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            users,
            sessions,
            hasher,
            clock,
        }
    }

    /// 注册即登录：创建用户并签发会话。
    pub async fn register(
        &self,
        username: &str,
        password: &str,
        display_name: &str,
    ) -> Result<SessionGrant, AccountError> {
        let username_key = Username::parse(username)?;
        let password = Password::parse(password)?;
        let display_name = display_name.trim();
        let display_chars = display_name.chars().count();
        if !(1..=40).contains(&display_chars) {
            return Err(AccountError::InvalidProfile {
                field: "displayName",
                reason: "must be 1..=40 characters",
            });
        }

        let password_hash = self.hash_blocking(&password).await?;
        let now = self.clock.now();
        let user = self
            .users
            .insert(NewUser {
                id: UserId::generate(),
                username: username.trim().to_owned(),
                username_key,
                password_hash,
                display_name: display_name.to_owned(),
                registered_at: now,
            })
            .await
            .map_err(|err| match err {
                UserStoreError::Conflict => AccountError::UsernameTaken,
                UserStoreError::NotFound => AccountError::Backend {
                    message: "insert user reported not found".to_owned(),
                    source: None,
                },
                UserStoreError::Backend { message, source } => {
                    AccountError::Backend { message, source }
                }
            })?;

        let grant = self.issue_session(user, now).await?;
        Ok(grant)
    }

    /// 登录失败统一返回 `InvalidCredentials`；未知用户执行假验证拉平耗时。
    pub async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<SessionGrant, AccountError> {
        let username_key =
            Username::parse(username).map_err(|_| AccountError::InvalidCredentials)?;
        let password = Password::parse(password).map_err(|_| AccountError::InvalidCredentials)?;

        let credential = self
            .users
            .find_credential(username_key.as_str())
            .await
            .map_err(store_user_error)?;
        let credential = match credential {
            Some(credential) => credential,
            // 未知用户也执行一次同量级 Argon2 假验证，拉平与命中用户的耗时差（S-A06）。
            None => {
                self.dummy_verify_blocking(&password).await?;
                return Err(AccountError::InvalidCredentials);
            }
        };

        let verified = self
            .verify_blocking(&password, &credential.password_hash)
            .await?;
        if !verified {
            return Err(AccountError::InvalidCredentials);
        }

        let user = self
            .users
            .find_by_id(credential.id)
            .await
            .map_err(store_user_error)?
            .ok_or(AccountError::InvalidCredentials)?;
        self.issue_session(user, self.clock.now()).await
    }

    /// 撤销当前会话；幂等。
    pub async fn logout(&self, session_id: SessionId) -> Result<(), AccountError> {
        self.sessions
            .revoke(session_id, self.clock.now())
            .await
            .map_err(store_session_error)?;
        Ok(())
    }

    /// 按登录态查询用户；用户已不存在视为会话失效。
    pub async fn me(&self, user_id: UserId) -> Result<User, AccountError> {
        self.users
            .find_by_id(user_id)
            .await
            .map_err(store_user_error)?
            .ok_or(AccountError::Unauthenticated)
    }

    /// 只更新当前用户资料；字段边界见 `ProfilePatch::validate`。
    pub async fn update_profile(
        &self,
        user_id: UserId,
        patch: ProfilePatch,
    ) -> Result<User, AccountError> {
        patch.validate()?;
        self.users
            .update_profile(user_id, &patch, self.clock.now())
            .await
            .map_err(|err| match err {
                UserStoreError::NotFound => AccountError::Unauthenticated,
                other => store_user_error(other),
            })
    }

    /// 改密：校验旧密码 → 更新哈希 → 撤销其他会话 → 轮换当前令牌（S-A05）。
    /// 返回新令牌原文，旧令牌立即失效。
    pub async fn change_password(
        &self,
        user_id: UserId,
        session_id: SessionId,
        current_password: &str,
        new_password: &str,
    ) -> Result<String, AccountError> {
        let current =
            Password::parse(current_password).map_err(|_| AccountError::CurrentPasswordMismatch)?;
        let new_password = Password::parse(new_password)?;

        let user = self.me(user_id).await?;
        // 存量展示名重新归一化得到查询键；失败说明数据被绕过服务层改动。
        let username_key = Username::parse(&user.username).map_err(|_| AccountError::Backend {
            message: "stored username failed normalization".to_owned(),
            source: None,
        })?;
        let credential = self
            .users
            .find_credential(username_key.as_str())
            .await
            .map_err(store_user_error)?
            .ok_or(AccountError::Unauthenticated)?;

        let verified = self
            .verify_blocking(&current, &credential.password_hash)
            .await?;
        if !verified {
            return Err(AccountError::CurrentPasswordMismatch);
        }

        let new_hash = self.hash_blocking(&new_password).await?;
        let now = self.clock.now();
        self.users
            .update_password(user_id, &new_hash, now)
            .await
            .map_err(|err| match err {
                UserStoreError::NotFound => AccountError::Unauthenticated,
                other => store_user_error(other),
            })?;
        self.sessions
            .revoke_others(user_id, session_id, now)
            .await
            .map_err(store_session_error)?;

        let token = SessionToken::generate();
        let (_, new_session) = new_login_session(user_id, &token, now);
        self.sessions
            .rotate(session_id, new_session, now)
            .await
            .map_err(|err| match err {
                SessionStoreError::NotFound => AccountError::Unauthenticated,
                other => store_session_error(other),
            })?;
        Ok(token.encode())
    }

    /// 会话认证：令牌 → 哈希 → 查会话 → 校验过期 → 查用户。
    /// 超过节流窗口时顺延活跃时间；顺延失败不影响本次认证。
    pub async fn authenticate(&self, token: &str) -> Result<Authenticated, AccountError> {
        let token = SessionToken::decode(token).ok_or(AccountError::Unauthenticated)?;
        let session = self
            .sessions
            .find_by_token_hash(&token.hash())
            .await
            .map_err(store_session_error)?
            .ok_or(AccountError::Unauthenticated)?;

        let now = self.clock.now();
        if !session.is_active_at(now) {
            return Err(AccountError::Unauthenticated);
        }
        if session.needs_touch(now) {
            let touched_idle = session.touched_idle_expires_at(now);
            if let Err(err) = self.sessions.touch(session.id, now, touched_idle).await {
                // 活跃时间顺延是尽力而为；失败只影响空闲计时的起点。
                tracing::warn!(
                    error = %crate::error::error_chain(&err),
                    session_id = %session.id,
                    "failed to touch login session"
                );
            } else {
                tracing::debug!(session_id = %session.id, "login session touched");
            }
        }

        let user = self
            .users
            .find_by_id(session.user_id)
            .await
            .map_err(store_user_error)?
            .ok_or(AccountError::Unauthenticated)?;
        Ok(Authenticated {
            user,
            session_id: session.id,
        })
    }

    async fn issue_session(
        &self,
        user: User,
        now: DateTime<Utc>,
    ) -> Result<SessionGrant, AccountError> {
        let token = SessionToken::generate();
        let (_, new_session) = new_login_session(user.id, &token, now);
        self.sessions
            .insert(new_session)
            .await
            .map_err(store_session_error)?;
        Ok(SessionGrant {
            user,
            token: token.encode(),
        })
    }

    async fn hash_blocking(&self, password: &Password) -> Result<PasswordHash, AccountError> {
        let hasher = Arc::clone(&self.hasher);
        let owned = password.clone();
        task::spawn_blocking(move || hasher.hash(&owned))
            .await
            .map_err(|err| AccountError::Backend {
                message: "hash task failed".to_owned(),
                source: Some(Box::new(err)),
            })?
            .map_err(|err| AccountError::Backend {
                message: "hash password".to_owned(),
                source: Some(Box::new(err)),
            })
    }

    async fn verify_blocking(
        &self,
        password: &Password,
        hash: &PasswordHash,
    ) -> Result<bool, AccountError> {
        let hasher = Arc::clone(&self.hasher);
        let owned = password.clone();
        let owned_hash = hash.clone();
        task::spawn_blocking(move || hasher.verify(&owned, &owned_hash))
            .await
            .map_err(|err| AccountError::Backend {
                message: "verify task failed".to_owned(),
                source: Some(Box::new(err)),
            })?
            .map_err(|err| AccountError::Backend {
                message: "verify password".to_owned(),
                source: Some(Box::new(err)),
            })
    }

    /// 未知用户路径：执行一次与真实验证同量的 Argon2 计算（S-A06）。
    async fn dummy_verify_blocking(&self, password: &Password) -> Result<(), AccountError> {
        let hasher = Arc::clone(&self.hasher);
        let owned = password.clone();
        task::spawn_blocking(move || hasher.dummy_verify(&owned))
            .await
            .map_err(|err| AccountError::Backend {
                message: "dummy verify task failed".to_owned(),
                source: Some(Box::new(err)),
            })?;
        Ok(())
    }
}

fn store_user_error(err: UserStoreError) -> AccountError {
    match err {
        UserStoreError::NotFound => AccountError::Unauthenticated,
        UserStoreError::Conflict => AccountError::UsernameTaken,
        UserStoreError::Backend { message, source } => AccountError::Backend { message, source },
    }
}

fn store_session_error(err: SessionStoreError) -> AccountError {
    match err {
        SessionStoreError::NotFound => AccountError::Unauthenticated,
        SessionStoreError::Backend { message, source } => AccountError::Backend { message, source },
    }
}
