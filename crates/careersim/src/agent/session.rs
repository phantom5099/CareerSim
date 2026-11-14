//! 会话、事件信封与事件类型（事件 v2）。

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use super::Role;
use super::tool::ToolCall;
use crate::llm::client::TokenUsage;
use crate::pipeline::{StageEvaluation, StageKey, StageKind, StageOutcome};
use crate::store::AppendBatch;
use crate::store::EVENT_VERSION;
use crate::store::ExpectedState;
use crate::store::SessionStore;
use crate::store::StoreError;
use crate::store::StoredEvent;
use crate::store::StoredSession;

/// 投递归属标识；事件绑定自描述用，不构成第二份状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ApplicationId(Uuid);

impl ApplicationId {
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ApplicationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ApplicationId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

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

/// 会话与业务聚合的绑定；不携带阶段，阶段由事件折叠得出。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionBinding {
    /// 流水线阶段会话；每个投递同一时刻只有一个进行中的阶段会话。
    ApplicationStage { application_id: ApplicationId },
}

impl SessionBinding {
    /// 归属的投递标识。
    pub fn application_id(self) -> ApplicationId {
        match self {
            Self::ApplicationStage { application_id } => application_id,
        }
    }
}

/// 阶段引用：稳定键 + 类型；类型决定角色与执行策略。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageRef {
    pub key: StageKey,
    pub kind: StageKind,
}

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
        binding: SessionBinding,
        initial_stage: StageRef,
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
        /// 完成的阶段键；必须等于会话当前阶段。
        stage_key: StageKey,
        /// 完成阶段的类型。
        stage_kind: StageKind,
        /// 服务端评估结论。
        evaluation: StageEvaluation,
        /// 转移结果。
        outcome: StageOutcome,
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
    pub binding: SessionBinding,
    /// 当前阶段；由 `initial_stage` 初始化，仅由 `StageCompleted` 更新。
    pub current_stage: StageRef,
    pub events: Vec<SessionEvent>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seq: u64,
    pub last_turn_id: TurnId,
}

impl Session {
    pub fn role(&self) -> Role {
        self.current_stage.kind.role()
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

    /// 把已落库的事件折叠进内存视图；阶段推进只由 `Advance` 驱动。
    pub(crate) fn absorb(&mut self, events: &[SessionEvent]) {
        for event in events {
            if let SessionEventKind::StageCompleted { outcome, .. } = &event.kind
                && let StageOutcome::Advance {
                    next_stage_key,
                    next_stage_kind,
                } = outcome
            {
                self.current_stage = StageRef {
                    key: next_stage_key.clone(),
                    kind: *next_stage_kind,
                };
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
    /// 缓存的当前阶段键；类型在行缓存中不可得，按需加载会话。
    pub stage_key: StageKey,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seq: u64,
    pub last_turn_id: TurnId,
}

/// 一次阶段完成的评估记录；事件之外的服务层视图。
#[derive(Debug, Clone, PartialEq)]
pub struct StageEvaluationRecord {
    pub turn_id: TurnId,
    pub stage_key: StageKey,
    pub stage_kind: StageKind,
    pub evaluation: StageEvaluation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StageCompletion {
    pub turn_id: TurnId,
    pub record: StageEvaluationRecord,
    pub outcome: StageOutcome,
}

/// `SessionService::complete_stage` 的入参：把评估回合的事件批次一次写入。
#[derive(Debug, Clone)]
pub struct CompleteStageBatch {
    pub turn_id: TurnId,
    pub steps: u32,
    pub assistant_content: String,
    pub assistant_tool_calls: Vec<ToolCall>,
    pub usage: Option<TokenUsage>,
    /// 控制动作的 tool_call_id；工具结果回填给它。
    pub control_call_id: String,
    pub record: StageEvaluationRecord,
    pub outcome: StageOutcome,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session not found")]
    NotFound,
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

/// 单个事件 payload 的大小上限（plan/02 §10.1 第 5 条）。
const MAX_EVENT_PAYLOAD_BYTES: usize = 64 * 1024;

/// 校验一个追加批次；返回 `StageCompleted` 推进后的阶段。
fn validate_batch(
    current: &StageRef,
    events: &[SessionEventKind],
) -> Result<Option<StageRef>, SessionError> {
    let mut next = None;
    for (index, event) in events.iter().enumerate() {
        match event {
            SessionEventKind::SessionMeta { .. } => {
                return Err(SessionError::InvalidEvent(
                    "session_meta is only written by create".to_owned(),
                ));
            }
            SessionEventKind::StageCompleted {
                stage_key,
                stage_kind,
                outcome,
                ..
            } => {
                if index + 1 != events.len() {
                    return Err(SessionError::InvalidEvent(
                        "stage_completed must be the last event".to_owned(),
                    ));
                }
                if stage_key != &current.key {
                    return Err(SessionError::InvalidEvent(format!(
                        "stage_completed for {stage_key} but current stage is {}",
                        current.key
                    )));
                }
                if stage_kind != &current.kind {
                    return Err(SessionError::InvalidEvent(format!(
                        "stage_completed for kind {stage_kind:?} but current stage is {:?}",
                        current.kind
                    )));
                }
                match outcome {
                    StageOutcome::Advance {
                        next_stage_key,
                        next_stage_kind,
                    } => {
                        if next_stage_key == &current.key {
                            return Err(SessionError::InvalidEvent(
                                "advance next_stage_key must differ from the current stage"
                                    .to_owned(),
                            ));
                        }
                        next = Some(StageRef {
                            key: next_stage_key.clone(),
                            kind: *next_stage_kind,
                        });
                    }
                    StageOutcome::OfferPending { .. } | StageOutcome::Rejected => {}
                }
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
    let stage_key =
        StageKey::parse(&row.stage).map_err(|err| SessionError::Corrupt(err.to_string()))?;
    Ok(SessionSummary {
        id: SessionId::from_uuid(row.id),
        stage_key,
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
    let row_stage_key =
        StageKey::parse(&row.stage).map_err(|err| SessionError::Corrupt(err.to_string()))?;

    let events = stored_events
        .into_iter()
        .map(decode_event)
        .collect::<Result<Vec<_>, _>>()?;

    let first = events
        .first()
        .ok_or_else(|| SessionError::Corrupt("event log is empty".to_owned()))?;

    let binding;
    let mut current_stage = match &first.kind {
        SessionEventKind::SessionMeta {
            session_id,
            binding: event_binding,
            initial_stage,
        } if first.seq == 0 => {
            if *session_id != id {
                return Err(SessionError::Corrupt(
                    "session_meta has a different session_id".to_owned(),
                ));
            }
            binding = *event_binding;
            initial_stage.clone()
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
            stage_key,
            stage_kind,
            outcome,
            ..
        } = &event.kind
        {
            if *stage_key != current_stage.key || *stage_kind != current_stage.kind {
                return Err(SessionError::Corrupt(format!(
                    "stage_completed for {stage_key} while the log is at {}",
                    current_stage.key
                )));
            }
            if let StageOutcome::Advance {
                next_stage_key,
                next_stage_kind,
            } = outcome
            {
                current_stage = StageRef {
                    key: next_stage_key.clone(),
                    kind: *next_stage_kind,
                };
            }
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
    if current_stage.key != row_stage_key {
        return Err(SessionError::Corrupt(format!(
            "row stage is {row_stage_key} but the event log folds to {}",
            current_stage.key
        )));
    }

    Ok(Session {
        id,
        binding,
        current_stage,
        events,
        created_at: row.created_at,
        updated_at: row.updated_at,
        last_seq: row.last_seq,
        last_turn_id,
    })
}

/// 最后一个 `StageCompleted` 之后的事件起点；当前阶段的事件区间起点。
pub(crate) fn stage_started_at(session: &Session) -> usize {
    session
        .events
        .iter()
        .rposition(|event| matches!(event.kind, SessionEventKind::StageCompleted { .. }))
        .map_or(0, |index| index + 1)
}

/// 当前阶段内的助手消息数；`max_rounds` 与评估轮触发依据。
pub(crate) fn assistant_count_in_stage(session: &Session) -> usize {
    let start = stage_started_at(session);
    session.events[start..]
        .iter()
        .filter(|event| matches!(event.kind, SessionEventKind::AssistantMessage { .. }))
        .count()
}

/// 当前阶段内的用户消息数（对话轮次）；`min_rounds` 校验与轮次注一致。
pub(crate) fn user_round_in_stage(session: &Session) -> usize {
    let start = stage_started_at(session);
    session.events[start..]
        .iter()
        .filter(|event| matches!(event.kind, SessionEventKind::UserMessage { .. }))
        .count()
}

/// 会话事件的持久化入口。
///
/// 回合事件如何追加、半截回合如何修复、阶段完成如何与回合事件同批写入，
/// 都由实现决定；`AgentLoop` 只依赖本 trait。
#[async_trait]
pub trait SessionService: Send + Sync {
    /// 创建会话并写入绑定与首阶段。
    async fn create(
        &self,
        binding: SessionBinding,
        initial_stage: StageRef,
    ) -> Result<Session, SessionError>;

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

    /// 把评估结论与阶段完成批次一次性写入：助手消息、控制动作工具结果、
    /// 回合终止与 `StageCompleted` 同批落库；返回服务层评估记录。
    async fn complete_stage(
        &self,
        session: &mut Session,
        batch: CompleteStageBatch,
    ) -> Result<StageCompletion, SessionError>;
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
    async fn create(
        &self,
        binding: SessionBinding,
        initial_stage: StageRef,
    ) -> Result<Session, SessionError> {
        for _ in 0..SESSION_ID_RETRIES {
            let id = SessionId::generate();
            let stage = initial_stage.key.clone();
            let meta = SessionEventKind::SessionMeta {
                session_id: id,
                binding,
                initial_stage: initial_stage.clone(),
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

        let next_stage = validate_batch(&session.current_stage, events)?;
        let last_turn_id = max_turn_id(events).map_or(0, |turn_id| turn_id.0);
        let mut payloads = Vec::with_capacity(events.len());
        for event in events {
            let payload = serde_json::to_value(event)
                .map_err(|err| SessionError::backend_with_source("encode event", err))?;
            let size = serde_json::to_vec(&payload)
                .map_err(|err| SessionError::backend_with_source("measure event payload", err))?
                .len();
            if size > MAX_EVENT_PAYLOAD_BYTES {
                return Err(SessionError::InvalidEvent(format!(
                    "event payload is {size} bytes, limit is {MAX_EVENT_PAYLOAD_BYTES}"
                )));
            }
            payloads.push(payload);
        }

        let stored = self
            .store
            .append(
                session.id.into_uuid(),
                AppendBatch {
                    expected: ExpectedState {
                        stage: session.current_stage.key.as_str(),
                        last_seq: session.last_seq,
                    },
                    events: &payloads,
                    last_turn_id,
                    next_stage: next_stage.as_ref().map(|stage| stage.key.as_str()),
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

    async fn complete_stage(
        &self,
        session: &mut Session,
        batch: CompleteStageBatch,
    ) -> Result<StageCompletion, SessionError> {
        // 评估回合进行中，不做半截回合修复；`start_turn` 已负责。
        let record = batch.record;
        let outcome = batch.outcome;
        self.append(
            session,
            &[
                SessionEventKind::AssistantMessage {
                    turn_id: batch.turn_id,
                    content: batch.assistant_content,
                    tool_calls: batch.assistant_tool_calls,
                    usage: batch.usage,
                },
                SessionEventKind::ToolResult {
                    turn_id: batch.turn_id,
                    tool_call_id: batch.control_call_id,
                    status: ToolStatus::Ok,
                    output: "{\"accepted\":true}".to_owned(),
                },
                SessionEventKind::TurnEnded {
                    turn_id: batch.turn_id,
                    reason: TurnEndReason::Done,
                    steps: batch.steps,
                    error: None,
                },
                SessionEventKind::StageCompleted {
                    turn_id: record.turn_id,
                    stage_key: record.stage_key.clone(),
                    stage_kind: record.stage_kind,
                    evaluation: record.evaluation.clone(),
                    outcome: outcome.clone(),
                },
            ],
        )
        .await?;

        Ok(StageCompletion {
            turn_id: record.turn_id,
            record,
            outcome,
        })
    }
}

#[cfg(test)]
mod tests;
