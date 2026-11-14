//! 上下文投影使用的消息；不落盘，也不是会话的第二份快照。

use serde::{Deserialize, Serialize};

use super::tool::ToolCall;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        content: String,
        tool_call_id: String,
    },
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self::System {
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::User {
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self::Assistant {
            content: content.into(),
            tool_calls,
        }
    }

    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self::Tool {
            content: content.into(),
            tool_call_id: tool_call_id.into(),
        }
    }

    /// 消息角色标签；投影分组与预算估算用。
    pub fn role_kind(&self) -> &'static str {
        match self {
            Self::System { .. } => "system",
            Self::User { .. } => "user",
            Self::Assistant { .. } => "assistant",
            Self::Tool { .. } => "tool",
        }
    }

    /// 消息文本内容；预算估算用（工具调用列表不计）。
    pub fn text(&self) -> &str {
        match self {
            Self::System { content }
            | Self::User { content }
            | Self::Tool { content, .. }
            | Self::Assistant { content, .. } => content,
        }
    }
}
