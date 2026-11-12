//! 会话回合循环：per-session 互斥、对话/评估两种回合模式、控制动作
//! `complete_stage` 的识别与一次修复、取消与阶段推进（plan/02 §8）。

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use serde_json::json;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use super::BuildOptions;
use super::CompleteStageBatch;
use super::ContextBuilder;
use super::Session;
use super::SessionError;
use super::SessionId;
use super::SessionService;
use super::StageCompletion;
use super::StageEvaluationRecord;
use super::ToolCall;
use super::ToolRegistry;
use super::ToolStatus;
use super::TurnContext;
use super::TurnEndReason;
use super::TurnId;
use super::session::assistant_count_in_stage;
use super::session::user_round_in_stage;
use super::tool::CONTROL_ACTION_NAME;
use super::tool::CompleteStageArgs;
use super::tool::CompleteStageError;
use super::tool::complete_stage_spec;
use crate::error::error_chain;
use crate::llm::LlmClient;
use crate::llm::LlmError;
use crate::llm::ToolChoice;
use crate::pipeline::StageKind;
use crate::pipeline::StageOutcome;
use crate::pipeline::build_offer_terms;
use crate::pipeline::evaluate;
use crate::pipeline::plan_transition;

/// 评估轮采样温度（plan/02 §7.5）；低于对话轮，减少评分抖动。
const EVALUATION_TEMPERATURE: f32 = 0.2;

/// 非法评估输出的修复次数上限（plan/02 §6.4）；`stage_not_ready` 不消耗。
const MAX_REPAIRS: usize = 1;

/// 循环参数；模型连接与采样参数由 `LlmClient` 在构造时固定。
#[derive(Debug, Clone, Copy)]
pub struct LoopConfig {
    pub max_steps: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TurnResult {
    pub turn_id: TurnId,
    pub reason: TurnEndReason,
    pub steps: u32,
    pub output: String,
    pub error: Option<String>,
    /// 评估结论与阶段转移；仅评估成功时存在。
    pub stage_completion: Option<StageCompletion>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoopError {
    #[error("session is busy")]
    SessionBusy,
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error("turn context stage `{context}` does not match session stage `{session}`")]
    StageMismatch { context: String, session: String },
}

/// 非法输出的机器码载荷；不含内部细节。
fn error_payload(code: &str, message: &str) -> String {
    json!({ "code": code, "message": message }).to_string()
}

pub struct AgentLoop {
    llm: Arc<dyn LlmClient>,
    context: Arc<dyn ContextBuilder>,
    tools: Arc<ToolRegistry>,
    sessions: Arc<dyn SessionService>,
    config: LoopConfig,
    // std 锁只用于查找/插入，不跨 await；每个会话的回合锁是 tokio 异步锁
    session_locks: std::sync::Mutex<HashMap<SessionId, Arc<Mutex<()>>>>,
}

impl AgentLoop {
    pub fn new(
        llm: Arc<dyn LlmClient>,
        context: Arc<dyn ContextBuilder>,
        tools: Arc<ToolRegistry>,
        sessions: Arc<dyn SessionService>,
        config: LoopConfig,
    ) -> Self {
        Self {
            llm,
            context,
            tools,
            sessions,
            config,
            session_locks: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub async fn run_turn(
        &self,
        id: SessionId,
        input: impl Into<String>,
        ctx: &TurnContext,
        cancel: CancellationToken,
    ) -> Result<TurnResult, LoopError> {
        let lock = self.session_lock(id);
        let _guard = match lock.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(LoopError::SessionBusy),
        };

        let mut session = self.sessions.load(id).await?;
        // §8.1：会话当前阶段必须与 TurnContext 一致（key 与 kind 都要一致）；
        // 同 key 改 kind 时事件 kind 必须反映实际所用 rubric 的阶段，不猜测。
        if session.current_stage.key != ctx.stage.key
            || session.current_stage.kind != ctx.stage.kind
            || ctx.pipeline.get(&ctx.stage.key) != Some(&ctx.stage)
        {
            return Err(LoopError::StageMismatch {
                context: format!("{}:{}", ctx.stage.key, ctx.stage.kind.as_str()),
                session: format!(
                    "{}:{}",
                    session.current_stage.key,
                    session.current_stage.kind.as_str()
                ),
            });
        }

        let turn_id = self.sessions.start_turn(&mut session).await?;
        info!(
            session_id = %id,
            turn_id = turn_id.0,
            stage = %session.current_stage.key,
            role = %session.role(),
            "turn started"
        );

        if cancel.is_cancelled() {
            return self
                .finish_turn(
                    &mut session,
                    turn_id,
                    TurnEndReason::Interrupted,
                    0,
                    String::new(),
                    None,
                )
                .await;
        }

        self.sessions
            .record_user_message(&mut session, turn_id, input.into())
            .await?;

        let control_spec = complete_stage_spec(&ctx.stage);
        let mut steps: u32 = 0;
        let mut output = String::new();
        let mut repairs_left = MAX_REPAIRS;

        for _ in 0..self.config.max_steps {
            if cancel.is_cancelled() {
                return self
                    .finish_turn(
                        &mut session,
                        turn_id,
                        TurnEndReason::Interrupted,
                        steps,
                        output,
                        None,
                    )
                    .await;
            }

            // §8.2：本次响应后助手消息数将超过 max_rounds 时进入评估轮。
            let evaluation_mode =
                assistant_count_in_stage(&session) + 1 > usize::from(ctx.stage.max_rounds);
            let tools = if evaluation_mode {
                vec![control_spec.clone()]
            } else {
                let mut specs = self.tools.specs();
                specs.push(control_spec.clone());
                specs.sort_by(|a, b| a.name.cmp(&b.name));
                specs
            };

            debug!(
                session_id = %id,
                turn_id = turn_id.0,
                step = steps + 1,
                role = %session.role(),
                evaluation_mode,
                "requesting model"
            );

            let mut request = match self
                .context
                .build(&session, ctx, &tools, BuildOptions::default())
                .await
            {
                Ok(request) => request,
                Err(err) => {
                    return self
                        .finish_turn(
                            &mut session,
                            turn_id,
                            TurnEndReason::Error,
                            steps,
                            output,
                            Some(error_chain(&err)),
                        )
                        .await;
                }
            };
            if evaluation_mode {
                request.tool_choice = ToolChoice::Function {
                    name: CONTROL_ACTION_NAME.to_owned(),
                };
                request.temperature = Some(EVALUATION_TEMPERATURE);
            }

            let response = match self.llm.complete(request, &cancel).await {
                Ok(response) => response,
                Err(LlmError::Cancelled) => {
                    return self
                        .finish_turn(
                            &mut session,
                            turn_id,
                            TurnEndReason::Interrupted,
                            steps,
                            output,
                            None,
                        )
                        .await;
                }
                Err(err) => {
                    if cancel.is_cancelled() {
                        return self
                            .finish_turn(
                                &mut session,
                                turn_id,
                                TurnEndReason::Interrupted,
                                steps,
                                output,
                                None,
                            )
                            .await;
                    }
                    return self
                        .finish_turn(
                            &mut session,
                            turn_id,
                            TurnEndReason::Error,
                            steps,
                            output,
                            Some(error_chain(&err)),
                        )
                        .await;
                }
            };

            steps += 1;
            output = response.content.clone();

            if response.tool_calls.is_empty() {
                self.sessions
                    .record_assistant_message(
                        &mut session,
                        turn_id,
                        response.content.clone(),
                        Vec::new(),
                        response.usage,
                    )
                    .await?;
                if response.content.trim().is_empty() {
                    return self
                        .finish_turn(
                            &mut session,
                            turn_id,
                            TurnEndReason::Error,
                            steps,
                            output,
                            Some("empty response".to_owned()),
                        )
                        .await;
                }
                return self
                    .finish_turn(
                        &mut session,
                        turn_id,
                        TurnEndReason::Done,
                        steps,
                        output,
                        None,
                    )
                    .await;
            }

            // 控制动作识别（§6.3 第 1 条）。
            let control_index = response
                .tool_calls
                .iter()
                .position(|call| call.name == CONTROL_ACTION_NAME);
            // 控制动作与其他调用混在同一条响应才是“混合”；单独出现即合法。
            let mixed = control_index.is_some() && response.tool_calls.len() > 1;

            let Some(index) = control_index else {
                // 纯业务工具回合。
                self.sessions
                    .record_assistant_message(
                        &mut session,
                        turn_id,
                        response.content.clone(),
                        response.tool_calls.clone(),
                        response.usage,
                    )
                    .await?;

                let calls = response.tool_calls;
                let mut results = Vec::with_capacity(calls.len());
                let mut aborted = false;
                for (offset, call) in calls.iter().enumerate() {
                    if cancel.is_cancelled() {
                        for call in &calls[offset..] {
                            results.push((
                                call.call_id.clone(),
                                ToolStatus::Error,
                                "aborted".to_owned(),
                            ));
                        }
                        aborted = true;
                        break;
                    }
                    let (status, result) = self.tools.execute(call, id, turn_id, &cancel).await;
                    results.push((call.call_id.clone(), status, result));
                }

                self.sessions
                    .record_tool_results(&mut session, turn_id, &results)
                    .await?;

                if aborted || cancel.is_cancelled() {
                    return self
                        .finish_turn(
                            &mut session,
                            turn_id,
                            TurnEndReason::Interrupted,
                            steps,
                            output,
                            None,
                        )
                        .await;
                }
                continue;
            };

            if mixed {
                // 控制动作必须单独出现：整条响应拒绝，业务工具不执行。
                let payload = error_payload(
                    "control_action_must_be_alone",
                    "the complete_stage control action must be the only tool call in a response",
                );
                let results = response
                    .tool_calls
                    .iter()
                    .map(|call| (call.call_id.clone(), ToolStatus::Error, payload.clone()))
                    .collect::<Vec<_>>();
                self.sessions
                    .record_assistant_message(
                        &mut session,
                        turn_id,
                        response.content.clone(),
                        response.tool_calls.clone(),
                        response.usage,
                    )
                    .await?;
                self.sessions
                    .record_tool_results(&mut session, turn_id, &results)
                    .await?;
                if repairs_left == 0 {
                    return self
                        .finish_turn(
                            &mut session,
                            turn_id,
                            TurnEndReason::Error,
                            steps,
                            output,
                            Some("evaluation_failed".to_owned()),
                        )
                        .await;
                }
                repairs_left -= 1;
                continue;
            }

            let call = &response.tool_calls[index];
            match Self::validate_control(&session, ctx, turn_id, call) {
                Ok((record, outcome)) => {
                    let completion = self
                        .sessions
                        .complete_stage(
                            &mut session,
                            CompleteStageBatch {
                                turn_id,
                                steps,
                                assistant_content: response.content.clone(),
                                assistant_tool_calls: response.tool_calls.clone(),
                                usage: response.usage,
                                control_call_id: call.call_id.clone(),
                                record,
                                outcome,
                            },
                        )
                        .await?;

                    info!(
                        session_id = %id,
                        turn_id = turn_id.0,
                        stage = %completion.record.stage_key,
                        "stage completed"
                    );

                    return Ok(TurnResult {
                        turn_id,
                        reason: TurnEndReason::Done,
                        steps,
                        output,
                        error: None,
                        stage_completion: Some(completion),
                    });
                }
                Err(err) => {
                    // stage_not_ready 引导继续对话，不消耗修复预算（§6.4）。
                    let not_ready = matches!(err, CompleteStageError::StageNotReady(_));
                    let payload = error_payload(err.code(), &err.to_string());
                    let results = vec![(call.call_id.clone(), ToolStatus::Error, payload)];
                    self.sessions
                        .record_assistant_message(
                            &mut session,
                            turn_id,
                            response.content.clone(),
                            response.tool_calls.clone(),
                            response.usage,
                        )
                        .await?;
                    self.sessions
                        .record_tool_results(&mut session, turn_id, &results)
                        .await?;
                    if !not_ready {
                        if repairs_left == 0 {
                            return self
                                .finish_turn(
                                    &mut session,
                                    turn_id,
                                    TurnEndReason::Error,
                                    steps,
                                    output,
                                    Some("evaluation_failed".to_owned()),
                                )
                                .await;
                        }
                        repairs_left -= 1;
                    }
                    continue;
                }
            }
        }

        self.finish_turn(
            &mut session,
            turn_id,
            TurnEndReason::MaxSteps,
            steps,
            output,
            None,
        )
        .await
    }

    /// 控制动作校验（§6.3）：解析 → 评估 → min_rounds → Offer → 转移。
    /// 纯函数路径，无 IO；结果由调用方落库。
    fn validate_control(
        session: &Session,
        ctx: &TurnContext,
        turn_id: TurnId,
        call: &ToolCall,
    ) -> Result<(StageEvaluationRecord, StageOutcome), CompleteStageError> {
        if call.arguments.trim().is_empty() {
            return Err(CompleteStageError::InvalidArguments(
                "arguments are empty".to_owned(),
            ));
        }
        let args: CompleteStageArgs = serde_json::from_str(&call.arguments)
            .map_err(|err| CompleteStageError::InvalidArguments(err.to_string()))?;

        if ctx.stage.kind != StageKind::Offer && args.offer.is_some() {
            return Err(CompleteStageError::InvalidArguments(
                "offer is only allowed at the offer stage".to_owned(),
            ));
        }

        let evaluation = evaluate(&ctx.stage.rubric, args.dimensions, args.summary)?;

        let user_rounds = user_round_in_stage(session);
        if user_rounds < usize::from(ctx.stage.min_rounds) {
            return Err(CompleteStageError::StageNotReady(ctx.stage.min_rounds));
        }

        let offer = match (&ctx.stage.kind, args.offer) {
            (StageKind::Offer, Some(input)) => Some(
                build_offer_terms(input, &ctx.offer_policy, Utc::now())
                    .map_err(CompleteStageError::from)?,
            ),
            (StageKind::Offer, None) => {
                return Err(CompleteStageError::InvalidArguments(
                    "offer terms are required at the offer stage".to_owned(),
                ));
            }
            _ => None,
        };

        let transition =
            plan_transition(&ctx.pipeline, &session.current_stage.key, evaluation, offer)
                .map_err(|err| CompleteStageError::InvalidArguments(err.to_string()))?;

        let record = StageEvaluationRecord {
            turn_id,
            stage_key: session.current_stage.key.clone(),
            stage_kind: session.current_stage.kind,
            evaluation: transition.evaluation,
        };
        Ok((record, transition.outcome))
    }

    fn session_lock(&self, id: SessionId) -> Arc<Mutex<()>> {
        let mut locks = self
            .session_locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // 只保留正在执行或等待中的会话锁；空闲会话的条目在下次访问时回收。
        locks.retain(|_, lock| Arc::strong_count(lock) > 1);
        locks
            .entry(id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    async fn finish_turn(
        &self,
        session: &mut Session,
        turn_id: TurnId,
        reason: TurnEndReason,
        steps: u32,
        output: String,
        error: Option<String>,
    ) -> Result<TurnResult, LoopError> {
        self.sessions
            .end_turn(session, turn_id, reason, steps, error.clone())
            .await?;

        info!(
            session_id = %session.id,
            turn_id = turn_id.0,
            step = steps,
            role = %session.role(),
            reason = ?reason,
            "turn ended"
        );

        Ok(TurnResult {
            turn_id,
            reason,
            steps,
            output,
            error,
            stage_completion: None,
        })
    }
}

#[cfg(test)]
mod tests;
