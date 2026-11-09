//! CareerSim HTTP 服务：演示账号认证与后续模拟业务接口。

pub mod auth;
pub mod config;
pub mod domain;
pub mod error;
pub mod http;
pub mod state;

pub use http::build_router;
