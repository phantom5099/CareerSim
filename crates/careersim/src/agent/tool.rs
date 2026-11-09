//! 工具定义与注册表。首版生产注册表为空。

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use super::SessionId;
use super::TurnId;
use super::session::ToolStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    /// 模型返回的原始 JSON 字符串，由工具自行解析。
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema；首版不做额外校验。
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    pub content: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("unknown tool: {0}")]
    UnknownTool(String),
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("tool failed: {0}")]
    Failed(String),
    #[error("cancelled")]
    Cancelled,
}

pub struct ToolContext<'a> {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub cancel: &'a CancellationToken,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;

    async fn call(
        &self,
        arguments: serde_json::Value,
        ctx: &ToolContext<'_>,
    ) -> Result<ToolOutput, ToolError>;
}

/// 工具注册表；生产环境不注册任何工具。
#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.spec().name;
        self.tools.insert(name, tool);
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        let mut specs: Vec<ToolSpec> = self.tools.values().map(|tool| tool.spec()).collect();
        // 固定顺序，避免同一会话的不同步骤拿到不同顺序的工具定义。
        specs.sort_by(|a, b| a.name.cmp(&b.name));
        specs
    }

    pub(crate) async fn execute(
        &self,
        call: &ToolCall,
        session_id: SessionId,
        turn_id: TurnId,
        cancel: &CancellationToken,
    ) -> (ToolStatus, String) {
        let Some(tool) = self.tools.get(&call.name).cloned() else {
            let err = ToolError::UnknownTool(call.name.clone());
            return (ToolStatus::Error, err.to_string());
        };

        let arguments = if call.arguments.trim().is_empty() {
            serde_json::Value::Null
        } else {
            match serde_json::from_str(&call.arguments) {
                Ok(value) => value,
                Err(err) => {
                    let err = ToolError::InvalidArguments(err.to_string());
                    return (ToolStatus::Error, err.to_string());
                }
            }
        };

        let ctx = ToolContext {
            session_id,
            turn_id,
            cancel,
        };

        match tool.call(arguments, &ctx).await {
            Ok(output) => (ToolStatus::Ok, output.content),
            Err(ToolError::Cancelled) => (ToolStatus::Error, "aborted".to_owned()),
            Err(err) => (ToolStatus::Error, err.to_string()),
        }
    }
}

#[cfg(test)]
pub(crate) mod testing;
