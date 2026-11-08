//! 从事件日志投影模型上下文。首版注入完整历史。

use async_trait::async_trait;

use super::Message;
use super::Session;
use super::SessionEventKind;
use super::ToolSpec;
use crate::llm::client::ChatRequest;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildOptions {
    /// 首版不支持 token 预算；为 `Some` 时直接报错，不做静默截断。
    pub max_prompt_tokens: Option<usize>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ContextError {
    #[error("max_prompt_tokens is not supported")]
    BudgetUnsupported,
}

#[async_trait]
pub trait ContextBuilder: Send + Sync {
    async fn build(
        &self,
        session: &Session,
        tools: &[ToolSpec],
        opts: BuildOptions,
    ) -> Result<ChatRequest, ContextError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultContextBuilder;

#[async_trait]
impl ContextBuilder for DefaultContextBuilder {
    async fn build(
        &self,
        session: &Session,
        tools: &[ToolSpec],
        opts: BuildOptions,
    ) -> Result<ChatRequest, ContextError> {
        if opts.max_prompt_tokens.is_some() {
            return Err(ContextError::BudgetUnsupported);
        }

        let mut messages = Vec::with_capacity(session.events.len() + 1);
        messages.push(Message::system(session.role().system_prompt()));

        for (index, event) in session.events.iter().enumerate() {
            match &event.kind {
                SessionEventKind::UserMessage { content, .. } => {
                    messages.push(Message::user(content.clone()));
                }
                SessionEventKind::AssistantMessage {
                    content,
                    tool_calls,
                    ..
                } => {
                    messages.push(Message::assistant(content.clone(), tool_calls.clone()));
                    // 工具结果紧跟对应的 assistant 消息；缺失结果补 aborted，
                    // 避免模型接口拒绝整段历史。只在本条消息之后查找，
                    // 同名 call id 在更晚回合出现时不会串到旧消息上。
                    let following = session.events[index + 1..]
                        .iter()
                        .map(|event| &event.kind)
                        .take_while(|kind| matches!(kind, SessionEventKind::ToolResult { .. }))
                        .collect::<Vec<_>>();
                    for call in tool_calls {
                        let content = following
                            .iter()
                            .find_map(|kind| match kind {
                                SessionEventKind::ToolResult {
                                    tool_call_id,
                                    output,
                                    ..
                                } if tool_call_id == &call.call_id => Some(output.as_str()),
                                _ => None,
                            })
                            .unwrap_or("aborted");
                        messages.push(Message::tool(content.to_owned(), call.call_id.clone()));
                    }
                }
                // 孤儿 tool_result 不进入上下文；生命周期事件不进入上下文。
                SessionEventKind::ToolResult { .. }
                | SessionEventKind::SessionMeta { .. }
                | SessionEventKind::TurnStarted { .. }
                | SessionEventKind::TurnEnded { .. }
                | SessionEventKind::StageCompleted { .. } => {}
            }
        }

        Ok(ChatRequest {
            messages,
            tools: tools.to_vec(),
        })
    }
}
