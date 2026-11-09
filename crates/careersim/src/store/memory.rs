//! 内存存储实现，语义与 PostgreSQL 实现一致，用于单元测试。

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::Mutex;

use super::EVENT_VERSION;
use super::SessionStore;
use super::StoreError;
use super::validate_batch;
use crate::agent::Session;
use crate::agent::SessionEvent;
use crate::agent::SessionEventKind;
use crate::agent::SessionId;
use crate::agent::SessionSummary;
use crate::agent::Stage;
use crate::agent::TurnId;

#[derive(Default)]
pub struct MemorySessionStore {
    sessions: Mutex<HashMap<SessionId, Session>>,
}

impl MemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionStore for MemorySessionStore {
    async fn create(&self, stage: Stage) -> Result<Session, StoreError> {
        let id = SessionId::generate();
        let now = Utc::now();
        let session = Session {
            id,
            stage,
            events: vec![SessionEvent {
                v: EVENT_VERSION,
                seq: 0,
                at: now,
                kind: SessionEventKind::SessionMeta {
                    session_id: id,
                    stage,
                },
            }],
            created_at: now,
            updated_at: now,
            last_seq: 0,
            last_turn_id: TurnId(0),
        };

        self.sessions.lock().await.insert(id, session.clone());
        Ok(session)
    }

    async fn load(&self, id: SessionId) -> Result<Session, StoreError> {
        self.sessions
            .lock()
            .await
            .get(&id)
            .cloned()
            .ok_or(StoreError::NotFound)
    }

    async fn list(&self, limit: usize) -> Result<Vec<SessionSummary>, StoreError> {
        let sessions = self.sessions.lock().await;
        let mut summaries: Vec<SessionSummary> = sessions
            .values()
            .map(|session| SessionSummary {
                id: session.id,
                stage: session.stage,
                created_at: session.created_at,
                updated_at: session.updated_at,
                last_seq: session.last_seq,
                last_turn_id: session.last_turn_id,
            })
            .collect();
        summaries.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.id.into_uuid().cmp(&a.id.into_uuid()))
        });
        summaries.truncate(limit);
        Ok(summaries)
    }

    async fn delete(&self, id: SessionId) -> Result<(), StoreError> {
        self.sessions
            .lock()
            .await
            .remove(&id)
            .map(|_| ())
            .ok_or(StoreError::NotFound)
    }

    async fn append(
        &self,
        id: SessionId,
        events: &[SessionEventKind],
    ) -> Result<Vec<SessionEvent>, StoreError> {
        if events.is_empty() {
            return Ok(Vec::new());
        }

        let mut sessions = self.sessions.lock().await;
        let session = sessions.get_mut(&id).ok_or(StoreError::NotFound)?;
        validate_batch(session.stage, events)?;

        let now = Utc::now();
        let first_seq = session.last_seq + 1;
        let events: Vec<SessionEvent> = events
            .iter()
            .enumerate()
            .map(|(index, kind)| SessionEvent {
                v: EVENT_VERSION,
                seq: first_seq + index as u64,
                at: now,
                kind: kind.clone(),
            })
            .collect();

        session.absorb(&events);
        session.updated_at = now;
        Ok(events)
    }
}

#[cfg(test)]
mod tests;
