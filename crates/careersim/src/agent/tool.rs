//! 工具定义与注册表、控制动作 `complete_stage` 的参数与 Schema。

use std::{collections::HashMap, sync::Arc, time::Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument};

use super::session::{SessionId, ToolStatus, TurnId};
use crate::pipeline::{
    model::{BONUS_MAX_CHARS, OfferTermsInput, StageDefinition, StageKind},
    rubric::{DimensionScore, NOTE_MAX_CHARS, SUMMARY_MAX_CHARS},
};

/// 控制动作保留名；业务工具不得占用。
pub(crate) const CONTROL_ACTION_NAME: &str = "complete_stage";

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
    /// JSON Schema；服务端仍做完整校验。
    pub parameters: serde_json::Value,
    /// 是否请求供应商严格 Schema；由映射层结合部署能力决定最终取值。
    pub strict: bool,
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
    #[error("tool name `{0}` is reserved")]
    ReservedName(String),
    #[error("cancelled")]
    Cancelled,
}

/// 控制动作参数（plan/02 §6.3）；服务端在 `evaluate` 中做完整校验。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompleteStageArgs {
    /// 模型提交的维度分数与依据。
    pub dimensions: Vec<DimensionScore>,
    /// 阶段总评。
    pub summary: String,
    /// Offer 条款输入；仅 Offer 阶段必须提供。
    #[serde(default)]
    pub offer: Option<OfferTermsInput>,
}

/// 控制动作校验错误；`code()` 是返回给模型的机器码，不含内部细节。
#[derive(Debug, thiserror::Error)]
pub enum CompleteStageError {
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("invalid evaluation: {0}")]
    InvalidEvaluation(#[from] crate::pipeline::EvaluationError),
    #[error("invalid offer terms: {0}")]
    InvalidOffer(#[from] crate::pipeline::OfferError),
    #[error("stage not ready: at least {0} dialogue rounds are required")]
    StageNotReady(u8),
}

impl CompleteStageError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArguments(_) => "invalid_arguments",
            Self::InvalidEvaluation(_) => "invalid_evaluation",
            Self::InvalidOffer(_) => "invalid_offer",
            Self::StageNotReady(_) => "stage_not_ready",
        }
    }
}

/// 按阶段生成 `complete_stage` 的 ToolSpec；维度键枚举来自 rubric。
pub(crate) fn complete_stage_spec(stage: &StageDefinition) -> ToolSpec {
    let mut keys: Vec<String> = stage
        .rubric
        .dimensions
        .iter()
        .map(|dimension| dimension.key.clone())
        .collect();
    keys.sort();

    let mut properties = json!({
        "dimensions": {
            "type": "array",
            "items": {
                "type": "object",
                "additionalProperties": false,
                "required": ["key", "score", "note"],
                "properties": {
                    "key": { "type": "string", "enum": keys },
                    "score": { "type": "integer", "minimum": 0, "maximum": 100 },
                    "note": { "type": "string", "maxLength": NOTE_MAX_CHARS }
                }
            }
        },
        "summary": { "type": "string", "maxLength": SUMMARY_MAX_CHARS }
    });
    let mut required = vec!["dimensions", "summary"];

    if stage.kind == StageKind::Offer {
        properties["offer"] = json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["baseSalary", "months", "bonus"],
            "properties": {
                "baseSalary": { "type": "integer", "minimum": 0 },
                "months": { "type": "integer", "minimum": 12, "maximum": 24 },
                "bonus": { "type": "string", "maxLength": BONUS_MAX_CHARS }
            }
        });
        required.push("offer");
    }

    ToolSpec {
        name: CONTROL_ACTION_NAME.to_owned(),
        description: "提交当前阶段的评估结论；服务端计算总分并决定是否推进。".to_owned(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "required": required,
            "properties": properties
        }),
        strict: true,
    }
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

    pub fn register(&mut self, tool: Arc<dyn Tool>) -> Result<(), ToolError> {
        let name = tool.spec().name;
        if name == CONTROL_ACTION_NAME {
            return Err(ToolError::ReservedName(name));
        }
        self.tools.insert(name, tool);
        Ok(())
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

    #[instrument(
        name = "tool.execute",
        level = "debug",
        skip(self, call, cancel),
        fields(
            session_id = %session_id,
            turn_id = turn_id.0,
            tool_class = tracing::field::Empty,
            result = tracing::field::Empty,
            duration_ms = tracing::field::Empty
        )
    )]
    pub(crate) async fn execute(
        &self,
        call: &ToolCall,
        session_id: SessionId,
        turn_id: TurnId,
        cancel: &CancellationToken,
    ) -> (ToolStatus, String) {
        let started = Instant::now();
        let tool_class = if call.name == CONTROL_ACTION_NAME {
            "control"
        } else if self.tools.contains_key(&call.name) {
            "registered"
        } else {
            "unknown"
        };
        tracing::Span::current().record("tool_class", tool_class);
        let Some(tool) = self.tools.get(&call.name).cloned() else {
            let err = ToolError::UnknownTool(call.name.clone());
            let outcome = (ToolStatus::Error, err.to_string());
            record_tool_outcome(tool_class, outcome.0, started);
            return outcome;
        };

        let arguments = if call.arguments.trim().is_empty() {
            serde_json::Value::Null
        } else {
            match serde_json::from_str(&call.arguments) {
                Ok(value) => value,
                Err(err) => {
                    let err = ToolError::InvalidArguments(err.to_string());
                    let outcome = (ToolStatus::Error, err.to_string());
                    record_tool_outcome(tool_class, outcome.0, started);
                    return outcome;
                }
            }
        };

        let ctx = ToolContext {
            session_id,
            turn_id,
            cancel,
        };
        let outcome = match tool.call(arguments, &ctx).await {
            Ok(output) => (ToolStatus::Ok, output.content),
            Err(ToolError::Cancelled) => (ToolStatus::Error, "aborted".to_owned()),
            Err(err) => (ToolStatus::Error, err.to_string()),
        };
        record_tool_outcome(tool_class, outcome.0, started);
        outcome
    }
}

fn record_tool_outcome(tool_class: &'static str, status: ToolStatus, started: Instant) {
    let duration_ms = started.elapsed().as_secs_f64() * 1_000.0;
    tracing::Span::current().record("result", tracing::field::debug(status));
    tracing::Span::current().record("duration_ms", duration_ms);
    debug!(
        tool_class,
        result = ?status,
        duration_ms,
        "tool call completed"
    );
    let result = match status {
        ToolStatus::Ok => "ok",
        ToolStatus::Error => "error",
    };
    metrics::counter!(
        "careersim_tool_calls_total",
        "tool_class" => tool_class,
        "result" => result
    )
    .increment(1);
    metrics::histogram!("careersim_tool_duration_seconds", "tool_class" => tool_class)
        .record(started.elapsed().as_secs_f64());
}

#[cfg(test)]
pub(crate) mod testing;
