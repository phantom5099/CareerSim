//! 认证提取器、Origin 校验与会话 Cookie 构造。

use std::sync::Arc;

use axum::Json;
use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;

use crate::account::model::{User, UserId};
use crate::account::session::{COOKIE_MAX_AGE_SECS, SESSION_COOKIE, SessionId};

use super::error::ApiError;
use super::router::AppState;

/// 当前登录态；处理器唯一合法的归属来源（plan/01 §7.3）。
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: UserId,
    pub session_id: SessionId,
    /// 认证时顺带取回的用户，处理器无需再查。
    pub user: User,
}

impl FromRequestParts<AppState> for AuthContext {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_infallible| ApiError::unauthenticated())?;
        let token = jar
            .get(SESSION_COOKIE)
            .map(|cookie| cookie.value())
            .ok_or_else(ApiError::unauthenticated)?;

        let authenticated = state
            .account
            .authenticate(token)
            .await
            .map_err(ApiError::from)?;
        Ok(AuthContext {
            user_id: authenticated.user.id,
            session_id: authenticated.session_id,
            user: authenticated.user,
        })
    }
}

/// 非 GET/HEAD/OPTIONS 请求校验 `Origin`：缺失或不允许的来源一律 403（plan/01 §7.2）。
pub async fn origin_check(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, OriginRejection> {
    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
        return Ok(next.run(request).await);
    }
    let origin = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(OriginRejection)?;
    // 配置侧统一小写比较，避免 scheme/主机名大小写差异误伤。
    let allowed = state
        .allowed_origins
        .iter()
        .any(|item| item.eq_ignore_ascii_case(origin));
    if !allowed {
        return Err(OriginRejection);
    }
    Ok(next.run(request).await)
}

/// Origin 校验失败：403，code=forbidden。
#[derive(Debug)]
pub struct OriginRejection;

impl IntoResponse for OriginRejection {
    fn into_response(self) -> Response {
        (
            StatusCode::FORBIDDEN,
            Json(ApiError::new("forbidden", "origin not allowed".to_owned())),
        )
            .into_response()
    }
}

/// Set-Cookie 头构造；`axum-extra` 不带 `time` 依赖，因此手工拼装属性串。
/// 令牌为 Base64URL 字符集，不会注入属性分隔符。
pub fn session_cookie_header(token: &str, secure: bool) -> String {
    let mut header = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={COOKIE_MAX_AGE_SECS}"
    );
    if secure {
        header.push_str("; Secure");
    }
    header
}

/// 清除 Cookie 的 Set-Cookie 头；属性需与写入时一致才能确保覆盖。
pub fn clear_session_cookie_header(secure: bool) -> String {
    let mut header = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    if secure {
        header.push_str("; Secure");
    }
    header
}

/// 供 app.rs 装配 `Arc<AccountService>` 时的类型别名。
pub type SharedAccount = Arc<crate::account::service::AccountService>;
