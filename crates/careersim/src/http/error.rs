//! 统一错误体与内部错误到 HTTP 响应的映射（plan/00 §6.2）。

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::account::model::AccountError;

/// 错误响应体：`{"error": {"code", "message", "fields"?}}`。
/// 状态码由 `code` 统一推导，保证映射关系只有一处。
#[derive(Debug, Serialize)]
struct ApiErrorBody {
    error: ErrorPayload,
}

#[derive(Debug, Serialize)]
struct ErrorPayload {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: Option<Vec<String>>,
}

/// 对外的 API 错误；构造函数即映射表（plan/00 §6.2）。
#[derive(Debug, Serialize)]
pub struct ApiError(ApiErrorBody);

impl ApiError {
    pub(crate) fn new(code: &'static str, message: String) -> Self {
        Self(ApiErrorBody {
            error: ErrorPayload {
                code,
                message,
                fields: None,
            },
        })
    }

    /// 400 输入校验失败。
    pub fn validation_failed(message: impl Into<String>) -> Self {
        Self::new("validation_failed", message.into())
    }

    /// 400 输入校验失败，附带出错字段清单。
    pub fn validation_fields(message: impl Into<String>, fields: Vec<String>) -> Self {
        let mut error = Self::validation_failed(message);
        error.0.error.fields = Some(fields);
        error
    }

    /// 401 未登录或会话失效。
    pub fn unauthenticated() -> Self {
        Self::new("unauthenticated", "authentication required".to_owned())
    }

    /// 401 登录失败；文案不区分用户不存在与密码错误。
    pub fn invalid_credentials() -> Self {
        Self::new("invalid_credentials", "invalid credentials".to_owned())
    }

    /// 400 当前密码不正确；独立 code 便于前端给出准确文案。
    pub fn current_password_mismatch() -> Self {
        Self::new(
            "current_password_mismatch",
            "current password does not match".to_owned(),
        )
    }

    /// 409 唯一冲突。
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new("conflict", message.into())
    }

    /// 500 内部错误；对外只给固定文案，细节仅进日志。
    pub fn internal_error() -> Self {
        Self::new("internal_error", "internal error".to_owned())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0.error.code {
            "validation_failed" | "current_password_mismatch" => StatusCode::BAD_REQUEST,
            "unauthenticated" | "invalid_credentials" => StatusCode::UNAUTHORIZED,
            "forbidden" => StatusCode::FORBIDDEN,
            "not_found" => StatusCode::NOT_FOUND,
            "conflict" => StatusCode::CONFLICT,
            "payload_too_large" => StatusCode::PAYLOAD_TOO_LARGE,
            "unsupported_media_type" => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "rate_limited" => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self.0)).into_response()
    }
}

impl From<AccountError> for ApiError {
    fn from(err: AccountError) -> Self {
        match err {
            AccountError::InvalidUsername { .. } | AccountError::InvalidPassword { .. } => {
                ApiError::validation_failed(err.to_string())
            }
            AccountError::InvalidProfile { field, .. } => {
                ApiError::validation_fields(err.to_string(), vec![field.to_owned()])
            }
            AccountError::InvalidCredentials => ApiError::invalid_credentials(),
            AccountError::CurrentPasswordMismatch => ApiError::current_password_mismatch(),
            AccountError::UsernameTaken => ApiError::conflict("username already taken"),
            AccountError::Unauthenticated => ApiError::unauthenticated(),
            // 后端错误细节只进日志；对外统一 internal_error，避免泄露内部信息。
            AccountError::Backend { message, source } => {
                tracing::error!(error = %render_backend_chain(&message, source.as_deref()), "account backend error");
                ApiError::internal_error()
            }
        }
    }
}

/// 把 message 与 source 链拼成一行；后续 `source()` 返回 `&dyn Error`，故游标做窄类型。
fn render_backend_chain(
    message: &str,
    source: Option<&(dyn std::error::Error + Send + Sync + 'static)>,
) -> String {
    let mut rendered = message.to_owned();
    let mut cursor: Option<&(dyn std::error::Error + 'static)> = source.map(|err| err as _);
    while let Some(cause) = cursor {
        rendered.push_str(": ");
        rendered.push_str(&cause.to_string());
        cursor = cause.source();
    }
    rendered
}
