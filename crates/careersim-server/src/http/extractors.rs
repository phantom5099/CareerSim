//! 认证、请求编号与 JSON 提取器；失败时统一返回 `ApiError`。

use axum::Json;
use axum::extract::FromRequest;
use axum::extract::FromRequestParts;
use axum::extract::Request;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::auth::AuthError;
use crate::domain::User;
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

impl RequestId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<S> FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<RequestId>()
            .cloned()
            .ok_or_else(|| ApiError::internal(Uuid::new_v4().to_string()))
    }
}

/// 为每个请求分配编号，并写入响应头的 `x-request-id`。
pub async fn attach_request_id(mut request: Request, next: Next) -> Response {
    let request_id = Uuid::new_v4().to_string();
    request
        .extensions_mut()
        .insert(RequestId(request_id.clone()));

    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

/// 包装 `axum::Json`，把解析失败映射成统一错误信封。
pub struct AppJson<T>(pub T);

impl<S, T> FromRequest<S> for AppJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let request_id = request
            .extensions()
            .get::<RequestId>()
            .map(|id| id.0.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        match Json::<T>::from_request(request, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(ApiError::validation(
                request_id,
                json_rejection_message(rejection),
            )),
        }
    }
}

fn json_rejection_message(rejection: JsonRejection) -> String {
    match rejection {
        JsonRejection::MissingJsonContentType(_) => {
            "请求需要 Content-Type: application/json".to_owned()
        }
        JsonRejection::JsonDataError(_) => "请求参数格式不正确".to_owned(),
        JsonRejection::JsonSyntaxError(_) => "请求体不是合法的 JSON".to_owned(),
        JsonRejection::BytesRejection(_) => "请求体读取失败".to_owned(),
        other => other.body_text(),
    }
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub user: User,
    pub token: String,
}

impl FromRequestParts<AppState> for AuthSession {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let request_id = parts
            .extensions
            .get::<RequestId>()
            .map(|id| id.0.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ApiError::unauthorized(request_id.clone()))?;

        let token = header
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| ApiError::unauthorized(request_id.clone()))?;

        let user = state
            .auth
            .authenticate(token)
            .await
            .map_err(|err| map_auth_error(err, &request_id))?;

        Ok(Self {
            user,
            token: token.to_owned(),
        })
    }
}

pub fn map_auth_error(error: AuthError, request_id: &str) -> ApiError {
    match error {
        AuthError::InvalidCredentials => ApiError::invalid_credentials(request_id),
        AuthError::Unauthorized => ApiError::unauthorized(request_id),
        AuthError::Disabled => ApiError::auth_unavailable(request_id),
    }
}
