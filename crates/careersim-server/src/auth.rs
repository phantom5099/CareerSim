//! 演示账号认证。live 模式不加载任何演示凭据，登录接口直接不可用。
//!
//! 演示 token 保存在进程内存中，服务重启即全部失效；生产实现应替换为
//! 持久化会话或签名 token。

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::domain::User;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub user: User,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("unauthorized")]
    Unauthorized,
    #[error("authentication is not enabled")]
    Disabled,
}

#[derive(Clone)]
pub struct AuthService {
    demo: Option<Arc<DemoAuth>>,
}

struct DemoAuth {
    user: User,
    password: String,
    ttl: Duration,
    tokens: RwLock<HashMap<String, ActiveToken>>,
}

struct ActiveToken {
    expires_at: DateTime<Utc>,
}

/// 演示用户资料；密码只由 `AuthService` 持有。
pub fn demo_user(username: &str) -> User {
    User {
        id: "u_demo".to_owned(),
        username: username.to_owned(),
        display_name: "林同学".to_owned(),
        school: Some("南山大学".to_owned()),
        major: Some("计算机科学".to_owned()),
        graduation_year: Some(2027),
        target_roles: vec!["前端工程师".to_owned(), "全栈工程师".to_owned()],
        target_cities: vec!["杭州".to_owned(), "上海".to_owned()],
        avatar_text: "林".to_owned(),
        avatar_color: "#D94A2B".to_owned(),
    }
}

impl AuthService {
    pub fn demo(user: User, password: impl Into<String>, ttl: Duration) -> Self {
        Self {
            demo: Some(Arc::new(DemoAuth {
                user,
                password: password.into(),
                ttl,
                tokens: RwLock::new(HashMap::new()),
            })),
        }
    }

    pub fn disabled() -> Self {
        Self { demo: None }
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<LoginResponse, AuthError> {
        let demo = self.demo.as_ref().ok_or(AuthError::Disabled)?;
        if username != demo.user.username || password != demo.password {
            return Err(AuthError::InvalidCredentials);
        }

        let now = Utc::now();
        let expires_at = now + demo.ttl;
        let token = format!("cst_{}", Uuid::new_v4().simple());

        let mut tokens = demo.tokens.write().await;
        tokens.retain(|_, active| active.expires_at > now);
        tokens.insert(token.clone(), ActiveToken { expires_at });

        Ok(LoginResponse {
            token,
            expires_at,
            user: demo.user.clone(),
        })
    }

    pub async fn authenticate(&self, token: &str) -> Result<User, AuthError> {
        let demo = self.demo.as_ref().ok_or(AuthError::Disabled)?;
        let now = Utc::now();

        let mut tokens = demo.tokens.write().await;
        match tokens.get(token) {
            Some(active) if active.expires_at > now => Ok(demo.user.clone()),
            Some(_) => {
                tokens.remove(token);
                Err(AuthError::Unauthorized)
            }
            None => Err(AuthError::Unauthorized),
        }
    }

    pub async fn logout(&self, token: &str) {
        if let Some(demo) = &self.demo {
            demo.tokens.write().await.remove(token);
        }
    }
}
