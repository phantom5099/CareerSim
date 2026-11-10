//! 会话回合循环：per-session 互斥、工具循环、取消与阶段推进。

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use super::BuildOptions;
use super::ContextBuilder;
use super::Session;
use super::SessionError;
use super::SessionId;
use super::SessionService;
use super::StageCompletion;
use super::ToolRegistry;
use super::ToolStatus;
use super::TurnEndReason;
use super::TurnId;
use crate::error::error_chain;
use crate::llm::LlmClient;
use crate::llm::LlmError;

/// 循环参数；模型连接与采样参数由 `LlmClient` 在构造时固定。
#[derive(Debug, Clone, Copy)]
pub struct LoopConfig {
    pub max_steps: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnResult {
    pub turn_id: TurnId,
    pub reason: TurnEndReason,
    pub steps: u32,
    pub output: String,
    pub error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoopError {
    #[error("session is busy")]
    SessionBusy,
    #[error(transparent)]
    Session(#[from] SessionError),
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
        cancel: CancellationToken,
    ) -> Result<TurnResult, LoopError> {
        let lock = self.session_lock(id);
        let _guard = match lock.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(LoopError::SessionBusy),
        };

        let mut session = self.sessions.load(id).await?;
        let turn_id = self.sessions.start_turn(&mut session).await?;
        info!(
            session_id = %id,
            turn_id = turn_id.0,
            stage = %session.stage,
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

        let mut steps: u32 = 0;
        let mut output = String::new();
        let has_tools = !self.tools.is_empty();

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

            debug!(
                session_id = %id,
                turn_id = turn_id.0,
                step = steps + 1,
                role = %session.role(),
                "requesting model"
            );

            let tools = if has_tools {
                self.tools.specs()
            } else {
                Vec::new()
            };

            let request = match self
                .context
                .build(&session, &tools, BuildOptions::default())
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

            self.sessions
                .record_assistant_message(
                    &mut session,
                    turn_id,
                    response.content.clone(),
                    response.tool_calls.clone(),
                    response.usage,
                )
                .await?;

            if response.tool_calls.is_empty() {
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

            let calls = response.tool_calls;
            let mut results = Vec::with_capacity(calls.len());
            let mut aborted = false;

            for (index, call) in calls.iter().enumerate() {
                if cancel.is_cancelled() {
                    for call in &calls[index..] {
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

    /// 结束当前阶段并推进到下一阶段；`Stage::Review` 没有下一阶段，返回
    /// `SessionError::NoNextStage`。
    pub async fn finish_stage(&self, id: SessionId) -> Result<StageCompletion, LoopError> {
        let lock = self.session_lock(id);
        let _guard = match lock.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(LoopError::SessionBusy),
        };

        let mut session = self.sessions.load(id).await?;
        let from = session.stage;
        let completion = self.sessions.finish_stage(&mut session).await?;

        info!(
            session_id = %id,
            turn_id = completion.turn_id.0,
            from = %from,
            to = %completion.stage,
            "stage completed"
        );

        Ok(completion)
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
        })
    }
}

#[cfg(test)]
mod tests;
