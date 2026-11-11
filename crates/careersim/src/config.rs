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
    /// 请求结构化输出时是否附带 `response_format: json_schema`。
    pub strict_schema: bool,
    /// 请求结构化输出时是否强制 `tool_choice`；两者按模型能力组合。
    pub forced_tool_choice: bool,
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
        let strict_schema = optional_parse("MODEL_STRICT_SCHEMA", false)?;
        let forced_tool_choice = optional_parse("MODEL_FORCED_TOOL_CHOICE", false)?;

        Ok(Self {
            provider,
            model,
            base_url,
            api_key,
            temperature,
            max_tokens,
            strict_schema,
            forced_tool_choice,
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
    pub http: HttpConfig,
    pub argon2: Argon2Config,
    pub resume_storage_dir: String,
    pub max_steps: usize,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let db = DbConfig::from_env()?;
        let llm = LlmConfig::from_env()?;
        let http = HttpConfig::from_env()?;
        let argon2 = Argon2Config::from_env()?;
        let resume_storage_dir = match std::env::var("RESUME_STORAGE_DIR") {
            Ok(value) if !value.trim().is_empty() => value,
            _ => "data/resumes".to_owned(),
        };
        let max_steps = optional_parse("MAX_STEPS", 8_usize)?;
        if max_steps == 0 {
            return Err(ConfigError::Invalid {
                name: "MAX_STEPS",
                value: max_steps.to_string(),
            });
        }

        Ok(Self {
            db,
            llm,
            http,
            argon2,
            resume_storage_dir,
            max_steps,
        })
    }
}

/// HTTP 绑定与 Cookie 安全选项。
///
/// `allowed_origins` 为逗号分隔列表；非 GET/HEAD/OPTIONS 请求会校验 `Origin` 头。
#[derive(Clone)]
pub struct HttpConfig {
    pub bind: String,
    /// 生产默认 true；本地开发可关闭（HttpOnly 恒开启，不受此开关影响）。
    pub cookie_secure: bool,
    pub allowed_origins: Vec<String>,
}

impl HttpConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind = match std::env::var("HTTP_BIND") {
            Ok(value) if !value.trim().is_empty() => value,
            _ => "127.0.0.1:3000".to_owned(),
        };
        let cookie_secure = optional_parse("HTTP_COOKIE_SECURE", true)?;
        let allowed_origins = required("HTTP_ALLOWED_ORIGINS")?
            .split(',')
            .map(|item| item.trim().to_owned())
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>();
        if allowed_origins.is_empty() {
            return Err(ConfigError::Invalid {
                name: "HTTP_ALLOWED_ORIGINS",
                value: String::new(),
            });
        }

        Ok(Self {
            bind,
            cookie_secure,
            allowed_origins,
        })
    }
}

/// Argon2id 参数（OWASP 推荐默认：m=19 MiB, t=2, p=1）。
#[derive(Clone, Copy, Debug)]
pub struct Argon2Config {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl Argon2Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let m_cost_kib = optional_parse("ARGON2_M_COST_KIB", 19_456_u32)?;
        let t_cost = optional_parse("ARGON2_T_COST", 2_u32)?;
        let p_cost = optional_parse("ARGON2_P_COST", 1_u32)?;
        if m_cost_kib < 19_456 || t_cost == 0 || p_cost == 0 {
            return Err(ConfigError::Invalid {
                name: "ARGON2_*",
                value: format!("m={m_cost_kib}, t={t_cost}, p={p_cost}"),
            });
        }

        Ok(Self {
            m_cost_kib,
            t_cost,
            p_cost,
        })
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
