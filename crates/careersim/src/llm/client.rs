//! 模型客户端接口与供应商无关的请求/响应类型。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::agent::Message;
use crate::agent::ToolCall;
use crate::agent::ToolSpec;

/// 工具选择策略；评估轮强制指定 `Function`。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ToolChoice {
    /// 模型自行决定是否调用工具。
    #[default]
    Auto,
    /// 强制调用指定函数；评估轮指向 `complete_stage`。
    Function { name: String },
}

/// 供应商无关的模型请求；模型名与采样参数由客户端构造时固定。
#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    /// 工具选择策略；`None` 时按 `Auto` 处理。
    pub tool_choice: ToolChoice,
    /// 采样温度；评估轮调低（如 0.2），`None` 时用部署配置。
    pub temperature: Option<f32>,
    /// 输出 token 上限；`None` 时用部署配置。
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("request cancelled")]
    Cancelled,
    #[error("model request failed: {message}")]
    Api {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl LlmError {
    pub fn api(message: impl Into<String>) -> Self {
        Self::Api {
            message: message.into(),
            source: None,
        }
    }

    pub fn api_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Api {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(
        &self,
        request: ChatRequest,
        cancel: &CancellationToken,
    ) -> Result<ChatResponse, LlmError>;
}
