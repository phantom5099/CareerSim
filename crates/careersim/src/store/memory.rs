//! 内存存储实现，语义与 PostgreSQL 实现一致，用于单元测试。

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::AppendBatch;
use super::EVENT_VERSION;
use super::SessionStore;
use super::StoreError;
use super::StoredEvent;
use super::StoredSession;

#[derive(Default)]
pub(crate) struct MemorySessionStore {
    sessions: Mutex<HashMap<Uuid, Stored>>,
}

struct Stored {
    session: StoredSession,
    events: Vec<StoredEvent>,
}

impl MemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionStore for MemorySessionStore {
    async fn create(
        &self,
        id: Uuid,
        stage: &str,
        meta: Value,
    ) -> Result<(StoredSession, StoredEvent), StoreError> {
        let now = Utc::now();
        let event = StoredEvent {
            v: EVENT_VERSION,
            seq: 0,
            at: now,
            kind: meta,
        };
        let session = StoredSession {
            id,
            stage: stage.to_owned(),
            created_at: now,
            updated_at: now,
            last_seq: 0,
            last_turn_id: 0,
        };

        let mut sessions = self.sessions.lock().await;
        if sessions.contains_key(&id) {
            return Err(StoreError::Conflict);
        }
        sessions.insert(
            id,
            Stored {
                session: session.clone(),
                events: vec![event.clone()],
            },
        );
        Ok((session, event))
    }

    async fn load(&self, id: Uuid) -> Result<(StoredSession, Vec<StoredEvent>), StoreError> {
        self.sessions
            .lock()
            .await
            .get(&id)
            .map(|stored| (stored.session.clone(), stored.events.clone()))
            .ok_or(StoreError::NotFound)
    }

    async fn list(&self, limit: usize) -> Result<Vec<StoredSession>, StoreError> {
        let sessions = self.sessions.lock().await;
        let mut summaries: Vec<StoredSession> = sessions
            .values()
            .map(|stored| stored.session.clone())
            .collect();
        summaries.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.id.cmp(&a.id))
        });
        summaries.truncate(limit);
        Ok(summaries)
    }

    async fn delete(&self, id: Uuid) -> Result<(), StoreError> {
        self.sessions
            .lock()
            .await
            .remove(&id)
            .map(|_| ())
            .ok_or(StoreError::NotFound)
    }

    async fn append(
        &self,
        id: Uuid,
        batch: AppendBatch<'_>,
    ) -> Result<Vec<StoredEvent>, StoreError> {
        if batch.events.is_empty() {
            return Ok(Vec::new());
        }

        let mut sessions = self.sessions.lock().await;
        let stored = sessions.get_mut(&id).ok_or(StoreError::NotFound)?;
        if stored.session.stage != batch.expected.stage
            || stored.session.last_seq != batch.expected.last_seq
        {
            return Err(StoreError::Conflict);
        }

        let now = Utc::now();
        let first_seq = stored.session.last_seq + 1;
        let events: Vec<StoredEvent> = batch
            .events
            .iter()
            .enumerate()
            .map(|(index, kind)| StoredEvent {
                v: EVENT_VERSION,
                seq: first_seq + index as u64,
                at: now,
                kind: kind.clone(),
            })
            .collect();

        stored.session.last_seq = first_seq + events.len() as u64 - 1;
        stored.session.last_turn_id = stored.session.last_turn_id.max(batch.last_turn_id);
        if let Some(next) = batch.next_stage {
            stored.session.stage = next.to_owned();
        }
        stored.session.updated_at = now;
        stored.events.extend(events.iter().cloned());
        Ok(events)
    }
}

#[cfg(test)]
mod tests;
