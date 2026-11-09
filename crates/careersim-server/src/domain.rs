//! 业务领域类型；与 HTTP 请求/响应分离，后续可迁入共享 crate。

pub mod user;

pub use user::User;
