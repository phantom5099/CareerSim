//! 基于 `async-openai` 的 Chat Completions 适配层。

use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls,
    ChatCompletionRequestAssistantMessage, ChatCompletionRequestAssistantMessageContent,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestToolMessage,
    ChatCompletionRequestToolMessageContent, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent, ChatCompletionTool, ChatCompletionTools,
    CreateChatCompletionRequest, CreateChatCompletionRequestArgs, FunctionCall, FunctionObject,
};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::client::ChatRequest;
use super::client::ChatResponse;
use super::client::LlmClient;
use super::client::LlmError;
use super::client::TokenUsage;
use crate::agent::Message;
use crate::agent::ToolCall;
use crate::agent::ToolSpec;
use crate::config::LlmConfig;

pub struct OpenAiCompatibleClient {
    client: Client<OpenAIConfig>,
    config: LlmConfig,
}

impl OpenAiCompatibleClient {
    pub fn new(config: LlmConfig, http: reqwest::Client) -> Self {
        let sdk = OpenAIConfig::new()
            .with_api_base(config.base_url.trim_end_matches('/'))
            .with_api_key(&config.api_key);
        Self {
            client: Client::with_config(sdk).with_http_client(http),
            config,
        }
    }

    fn build_request(&self, request: ChatRequest) -> Result<CreateChatCompletionRequest, LlmError> {
        let messages = request
            .messages
            .into_iter()
            .map(to_sdk_message)
            .collect::<Vec<_>>();

        let mut builder = CreateChatCompletionRequestArgs::default();
        builder
            .model(self.config.model.clone())
            .messages(messages)
            .temperature(self.config.temperature)
            .max_tokens(self.config.max_tokens);

        if !request.tools.is_empty() {
            builder.tools(
                request
                    .tools
                    .into_iter()
                    .map(to_sdk_tool)
                    .collect::<Vec<_>>(),
            );
        }

        builder
            .build()
            .map_err(|err| LlmError::api_with_source("build request", err))
    }
}

#[async_trait]
impl LlmClient for OpenAiCompatibleClient {
    async fn complete(
        &self,
        request: ChatRequest,
        cancel: &CancellationToken,
    ) -> Result<ChatResponse, LlmError> {
        let request = self.build_request(request)?;

        let chat = self.client.chat();
        let response = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(LlmError::Cancelled),
            result = chat.create(request) => {
                result.map_err(|err| LlmError::api_with_source("send request", err))?
            }
        };

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| LlmError::api("response contains no choices"))?;

        let mut tool_calls = Vec::new();
        for call in choice.message.tool_calls.unwrap_or_default() {
            match call {
                ChatCompletionMessageToolCalls::Function(call) => tool_calls.push(ToolCall {
                    call_id: call.id,
                    name: call.function.name,
                    arguments: call.function.arguments,
                }),
                ChatCompletionMessageToolCalls::Custom(_) => {
                    return Err(LlmError::api("custom tool calls are not supported"));
                }
            }
        }

        Ok(ChatResponse {
            content: choice.message.content.unwrap_or_default(),
            tool_calls,
            usage: response.usage.map(|usage| TokenUsage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
            }),
        })
    }
}

fn to_sdk_message(message: Message) -> ChatCompletionRequestMessage {
    match message {
        Message::System { content } => {
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(content),
                name: None,
            })
        }
        Message::User { content } => {
            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(content),
                name: None,
            })
        }
        Message::Assistant {
            content,
            tool_calls,
        } => {
            let tool_calls = if tool_calls.is_empty() {
                None
            } else {
                Some(
                    tool_calls
                        .into_iter()
                        .map(|call| {
                            ChatCompletionMessageToolCalls::Function(
                                ChatCompletionMessageToolCall {
                                    id: call.call_id,
                                    function: FunctionCall {
                                        name: call.name,
                                        arguments: call.arguments,
                                    },
                                },
                            )
                        })
                        .collect(),
                )
            };

            // 只发起工具调用时，OpenAI 期望 assistant 的 content 为空。
            let content = if content.is_empty() && tool_calls.is_some() {
                None
            } else {
                Some(ChatCompletionRequestAssistantMessageContent::Text(content))
            };

            ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                content,
                tool_calls,
                ..Default::default()
            })
        }
        Message::Tool {
            content,
            tool_call_id,
        } => ChatCompletionRequestMessage::Tool(ChatCompletionRequestToolMessage {
            content: ChatCompletionRequestToolMessageContent::Text(content),
            tool_call_id,
        }),
    }
}

fn to_sdk_tool(spec: ToolSpec) -> ChatCompletionTools {
    ChatCompletionTools::Function(ChatCompletionTool {
        function: FunctionObject {
            name: spec.name,
            description: Some(spec.description),
            parameters: Some(spec.parameters),
            strict: None,
        },
    })
}
