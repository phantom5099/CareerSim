//! HTTP 层：路由、认证提取与统一错误体。

pub mod account_routes;
pub mod application_routes;
pub mod auth;
pub mod catalog_routes;
pub mod error;
pub mod resume_routes;
pub mod router;

use chrono::{DateTime, SecondsFormat, Utc};

/// HTTP 时间字段统一使用带毫秒的 RFC3339 字符串。
pub(crate) fn format_timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}
