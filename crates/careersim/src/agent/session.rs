//! 会话、事件信封与事件类型。

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use super::Role;
use super::Stage;
use super::tool::ToolCall;
use crate::llm::client::TokenUsage;
use crate::store::AppendBatch;
use crate::store::EVENT_VERSION;
use crate::store::ExpectedState;
use crate::store::SessionStore;
use crate::store::StoreError;
use crate::store::StoredEvent;
use crate::store::StoredSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn into_uuid(self) -> Uuid {
        self.0
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for SessionId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TurnId(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionEvent {
    pub v: u32,
    pub seq: u64,
    pub at: DateTime<Utc>,
    #[serde(flatten)]
    pub kind: SessionEventKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEventKind {
    SessionMeta {
        session_id: SessionId,
        stage: Stage,
    },
    TurnStarted {
        turn_id: TurnId,
    },
    UserMessage {
        turn_id: TurnId,
        content: String,
        source: UserMessageSource,
    },
    AssistantMessage {
        turn_id: TurnId,
        content: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<TokenUsage>,
    },
    ToolResult {
        turn_id: TurnId,
        tool_call_id: String,
        status: ToolStatus,
        output: String,
    },
    TurnEnded {
        turn_id: TurnId,
        reason: TurnEndReason,
        steps: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    StageCompleted {
        turn_id: TurnId,
        stage: Stage,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserMessageSource {
    User,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Ok,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnEndReason {
    Done,
    MaxSteps,
    Interrupted,
    Error,
}

pub(crate) fn event_turn_id(kind: &SessionEventKind) -> Option<TurnId> {
    match kind {
        SessionEventKind::SessionMeta { .. } => None,
        SessionEventKind::TurnStarted { turn_id }
        | SessionEventKind::UserMessage { turn_id, .. }
        | SessionEventKind::AssistantMessage { turn_id, .. }
        | SessionEventKind::ToolResult { turn_id, .. }
        | SessionEventKind::TurnEnded { turn_id, .. }
        | SessionEventKind::StageCompleted { turn_id, .. } => Some(*turn_id),
    }
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub stage: Stage,
    pub events: Vec<SessionEvent>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seq: u64,
    pub last_turn_id: TurnId,
}

impl Session {
    pub fn role(&self) -> Role {
        self.stage.role()
    }

    /// 最后一个回合只有 `turn_started`、没有 `turn_ended` 时返回它的编号。
    pub fn dangling_turn(&self) -> Option<TurnId> {
        if self.last_turn_id.0 == 0 {
            return None;
        }

        let mut started = false;
        let mut ended = false;
        for event in &self.events {
            if event_turn_id(&event.kind) != Some(self.last_turn_id) {
                continue;
            }
            match event.kind {
                SessionEventKind::TurnStarted { .. } => started = true,
                SessionEventKind::TurnEnded { .. } => ended = true,
                _ => {}
            }
        }

        (started && !ended).then_some(self.last_turn_id)
    }

    /// 把已落库的事件折叠进内存视图。
    pub(crate) fn absorb(&mut self, events: &[SessionEvent]) {
        for event in events {
            if let SessionEventKind::StageCompleted { stage, .. } = &event.kind
                && let Some(next) = stage.next()
            {
                self.stage = next;
            }
            if let Some(turn_id) = event_turn_id(&event.kind) {
                self.last_turn_id = self.last_turn_id.max(turn_id);
            }
            self.last_seq = self.last_seq.max(event.seq);
            self.events.push(event.clone());
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: SessionId,
    pub stage: Stage,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seq: u64,
    pub last_turn_id: TurnId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageCompletion {
    pub turn_id: TurnId,
    /// 推进后的阶段。
    pub stage: Stage,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session not found")]
    NotFound,
    #[error("current stage has no next stage")]
    NoNextStage,
    #[error("session state changed concurrently")]
    Conflict,
    #[error("corrupt session: {0}")]
    Corrupt(String),
    #[error("invalid event batch: {0}")]
    InvalidEvent(String),
    #[error("store backend error: {message}")]
    Backend {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl SessionError {
    fn backend(message: impl Into<String>) -> Self {
        Self::Backend {
            message: message.into(),
            source: None,
        }
    }

    fn backend_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Backend {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

impl From<StoreError> for SessionError {
    fn from(err: StoreError) -> Self {
        match err {
            StoreError::NotFound => Self::NotFound,
            StoreError::Conflict => Self::Conflict,
            StoreError::Corrupt(message) => Self::Corrupt(message),
            StoreError::Backend { message, source } => Self::Backend { message, source },
        }
    }
}

/// 校验一个追加批次；返回 `stage_completed` 推进后的阶段。
fn validate_batch(
    current: Stage,
    events: &[SessionEventKind],
) -> Result<Option<Stage>, SessionError> {
    let mut next = None;
    for (index, event) in events.iter().enumerate() {
        match event {
            SessionEventKind::SessionMeta { .. } => {
                return Err(SessionError::InvalidEvent(
                    "session_meta is only written by create".to_owned(),
                ));
            }
            SessionEventKind::StageCompleted { stage, .. } => {
                if index + 1 != events.len() {
                    return Err(SessionError::InvalidEvent(
                        "stage_completed must be the last event".to_owned(),
                    ));
                }
                if *stage != current {
                    return Err(SessionError::InvalidEvent(format!(
                        "stage_completed for {stage} but current stage is {current}"
                    )));
                }
                let next_stage = current.next().ok_or_else(|| {
                    SessionError::InvalidEvent(format!("stage {current} has no next stage"))
                })?;
                next = Some(next_stage);
            }
            _ => {}
        }
    }
    Ok(next)
}

fn max_turn_id(events: &[SessionEventKind]) -> Option<TurnId> {
    events.iter().filter_map(event_turn_id).max()
}

fn decode_event(stored: StoredEvent) -> Result<SessionEvent, SessionError> {
    if stored.v != EVENT_VERSION {
        return Err(SessionError::Corrupt(format!(
            "unsupported event version {} at seq {}",
            stored.v, stored.seq
        )));
    }

    let kind = serde_json::from_value(stored.kind).map_err(|err| {
        SessionError::Corrupt(format!(
            "invalid event payload at seq {}: {err}",
            stored.seq
        ))
    })?;

    Ok(SessionEvent {
        v: stored.v,
        seq: stored.seq,
        at: stored.at,
        kind,
    })
}

fn summary_from_row(row: StoredSession) -> Result<SessionSummary, SessionError> {
    let stage =
        Stage::from_str(&row.stage).map_err(|err| SessionError::Corrupt(err.to_string()))?;
    Ok(SessionSummary {
        id: SessionId::from_uuid(row.id),
        stage,
        created_at: row.created_at,
        updated_at: row.updated_at,
        last_seq: row.last_seq,
        last_turn_id: TurnId(row.last_turn_id),
    })
}

/// 折叠落库行与事件，并校验缓存字段和事件日志一致。
fn fold_session(
    row: StoredSession,
    stored_events: Vec<StoredEvent>,
) -> Result<Session, SessionError> {
    let id = SessionId::from_uuid(row.id);
    let row_stage =
        Stage::from_str(&row.stage).map_err(|err| SessionError::Corrupt(err.to_string()))?;

    let events = stored_events
        .into_iter()
        .map(decode_event)
        .collect::<Result<Vec<_>, _>>()?;

    let first = events
        .first()
        .ok_or_else(|| SessionError::Corrupt("event log is empty".to_owned()))?;

    let mut stage = match &first.kind {
        SessionEventKind::SessionMeta { session_id, stage } if first.seq == 0 => {
            if *session_id != id {
                return Err(SessionError::Corrupt(
                    "session_meta has a different session_id".to_owned(),
                ));
            }
            *stage
        }
        SessionEventKind::SessionMeta { .. } => {
            return Err(SessionError::Corrupt(
                "session_meta must be the first event (seq = 0)".to_owned(),
            ));
        }
        _ => {
            return Err(SessionError::Corrupt(
                "first event is not session_meta".to_owned(),
            ));
        }
    };

    let mut max_turn = TurnId(0);
    let mut previous_seq = None;
    for event in &events {
        if let Some(previous) = previous_seq
            && event.seq != previous + 1
        {
            return Err(SessionError::Corrupt(format!(
                "event seq is not contiguous at {}",
                event.seq
            )));
        }
        previous_seq = Some(event.seq);

        if let SessionEventKind::StageCompleted {
            stage: completed, ..
        } = &event.kind
        {
            if *completed != stage {
                return Err(SessionError::Corrupt(format!(
                    "stage_completed for {completed} while the log is at {stage}"
                )));
            }
            stage = completed.next().ok_or_else(|| {
                SessionError::Corrupt(format!("stage_completed after final stage {completed}"))
            })?;
        }
        if let Some(turn_id) = event_turn_id(&event.kind) {
            max_turn = max_turn.max(turn_id);
        }
    }

    let actual_last_seq = previous_seq.unwrap_or(0);
    if row.last_seq != actual_last_seq {
        return Err(SessionError::Corrupt(format!(
            "last_seq is {} but the event log ends at {actual_last_seq}",
            row.last_seq
        )));
    }
    let last_turn_id = TurnId(row.last_turn_id);
    if last_turn_id != max_turn {
        return Err(SessionError::Corrupt(format!(
            "last_turn_id is {} but the event log ends at {}",
            last_turn_id.0, max_turn.0
        )));
    }
    if stage != row_stage {
        return Err(SessionError::Corrupt(format!(
            "row stage is {row_stage} but the event log folds to {stage}"
        )));
    }

    Ok(Session {
        id,
        stage,
        events,
        created_at: row.created_at,
        updated_at: row.updated_at,
        last_seq: row.last_seq,
        last_turn_id,
    })
}

/// 会话事件的持久化入口。
///
/// 回合事件如何追加、半截回合如何修复、阶段完成如何与回合事件同批写入，
/// 都由实现决定；`AgentLoop` 只依赖本 trait。
#[async_trait]
pub trait SessionService: Send + Sync {
    async fn create(&self, stage: Stage) -> Result<Session, SessionError>;

    async fn load(&self, id: SessionId) -> Result<Session, SessionError>;

    /// 列出最近的会话摘要。
    async fn list(&self, limit: usize) -> Result<Vec<SessionSummary>, SessionError>;

    /// 删除会话及其事件日志。
    async fn delete(&self, id: SessionId) -> Result<(), SessionError>;

    /// 追加一批事件并把落库结果折叠进内存视图。
    async fn append(
        &self,
        session: &mut Session,
        events: &[SessionEventKind],
    ) -> Result<(), SessionError>;

    /// 修复上一进程遗留的半截回合，然后写入新回合的 `turn_started`。
    async fn start_turn(&self, session: &mut Session) -> Result<TurnId, SessionError>;

    async fn record_user_message(
        &self,
        session: &mut Session,
        turn_id: TurnId,
        content: String,
    ) -> Result<(), SessionError>;

    async fn record_assistant_message(
        &self,
        session: &mut Session,
        turn_id: TurnId,
        content: String,
        tool_calls: Vec<ToolCall>,
        usage: Option<TokenUsage>,
    ) -> Result<(), SessionError>;

    async fn record_tool_results(
        &self,
        session: &mut Session,
        turn_id: TurnId,
        results: &[(String, ToolStatus, String)],
    ) -> Result<(), SessionError>;

    async fn end_turn(
        &self,
        session: &mut Session,
        turn_id: TurnId,
        reason: TurnEndReason,
        steps: u32,
        error: Option<String>,
    ) -> Result<(), SessionError>;

    /// 结束当前阶段并推进到下一阶段；`Stage::Review` 没有下一阶段，返回
    /// `NoNextStage` 且不写事件。
    async fn finish_stage(&self, session: &mut Session) -> Result<StageCompletion, SessionError>;
}

/// 基于 `SessionStore` 的会话服务实现。
#[derive(Clone)]
pub struct StoreSessionService {
    store: Arc<dyn SessionStore>,
}

const SESSION_ID_RETRIES: usize = 3;

impl StoreSessionService {
    pub(crate) fn new(store: Arc<dyn SessionStore>) -> Self {
        Self { store }
    }

    /// 最后一个回合缺少 `turn_ended` 时补写 `interrupted` 终态。
    async fn repair_dangling(&self, session: &mut Session) -> Result<(), SessionError> {
        let Some(turn_id) = session.dangling_turn() else {
            return Ok(());
        };

        self.append(
            session,
            &[SessionEventKind::TurnEnded {
                turn_id,
                reason: TurnEndReason::Interrupted,
                steps: 0,
                error: None,
            }],
        )
        .await?;
        info!(
            session_id = %session.id,
            turn_id = turn_id.0,
            "repaired dangling turn"
        );
        Ok(())
    }
}

#[async_trait]
impl SessionService for StoreSessionService {
    async fn create(&self, stage: Stage) -> Result<Session, SessionError> {
        for _ in 0..SESSION_ID_RETRIES {
            let id = SessionId::generate();
            let meta = SessionEventKind::SessionMeta {
                session_id: id,
                stage,
            };
            let payload = serde_json::to_value(&meta)
                .map_err(|err| SessionError::backend_with_source("encode session_meta", err))?;

            match self
                .store
                .create(id.into_uuid(), stage.as_str(), payload)
                .await
            {
                Ok((row, event)) => return fold_session(row, vec![event]),
                Err(StoreError::Conflict) => continue,
                Err(err) => return Err(err.into()),
            }
        }

        Err(SessionError::backend(
            "could not allocate a unique session id",
        ))
    }

    async fn load(&self, id: SessionId) -> Result<Session, SessionError> {
        let (row, events) = self.store.load(id.into_uuid()).await?;
        fold_session(row, events)
    }

    async fn list(&self, limit: usize) -> Result<Vec<SessionSummary>, SessionError> {
        self.store
            .list(limit)
            .await?
            .into_iter()
            .map(summary_from_row)
            .collect()
    }

    async fn delete(&self, id: SessionId) -> Result<(), SessionError> {
        Ok(self.store.delete(id.into_uuid()).await?)
    }

    async fn append(
        &self,
        session: &mut Session,
        events: &[SessionEventKind],
    ) -> Result<(), SessionError> {
        if events.is_empty() {
            return Ok(());
        }

        let next_stage = validate_batch(session.stage, events)?;
        let last_turn_id = max_turn_id(events).map_or(0, |turn_id| turn_id.0);
        let payloads = events
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| SessionError::backend_with_source("encode event", err))?;

        let stored = self
            .store
            .append(
                session.id.into_uuid(),
                AppendBatch {
                    expected: ExpectedState {
                        stage: session.stage.as_str(),
                        last_seq: session.last_seq,
                    },
                    events: &payloads,
                    last_turn_id,
                    next_stage: next_stage.map(Stage::as_str),
                },
            )
            .await?;

        let events = stored
            .into_iter()
            .map(decode_event)
            .collect::<Result<Vec<_>, _>>()?;
        session.absorb(&events);
        Ok(())
    }

    async fn start_turn(&self, session: &mut Session) -> Result<TurnId, SessionError> {
        self.repair_dangling(session).await?;

        let turn_id = TurnId(session.last_turn_id.0 + 1);
        self.append(session, &[SessionEventKind::TurnStarted { turn_id }])
            .await?;
        Ok(turn_id)
    }

    async fn record_user_message(
        &self,
        session: &mut Session,
        turn_id: TurnId,
        content: String,
    ) -> Result<(), SessionError> {
        self.append(
            session,
            &[SessionEventKind::UserMessage {
                turn_id,
                content,
                source: UserMessageSource::User,
            }],
        )
        .await
    }

    async fn record_assistant_message(
        &self,
        session: &mut Session,
        turn_id: TurnId,
        content: String,
        tool_calls: Vec<ToolCall>,
        usage: Option<TokenUsage>,
    ) -> Result<(), SessionError> {
        self.append(
            session,
            &[SessionEventKind::AssistantMessage {
                turn_id,
                content,
                tool_calls,
                usage,
            }],
        )
        .await
    }

    async fn record_tool_results(
        &self,
        session: &mut Session,
        turn_id: TurnId,
        results: &[(String, ToolStatus, String)],
    ) -> Result<(), SessionError> {
        let events = results
            .iter()
            .map(
                |(tool_call_id, status, output)| SessionEventKind::ToolResult {
                    turn_id,
                    tool_call_id: tool_call_id.clone(),
                    status: *status,
                    output: output.clone(),
                },
            )
            .collect::<Vec<_>>();
        self.append(session, &events).await
    }

    async fn end_turn(
        &self,
        session: &mut Session,
        turn_id: TurnId,
        reason: TurnEndReason,
        steps: u32,
        error: Option<String>,
    ) -> Result<(), SessionError> {
        self.append(
            session,
            &[SessionEventKind::TurnEnded {
                turn_id,
                reason,
                steps,
                error,
            }],
        )
        .await
    }

    async fn finish_stage(&self, session: &mut Session) -> Result<StageCompletion, SessionError> {
        self.repair_dangling(session).await?;

        let from = session.stage;
        let next = from.next().ok_or(SessionError::NoNextStage)?;
        let turn_id = TurnId(session.last_turn_id.0 + 1);

        self.append(
            session,
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
                    stage: from,
                },
            ],
        )
        .await?;

        Ok(StageCompletion {
            turn_id,
            stage: next,
        })
    }
}

#[cfg(test)]
mod tests;
