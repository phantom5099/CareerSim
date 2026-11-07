//! OpenAI 兼容供应商的端点与凭据约定。

use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProvider {
    OpenAi,
    DeepSeek,
    Glm,
}

impl ModelProvider {
    pub fn base_url(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::DeepSeek => "https://api.deepseek.com",
            Self::Glm => "https://open.bigmodel.cn/api/paas/v4",
        }
    }

    pub fn api_key_env(self) -> &'static str {
        match self {
            Self::OpenAi => "OPENAI_API_KEY",
            Self::DeepSeek => "DEEPSEEK_API_KEY",
            Self::Glm => "ZAI_API_KEY",
        }
    }
}

impl FromStr for ModelProvider {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "openai" => Ok(Self::OpenAi),
            "deepseek" => Ok(Self::DeepSeek),
            "glm" => Ok(Self::Glm),
            _ => Err(()),
        }
    }
}
