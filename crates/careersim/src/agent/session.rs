//! 会话、事件信封与事件类型。

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Role;
use super::Stage;
use super::tool::ToolCall;
use crate::llm::client::TokenUsage;

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

#[cfg(test)]
mod tests;
