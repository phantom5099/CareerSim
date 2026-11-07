//! 会话与事件存储。事件日志是唯一事实来源。

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::agent::Session;
use crate::agent::SessionEvent;
use crate::agent::SessionEventKind;
use crate::agent::SessionId;
use crate::agent::SessionSummary;
use crate::agent::Stage;
use crate::agent::TurnId;
use crate::agent::session::event_turn_id;

pub mod memory;
pub mod postgres;

pub use memory::MemorySessionStore;
pub use postgres::PostgresSessionStore;

/// 当前事件格式版本。
pub const EVENT_VERSION: u32 = 1;

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create(&self, stage: Stage) -> Result<Session, StoreError>;
    async fn load(&self, id: SessionId) -> Result<Session, StoreError>;
    async fn list(&self, limit: usize) -> Result<Vec<SessionSummary>, StoreError>;
    async fn delete(&self, id: SessionId) -> Result<(), StoreError>;

    /// 追加事件；`v`、`seq`、`at` 由 store 分配。
    async fn append(
        &self,
        id: SessionId,
        events: &[SessionEventKind],
    ) -> Result<Vec<SessionEvent>, StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    #[error("session not found")]
    NotFound,
    #[error("corrupt session: {0}")]
    Corrupt(String),
    #[error("invalid event batch: {0}")]
    InvalidEvent(String),
    #[error("store backend error: {0}")]
    Backend(String),
}

/// 校验一个追加批次；返回 `stage_completed` 推进后的阶段。
pub(crate) fn validate_batch(
    current: Stage,
    events: &[SessionEventKind],
) -> Result<Option<Stage>, StoreError> {
    let mut next = None;
    for (index, event) in events.iter().enumerate() {
        match event {
            SessionEventKind::SessionMeta { .. } => {
                return Err(StoreError::InvalidEvent(
                    "session_meta is only written by create".to_owned(),
                ));
            }
            SessionEventKind::StageCompleted { stage, .. } => {
                if index + 1 != events.len() {
                    return Err(StoreError::InvalidEvent(
                        "stage_completed must be the last event".to_owned(),
                    ));
                }
                if *stage != current {
                    return Err(StoreError::InvalidEvent(format!(
                        "stage_completed for {stage} but current stage is {current}"
                    )));
                }
                let next_stage = current.next().ok_or_else(|| {
                    StoreError::InvalidEvent(format!("stage {current} has no next stage"))
                })?;
                next = Some(next_stage);
            }
            _ => {}
        }
    }
    Ok(next)
}

pub(crate) fn max_turn_id(events: &[SessionEventKind]) -> Option<TurnId> {
    events.iter().filter_map(event_turn_id).max()
}

/// 折叠落库行与事件，并校验缓存字段和事件日志一致。
pub(crate) fn fold_session(
    id: SessionId,
    row_stage: Stage,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_seq: u64,
    last_turn_id: TurnId,
    events: Vec<SessionEvent>,
) -> Result<Session, StoreError> {
    let first = events
        .first()
        .ok_or_else(|| StoreError::Corrupt("event log is empty".to_owned()))?;

    let mut stage = match &first.kind {
        SessionEventKind::SessionMeta { session_id, stage } if first.seq == 0 => {
            if *session_id != id {
                return Err(StoreError::Corrupt(
                    "session_meta has a different session_id".to_owned(),
                ));
            }
            *stage
        }
        SessionEventKind::SessionMeta { .. } => {
            return Err(StoreError::Corrupt(
                "session_meta must be the first event (seq = 0)".to_owned(),
            ));
        }
        _ => {
            return Err(StoreError::Corrupt(
                "first event is not session_meta".to_owned(),
            ));
        }
    };

    let mut max_turn = TurnId(0);
    let mut previous_seq = None;
    for event in &events {
        if event.v != EVENT_VERSION {
            return Err(StoreError::Corrupt(format!(
                "unsupported event version {} at seq {}",
                event.v, event.seq
            )));
        }
        if let Some(previous) = previous_seq
            && event.seq != previous + 1
        {
            return Err(StoreError::Corrupt(format!(
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
                return Err(StoreError::Corrupt(format!(
                    "stage_completed for {completed} while the log is at {stage}"
                )));
            }
            stage = completed.next().ok_or_else(|| {
                StoreError::Corrupt(format!("stage_completed after final stage {completed}"))
            })?;
        }
        if let Some(turn_id) = event_turn_id(&event.kind) {
            max_turn = max_turn.max(turn_id);
        }
    }

    let actual_last_seq = previous_seq.unwrap_or(0);
    if last_seq != actual_last_seq {
        return Err(StoreError::Corrupt(format!(
            "last_seq is {last_seq} but the event log ends at {actual_last_seq}"
        )));
    }
    if last_turn_id != max_turn {
        return Err(StoreError::Corrupt(format!(
            "last_turn_id is {} but the event log ends at {}",
            last_turn_id.0, max_turn.0
        )));
    }
    if stage != row_stage {
        return Err(StoreError::Corrupt(format!(
            "row stage is {row_stage} but the event log folds to {stage}"
        )));
    }

    Ok(Session {
        id,
        stage,
        events,
        created_at,
        updated_at,
        last_seq,
        last_turn_id,
    })
}
