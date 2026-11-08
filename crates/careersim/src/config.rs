//! 配置读取。

use std::str::FromStr;
use std::time::Duration;

use crate::error::ConfigError;
use crate::llm::provider::ModelProvider;

/// 模型连接与采样参数；由 `OpenAiCompatibleClient` 在构造时固定。
///
/// 刻意不实现 `Debug`，避免 API Key 进入日志或错误信息。
pub struct LlmConfig {
    pub provider: ModelProvider,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl LlmConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let provider_name = required("MODEL_PROVIDER")?;
        let provider =
            ModelProvider::from_str(&provider_name).map_err(|()| ConfigError::Invalid {
                name: "MODEL_PROVIDER",
                value: provider_name.clone(),
            })?;

        let model = required("MODEL")?;

        let base_url = match std::env::var("MODEL_BASE_URL") {
            Ok(value) if !value.trim().is_empty() => value,
            _ => provider.base_url().to_owned(),
        };

        let api_key = required(provider.api_key_env())?;

        let temperature = optional_parse("MODEL_TEMPERATURE", 0.7_f32)?;
        if !temperature.is_finite() || !(0.0..=2.0).contains(&temperature) {
            return Err(ConfigError::Invalid {
                name: "MODEL_TEMPERATURE",
                value: temperature.to_string(),
            });
        }

        let max_tokens = optional_parse("MODEL_MAX_TOKENS", 2048_u32)?;
        if max_tokens == 0 {
            return Err(ConfigError::Invalid {
                name: "MODEL_MAX_TOKENS",
                value: max_tokens.to_string(),
            });
        }

        Ok(Self {
            provider,
            model,
            base_url,
            api_key,
            temperature,
            max_tokens,
        })
    }
}

/// 数据库连接参数；连接串可能包含密码，因此不实现 `Debug`。
#[derive(Clone)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
}

impl DbConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let url = required("DATABASE_URL")?;
        let max_connections = optional_parse("DATABASE_MAX_CONNECTIONS", 10_u32)?;
        if max_connections == 0 {
            return Err(ConfigError::Invalid {
                name: "DATABASE_MAX_CONNECTIONS",
                value: max_connections.to_string(),
            });
        }
        let acquire_timeout_secs = optional_parse("DATABASE_ACQUIRE_TIMEOUT_SECS", 5_u64)?;

        Ok(Self {
            url,
            max_connections,
            acquire_timeout: Duration::from_secs(acquire_timeout_secs),
        })
    }
}

pub struct AppConfig {
    pub db: DbConfig,
    pub llm: LlmConfig,
    pub max_steps: usize,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let db = DbConfig::from_env()?;
        let llm = LlmConfig::from_env()?;
        let max_steps = optional_parse("MAX_STEPS", 8_usize)?;
        if max_steps == 0 {
            return Err(ConfigError::Invalid {
                name: "MAX_STEPS",
                value: max_steps.to_string(),
            });
        }

        Ok(Self { db, llm, max_steps })
    }
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        _ => Err(ConfigError::Missing(name)),
    }
}

fn optional_parse<T>(name: &'static str, default: T) -> Result<T, ConfigError>
where
    T: FromStr,
{
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => {
            value.trim().parse::<T>().map_err(|_| ConfigError::Invalid {
                name,
                value: value.clone(),
            })
        }
        _ => Ok(default),
    }
}
