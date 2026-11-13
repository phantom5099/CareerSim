//! 账户路由处理器：注册、登录、注销、资料与改密（plan/01 §6.1）。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::account::model::{ProfilePatch, User};
use crate::account::service::SessionGrant;

use super::auth::{AuthContext, clear_session_cookie_header, session_cookie_header};
use super::error::ApiError;
use super::router::AppState;

/// 用户 DTO；`avatarText`/`avatarColor` 由展示名稳定派生，不持久化。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDto {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub school: Option<String>,
    pub major: Option<String>,
    pub graduation_year: Option<u16>,
    pub target_roles: Vec<String>,
    pub target_cities: Vec<String>,
    pub avatar_text: String,
    pub avatar_color: String,
}

impl From<User> for UserDto {
    fn from(user: User) -> Self {
        let User {
            id,
            username,
            display_name,
            school,
            major,
            graduation_year,
            target_roles,
            target_cities,
            ..
        } = user;
        let avatar_text = display_name.chars().next().unwrap_or('U').to_string();
        let avatar_color = AVATAR_PALETTE[fnv1a(&display_name) % AVATAR_PALETTE.len()].to_owned();
        Self {
            id: id.into_uuid().to_string(),
            username,
            display_name,
            school,
            major,
            graduation_year,
            target_roles,
            target_cities,
            avatar_text,
            avatar_color,
        }
    }
}

/// 头像主题色调色板；索引由展示名的 FNV-1a 哈希决定。
const AVATAR_PALETTE: [&str; 8] = [
    "#4f6ef7", "#3d5a45", "#c13d24", "#7c5cff", "#0e9f8a", "#b5541c", "#2b6cb0", "#8b2f5f",
];

/// FNV-1a 32 位哈希；只用于稳定派生头像索引，非安全用途。
fn fnv1a(input: &str) -> usize {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in input.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash as usize
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// 资料补丁 DTO；双层 Option 表达 PATCH 语义。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePatchRequest {
    pub display_name: Option<String>,
    pub school: Option<Option<String>>,
    pub major: Option<Option<String>>,
    pub graduation_year: Option<Option<u16>>,
    pub target_roles: Option<Vec<String>>,
    pub target_cities: Option<Vec<String>>,
}

impl From<ProfilePatchRequest> for ProfilePatch {
    fn from(dto: ProfilePatchRequest) -> Self {
        Self {
            display_name: dto.display_name,
            school: dto.school,
            major: dto.major,
            graduation_year: dto.graduation_year,
            target_roles: dto.target_roles,
            target_cities: dto.target_cities,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// 201 + Set-Cookie：注册即登录。
pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Response, ApiError> {
    let grant = state
        .account
        .register(&request.username, &request.password, &request.display_name)
        .await?;
    Ok(session_response(
        StatusCode::CREATED,
        grant,
        state.cookie_secure,
    ))
}

/// 200 + Set-Cookie；失败统一 401。
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    let grant = state
        .account
        .login(&request.username, &request.password)
        .await?;
    Ok(session_response(StatusCode::OK, grant, state.cookie_secure))
}

/// 204 + 清 Cookie。
pub async fn logout(
    State(state): State<AppState>,
    auth: AuthContext,
) -> Result<Response, ApiError> {
    state.account.logout(auth.session_id).await?;
    let response = StatusCode::NO_CONTENT.into_response();
    Ok(with_cookie(
        response,
        &clear_session_cookie_header(state.cookie_secure),
    ))
}

/// 200 当前用户。
pub async fn me(auth: AuthContext) -> Result<Response, ApiError> {
    Ok((StatusCode::OK, Json(UserDto::from(auth.user))).into_response())
}

/// 200 修改后的用户。
pub async fn update_profile(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<ProfilePatchRequest>,
) -> Result<Response, ApiError> {
    let user = state
        .account
        .update_profile(auth.user_id, request.into())
        .await?;
    Ok((StatusCode::OK, Json(UserDto::from(user))).into_response())
}

/// 204 + 新 Cookie：撤销其他会话并轮换当前令牌。
pub async fn change_password(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Response, ApiError> {
    let token = state
        .account
        .change_password(
            auth.user_id,
            auth.session_id,
            &request.current_password,
            &request.new_password,
        )
        .await?;
    let response = StatusCode::NO_CONTENT.into_response();
    Ok(with_cookie(
        response,
        &session_cookie_header(&token, state.cookie_secure),
    ))
}

/// 认证相关的处理器不直接依赖 `AccountService` 具体类型以外的端口。
fn session_response(status: StatusCode, grant: SessionGrant, secure: bool) -> Response {
    let body = Json(UserDto::from(grant.user));
    let response = (status, body).into_response();
    with_cookie(response, &session_cookie_header(&grant.token, secure))
}

fn with_cookie(response: Response, cookie: &str) -> Response {
    let (mut parts, body) = response.into_parts();
    parts.headers.append(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(cookie).unwrap_or_else(|_| {
            // 令牌为 Base64URL 字符集；构造失败只可能来自编程错误。
            axum::http::HeaderValue::from_static("careersim_session=")
        }),
    );
    Response::from_parts(parts, body)
}

#[cfg(test)]
mod tests;
