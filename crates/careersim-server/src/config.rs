//! 服务配置。演示凭据只存在于 demo 模式，不实现 `Debug` 以免进入日志。

use std::net::SocketAddr;
use std::str::FromStr;

use careersim::error::ConfigError;

pub const DEFAULT_DEMO_USERNAME: &str = "admin";
pub const DEFAULT_DEMO_PASSWORD: &str = "123456";
const DEFAULT_HTTP_ADDR: &str = "127.0.0.1:3000";
const DEFAULT_CORS_ORIGINS: &str = "http://localhost:5175,http://127.0.0.1:5175";
const DEFAULT_TOKEN_TTL_SECS: &str = "604800";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Demo,
    Live,
}

pub struct ServerConfig {
    pub mode: AppMode,
    pub http_addr: SocketAddr,
    pub cors_origins: Vec<String>,
    pub demo_username: Option<String>,
    pub demo_password: Option<String>,
    pub token_ttl: chrono::Duration,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let mode = match optional("APP_MODE", "demo".to_owned())
            .to_ascii_lowercase()
            .as_str()
        {
            "demo" => AppMode::Demo,
            "live" => AppMode::Live,
            other => {
                return Err(ConfigError::Invalid {
                    name: "APP_MODE",
                    value: other.to_owned(),
                });
            }
        };

        let http_addr = parse("HTTP_ADDR", DEFAULT_HTTP_ADDR)?;
        let cors_origins = optional("CORS_ORIGINS", DEFAULT_CORS_ORIGINS.to_owned())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(str::to_owned)
            .collect();

        let (demo_username, demo_password) = match mode {
            AppMode::Demo => {
                let username = optional("DEMO_USERNAME", DEFAULT_DEMO_USERNAME.to_owned());
                let password = optional("DEMO_PASSWORD", DEFAULT_DEMO_PASSWORD.to_owned());
                if username.trim().is_empty() || password.is_empty() {
                    return Err(ConfigError::Invalid {
                        name: if username.trim().is_empty() {
                            "DEMO_USERNAME"
                        } else {
                            "DEMO_PASSWORD"
                        },
                        value: String::new(),
                    });
                }
                (Some(username), Some(password))
            }
            AppMode::Live => (None, None),
        };

        let token_ttl_secs = parse::<u64>("AUTH_TOKEN_TTL_SECS", DEFAULT_TOKEN_TTL_SECS)?;
        let token_ttl = i64::try_from(token_ttl_secs)
            .ok()
            .filter(|secs| *secs > 0)
            .and_then(chrono::Duration::try_seconds)
            .ok_or_else(|| ConfigError::Invalid {
                name: "AUTH_TOKEN_TTL_SECS",
                value: token_ttl_secs.to_string(),
            })?;

        Ok(Self {
            mode,
            http_addr,
            cors_origins,
            demo_username,
            demo_password,
            token_ttl,
        })
    }
}

fn optional(name: &'static str, default: String) -> String {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => default,
    }
}

fn parse<T>(name: &'static str, default: &str) -> Result<T, ConfigError>
where
    T: FromStr,
{
    let value = optional(name, default.to_owned());
    value
        .trim()
        .parse::<T>()
        .map_err(|_| ConfigError::Invalid { name, value })
}
