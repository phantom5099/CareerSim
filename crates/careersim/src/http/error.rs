//! 统一错误体与内部错误到 HTTP 响应的映射（plan/00 §6.2）。

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::account::model::AccountError;

/// 对外错误码集合；serde 序列化结果即 wire 契约（plan/00 §6.2 的 code）。
/// 新增变体时必须同步补全 `status()` 的穷尽 match，编译器强制两者同步演进。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ValidationFailed,
    CurrentPasswordMismatch,
    Unauthenticated,
    InvalidCredentials,
    Forbidden,
    NotFound,
    Conflict,
    PayloadTooLarge,
    UnsupportedMediaType,
    RateLimited,
    InternalError,
}

impl ErrorCode {
    /// code → HTTP 状态码；穷尽 match，禁止 `_` 兜底分支。
    pub fn status(self) -> StatusCode {
        match self {
            Self::ValidationFailed | Self::CurrentPasswordMismatch => StatusCode::BAD_REQUEST,
            Self::Unauthenticated | Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// 错误响应体：`{"error": {"code", "message", "fields"?}}`。
#[derive(Debug, Serialize)]
struct ApiErrorBody {
    error: ErrorPayload,
}

#[derive(Debug, Serialize)]
struct ErrorPayload {
    code: ErrorCode,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: Option<Vec<String>>,
}

/// 对外的 API 错误；错误码只能通过具名构造函数产生，状态码由
/// `ErrorCode::status` 统一推导，code → 状态码的映射不存在第二处。
#[derive(Debug, Serialize)]
pub struct ApiError(ApiErrorBody);

impl ApiError {
    fn with_code(code: ErrorCode, message: String) -> Self {
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
        Self::with_code(ErrorCode::ValidationFailed, message.into())
    }

    /// 400 输入校验失败，附带出错字段清单。
    pub fn validation_fields(message: impl Into<String>, fields: Vec<String>) -> Self {
        let mut error = Self::validation_failed(message);
        error.0.error.fields = Some(fields);
        error
    }

    /// 401 未登录或会话失效。
    pub fn unauthenticated() -> Self {
        Self::with_code(
            ErrorCode::Unauthenticated,
            "authentication required".to_owned(),
        )
    }

    /// 401 登录失败；文案不区分用户不存在与密码错误。
    pub fn invalid_credentials() -> Self {
        Self::with_code(
            ErrorCode::InvalidCredentials,
            "invalid credentials".to_owned(),
        )
    }

    /// 400 当前密码不正确；独立 code 便于前端给出准确文案。
    pub fn current_password_mismatch() -> Self {
        Self::with_code(
            ErrorCode::CurrentPasswordMismatch,
            "current password does not match".to_owned(),
        )
    }

    /// 403 越权或来源被拒。
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::with_code(ErrorCode::Forbidden, message.into())
    }

    /// 404 资源不存在；越权查询统一用它替代 403，避免泄露资源存在性。
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::with_code(ErrorCode::NotFound, message.into())
    }

    /// 409 唯一冲突。
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::with_code(ErrorCode::Conflict, message.into())
    }

    /// 413 请求体超过大小限制。
    pub fn payload_too_large(message: impl Into<String>) -> Self {
        Self::with_code(ErrorCode::PayloadTooLarge, message.into())
    }

    /// 415 媒体类型不支持。
    pub fn unsupported_media_type(message: impl Into<String>) -> Self {
        Self::with_code(ErrorCode::UnsupportedMediaType, message.into())
    }

    /// 429 触发限流。
    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::with_code(ErrorCode::RateLimited, message.into())
    }

    /// 500 内部错误；对外只给固定文案，细节仅进日志。
    pub fn internal_error() -> Self {
        Self::with_code(ErrorCode::InternalError, "internal error".to_owned())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0.error.code.status(), Json(self.0)).into_response()
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
