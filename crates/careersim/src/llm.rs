//! 模型适配层：OpenAI Chat Completions 兼容客户端。

pub mod client;
#[cfg(test)]
pub mod mock;
pub mod openai_compat;
pub mod provider;

pub use client::{ChatRequest, ChatResponse, LlmClient, LlmError, TokenUsage, ToolChoice};
pub use openai_compat::OpenAiCompatibleClient;
pub use provider::ModelProvider;
