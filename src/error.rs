//! 进程级配置错误。

/// 配置读取失败。错误信息只包含变量名或非法值，不包含 API Key。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("missing required environment variable `{0}`")]
    Missing(&'static str),
    #[error("invalid value for `{name}`: {value}")]
    Invalid { name: &'static str, value: String },
}
