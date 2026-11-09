//! 账号登录、当前用户与登出。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;

use crate::auth::LoginResponse;
use crate::domain::User;
use crate::error::ApiError;
use crate::http::extractors::AppJson;
use crate::http::extractors::AuthSession;
use crate::http::extractors::RequestId;
use crate::http::extractors::map_auth_error;
use crate::state::AppState;

const MAX_USERNAME_CHARS: usize = 64;
const MAX_PASSWORD_CHARS: usize = 128;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn login(
    State(state): State<AppState>,
    request_id: RequestId,
    AppJson(payload): AppJson<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let username = payload.username.trim();
    if username.is_empty() || username.chars().count() > MAX_USERNAME_CHARS {
        return Err(ApiError::validation(
            request_id.as_str(),
            "请输入有效的账号",
        ));
    }
    if payload.password.is_empty() || payload.password.chars().count() > MAX_PASSWORD_CHARS {
        return Err(ApiError::validation(
            request_id.as_str(),
            "请输入有效的密码",
        ));
    }

    state
        .auth
        .login(username, &payload.password)
        .await
        .map(Json)
        .map_err(|err| map_auth_error(err, request_id.as_str()))
}

pub async fn me(auth: AuthSession) -> Json<User> {
    Json(auth.user)
}

pub async fn logout(State(state): State<AppState>, auth: AuthSession) -> StatusCode {
    state.auth.logout(&auth.token).await;
    StatusCode::NO_CONTENT
}
