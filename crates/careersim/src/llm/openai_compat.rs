//! 基于 `async-openai` 的 Chat Completions 适配层。

use std::time::Instant;

use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls,
        ChatCompletionNamedToolChoice, ChatCompletionRequestAssistantMessage,
        ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestSystemMessageContent,
        ChatCompletionRequestToolMessage, ChatCompletionRequestToolMessageContent,
        ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
        ChatCompletionTool, ChatCompletionToolChoiceOption, ChatCompletionTools,
        CreateChatCompletionRequest, CreateChatCompletionRequestArgs, FunctionCall, FunctionName,
        FunctionObject, ToolChoiceOptions,
    },
};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::{Span, debug, error, field, instrument};

use super::client::{ChatRequest, ChatResponse, LlmClient, LlmError, TokenUsage, ToolChoice};
use crate::{
    agent::{
        message::Message,
        tool::{ToolCall, ToolSpec},
    },
    config::LlmConfig,
    llm::provider::ModelProvider,
};

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
            .temperature(request.temperature.unwrap_or(self.config.temperature))
            .max_tokens(request.max_tokens.unwrap_or(self.config.max_tokens));

        if !request.tools.is_empty() {
            builder.tools(
                request
                    .tools
                    .iter()
                    .map(|spec| to_sdk_tool(spec.clone(), self.config.strict_schema))
                    .collect::<Vec<_>>(),
            );
            builder.tool_choice(self.to_sdk_tool_choice(&request.tool_choice));
        }

        builder
            .build()
            .map_err(|err| LlmError::api_with_source("build request", err))
    }

    /// 领域 `ToolChoice` → SDK 映射；供应商不支持强制选择时按部署配置降级为
    /// `Auto`（能力由配置决定，不在每次调用试错）。
    fn to_sdk_tool_choice(&self, choice: &ToolChoice) -> ChatCompletionToolChoiceOption {
        match choice {
            ToolChoice::Auto => ChatCompletionToolChoiceOption::Mode(ToolChoiceOptions::Auto),
            ToolChoice::Function { name } => {
                if self.config.forced_tool_choice {
                    ChatCompletionToolChoiceOption::Function(ChatCompletionNamedToolChoice {
                        function: FunctionName { name: name.clone() },
                    })
                } else {
                    debug!(
                        tool = %name,
                        "MODEL_FORCED_TOOL_CHOICE is disabled; falling back to auto tool choice"
                    );
                    ChatCompletionToolChoiceOption::Mode(ToolChoiceOptions::Auto)
                }
            }
        }
    }
}

#[async_trait]
impl LlmClient for OpenAiCompatibleClient {
    #[instrument(
        name = "llm.complete",
        level = "debug",
        skip(self, request, cancel),
        fields(
            provider = ?self.config.provider,
            model = %self.config.model,
            tool_count = request.tools.len(),
            result = field::Empty,
            duration_ms = field::Empty
        )
    )]
    async fn complete(
        &self,
        request: ChatRequest,
        cancel: &CancellationToken,
    ) -> Result<ChatResponse, LlmError> {
        let started = Instant::now();
        let request = match self.build_request(request) {
            Ok(request) => request,
            Err(error) => {
                record_llm_metrics(self.config.provider, "error", started, None);
                return Err(error);
            }
        };

        let chat = self.client.chat();
        let response = tokio::select! {
            biased;
            () = cancel.cancelled() => {
                record_llm_metrics(self.config.provider, "cancelled", started, None);
                return Err(LlmError::Cancelled);
            },
            result = chat.create(request) => {
                result.map_err(|err| {
                    error!(error_kind = "api", "model provider request failed");
                    LlmError::api_with_source("send request", err)
                })
            }
        };
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                record_llm_metrics(self.config.provider, "error", started, None);
                return Err(error);
            }
        };

        let choice = match response.choices.into_iter().next() {
            Some(choice) => choice,
            None => {
                record_llm_metrics(self.config.provider, "error", started, None);
                return Err(LlmError::api("response contains no choices"));
            }
        };

        let mut tool_calls = Vec::new();
        for call in choice.message.tool_calls.unwrap_or_default() {
            match call {
                ChatCompletionMessageToolCalls::Function(call) => tool_calls.push(ToolCall {
                    call_id: call.id,
                    name: call.function.name,
                    arguments: call.function.arguments,
                }),
                ChatCompletionMessageToolCalls::Custom(_) => {
                    record_llm_metrics(self.config.provider, "error", started, None);
                    return Err(LlmError::api("custom tool calls are not supported"));
                }
            }
        }

        let result = ChatResponse {
            content: choice.message.content.unwrap_or_default(),
            tool_calls,
            usage: response.usage.map(|usage| TokenUsage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
            }),
        };
        let duration_ms = started.elapsed().as_secs_f64() * 1_000.0;
        tracing::Span::current().record("result", "success");
        tracing::Span::current().record("duration_ms", duration_ms);
        debug!(
            duration_ms,
            tool_calls = result.tool_calls.len(),
            input_tokens = result.usage.map(|usage| usage.input_tokens),
            output_tokens = result.usage.map(|usage| usage.output_tokens),
            "model request completed"
        );
        record_llm_metrics(self.config.provider, "ok", started, result.usage);
        Ok(result)
    }
}

fn record_llm_metrics(
    provider: ModelProvider,
    result: &'static str,
    started: Instant,
    usage: Option<TokenUsage>,
) {
    let provider = format!("{provider:?}");
    let duration_ms = started.elapsed().as_secs_f64() * 1_000.0;
    Span::current().record("result", result);
    Span::current().record("duration_ms", duration_ms);
    metrics::counter!(
        "careersim_llm_requests_total",
        "provider" => provider.clone(),
        "result" => result
    )
    .increment(1);
    metrics::histogram!(
        "careersim_llm_request_duration_seconds",
        "provider" => provider.clone()
    )
    .record(started.elapsed().as_secs_f64());
    if let Some(usage) = usage {
        metrics::counter!(
            "careersim_llm_tokens_total",
            "provider" => provider.clone(),
            "direction" => "input"
        )
        .increment(u64::from(usage.input_tokens));
        metrics::counter!(
            "careersim_llm_tokens_total",
            "provider" => provider,
            "direction" => "output"
        )
        .increment(u64::from(usage.output_tokens));
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

fn to_sdk_tool(spec: ToolSpec, strict_schema: bool) -> ChatCompletionTools {
    ChatCompletionTools::Function(ChatCompletionTool {
        function: FunctionObject {
            name: spec.name,
            description: Some(spec.description),
            parameters: Some(spec.parameters),
            // 严格 Schema 只在部署声明支持时发送；服务端始终完整校验。
            strict: (spec.strict && strict_schema).then_some(true),
        },
    })
}
