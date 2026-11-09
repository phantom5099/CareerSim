//! 会话回合循环：per-session 互斥、工具循环、取消与阶段推进。

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use super::BuildOptions;
use super::ContextBuilder;
use super::Session;
use super::SessionEventKind;
use super::SessionId;
use super::Stage;
use super::ToolCall;
use super::ToolRegistry;
use super::ToolStatus;
use super::TurnEndReason;
use super::TurnId;
use super::UserMessageSource;
use crate::llm::LlmClient;
use crate::llm::LlmError;
use crate::store::SessionStore;
use crate::store::StoreError;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageCompletion {
    pub turn_id: TurnId,
    /// 推进后的阶段。
    pub stage: Stage,
}

#[derive(Debug, thiserror::Error)]
pub enum LoopError {
    #[error("session not found")]
    SessionNotFound,
    #[error("session is busy")]
    SessionBusy,
    #[error("current stage has no next stage")]
    NoNextStage,
    #[error(transparent)]
    Store(StoreError),
}

impl From<StoreError> for LoopError {
    fn from(err: StoreError) -> Self {
        match err {
            StoreError::NotFound => Self::SessionNotFound,
            other => Self::Store(other),
        }
    }
}

pub struct AgentLoop {
    llm: Arc<dyn LlmClient>,
    context: Arc<dyn ContextBuilder>,
    tools: Arc<ToolRegistry>,
    store: Arc<dyn SessionStore>,
    config: LoopConfig,
    // std 锁只用于查找/插入，不跨 await；每个会话的回合锁是 tokio 异步锁
    session_locks: std::sync::Mutex<HashMap<SessionId, Arc<Mutex<()>>>>,
}

impl AgentLoop {
    pub fn new(
        llm: Arc<dyn LlmClient>,
        context: Arc<dyn ContextBuilder>,
        tools: Arc<ToolRegistry>,
        store: Arc<dyn SessionStore>,
        config: LoopConfig,
    ) -> Self {
        Self {
            llm,
            context,
            tools,
            store,
            config,
            session_locks: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub async fn create_session(&self, stage: Stage) -> Result<Session, LoopError> {
        Ok(self.store.create(stage).await?)
    }

    pub async fn load_session(&self, id: SessionId) -> Result<Session, LoopError> {
        Ok(self.store.load(id).await?)
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

        let mut session = self.store.load(id).await?;
        self.repair_dangling(&mut session).await?;

        let turn_id = TurnId(session.last_turn_id.0 + 1);
        info!(
            session_id = %id,
            turn_id = turn_id.0,
            stage = %session.stage,
            role = %session.role(),
            "turn started"
        );

        let stored = self
            .store
            .append(id, &[SessionEventKind::TurnStarted { turn_id }])
            .await?;
        session.absorb(&stored);

        if cancel.is_cancelled() {
            return self
                .close_turn(
                    &mut session,
                    turn_id,
                    TurnEndReason::Interrupted,
                    0,
                    String::new(),
                    None,
                )
                .await;
        }

        let stored = self
            .store
            .append(
                id,
                &[SessionEventKind::UserMessage {
                    turn_id,
                    content: input.into(),
                    source: UserMessageSource::User,
                }],
            )
            .await?;
        session.absorb(&stored);

        let mut steps: u32 = 0;
        let mut output = String::new();
        let has_tools = !self.tools.is_empty();

        for _ in 0..self.config.max_steps {
            if cancel.is_cancelled() {
                return self
                    .close_turn(
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
                        .close_turn(
                            &mut session,
                            turn_id,
                            TurnEndReason::Error,
                            steps,
                            output,
                            Some(err.to_string()),
                        )
                        .await;
                }
            };

            let response = match self.llm.complete(request, &cancel).await {
                Ok(response) => response,
                Err(LlmError::Cancelled) => {
                    return self
                        .close_turn(
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
                            .close_turn(
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
                        .close_turn(
                            &mut session,
                            turn_id,
                            TurnEndReason::Error,
                            steps,
                            output,
                            Some(err.to_string()),
                        )
                        .await;
                }
            };

            steps += 1;
            output = response.content.clone();

            let stored = self
                .store
                .append(
                    id,
                    &[SessionEventKind::AssistantMessage {
                        turn_id,
                        content: response.content.clone(),
                        tool_calls: response.tool_calls.clone(),
                        usage: response.usage,
                    }],
                )
                .await?;
            session.absorb(&stored);

            if response.tool_calls.is_empty() {
                if response.content.trim().is_empty() {
                    return self
                        .close_turn(
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
                    .close_turn(
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
                        results.push(tool_result(
                            turn_id,
                            call,
                            ToolStatus::Error,
                            "aborted".to_owned(),
                        ));
                    }
                    aborted = true;
                    break;
                }

                let (status, result) = self.tools.execute(call, id, turn_id, &cancel).await;
                results.push(tool_result(turn_id, call, status, result));
            }

            let stored = self.store.append(id, &results).await?;
            session.absorb(&stored);

            if aborted || cancel.is_cancelled() {
                return self
                    .close_turn(
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

        self.close_turn(
            &mut session,
            turn_id,
            TurnEndReason::MaxSteps,
            steps,
            output,
            None,
        )
        .await
    }

    /// 结束当前阶段并推进到下一阶段；`Stage::Review` 没有下一阶段，返回 `NoNextStage`。
    pub async fn finish_stage(&self, id: SessionId) -> Result<StageCompletion, LoopError> {
        let lock = self.session_lock(id);
        let _guard = match lock.try_lock() {
            Ok(guard) => guard,
            Err(_) => return Err(LoopError::SessionBusy),
        };

        let mut session = self.store.load(id).await?;
        self.repair_dangling(&mut session).await?;

        let from = session.stage;
        let next = from.next().ok_or(LoopError::NoNextStage)?;
        let turn_id = TurnId(session.last_turn_id.0 + 1);

        let stored = self
            .store
            .append(
                id,
                &[
                    SessionEventKind::TurnStarted { turn_id },
                    SessionEventKind::TurnEnded {
                        turn_id,
                        reason: TurnEndReason::Done,
                        steps: 0,
                        error: None,
                    },
                    SessionEventKind::StageCompleted {
                        turn_id,
                        stage: session.stage,
                    },
                ],
            )
            .await?;
        session.absorb(&stored);

        info!(
            session_id = %id,
            turn_id = turn_id.0,
            from = %from,
            to = %next,
            "stage completed"
        );

        Ok(StageCompletion {
            turn_id,
            stage: next,
        })
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

    async fn repair_dangling(&self, session: &mut Session) -> Result<(), LoopError> {
        let Some(turn_id) = session.dangling_turn() else {
            return Ok(());
        };

        let stored = self
            .store
            .append(
                session.id,
                &[SessionEventKind::TurnEnded {
                    turn_id,
                    reason: TurnEndReason::Interrupted,
                    steps: 0,
                    error: None,
                }],
            )
            .await?;
        session.absorb(&stored);
        info!(session_id = %session.id, turn_id = turn_id.0, "repaired dangling turn");
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn close_turn(
        &self,
        session: &mut Session,
        turn_id: TurnId,
        reason: TurnEndReason,
        steps: u32,
        output: String,
        error: Option<String>,
    ) -> Result<TurnResult, LoopError> {
        let stored = self
            .store
            .append(
                session.id,
                &[SessionEventKind::TurnEnded {
                    turn_id,
                    reason,
                    steps,
                    error: error.clone(),
                }],
            )
            .await?;
        session.absorb(&stored);

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

fn tool_result(
    turn_id: TurnId,
    call: &ToolCall,
    status: ToolStatus,
    output: String,
) -> SessionEventKind {
    SessionEventKind::ToolResult {
        turn_id,
        tool_call_id: call.call_id.clone(),
        status,
        output,
    }
}

#[cfg(test)]
mod tests;
