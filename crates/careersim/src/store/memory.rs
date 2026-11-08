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
                .then_with(|| b.id.as_uuid().cmp(&a.id.as_uuid()))
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
mod tests {
    use std::sync::Arc;

    use super::*;

    fn started(turn_id: u64) -> SessionEventKind {
        SessionEventKind::TurnStarted {
            turn_id: TurnId(turn_id),
        }
    }

    fn completed(stage: Stage, turn_id: u64) -> SessionEventKind {
        SessionEventKind::StageCompleted {
            turn_id: TurnId(turn_id),
            stage,
        }
    }

    #[tokio::test]
    async fn create_load_list_delete_roundtrip() {
        let store = MemorySessionStore::new();
        let session = store.create(Stage::Exam).await.unwrap();

        let loaded = store.load(session.id).await.unwrap();
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.stage, Stage::Exam);
        assert_eq!(loaded.events.len(), 1);

        let summaries = store.list(10).await.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, session.id);

        store.delete(session.id).await.unwrap();
        assert!(matches!(
            store.load(session.id).await,
            Err(StoreError::NotFound)
        ));
        assert!(matches!(
            store.delete(session.id).await,
            Err(StoreError::NotFound)
        ));
    }

    #[tokio::test]
    async fn append_assigns_contiguous_seq_and_updates_cache() {
        let store = MemorySessionStore::new();
        let session = store.create(Stage::HrScreen).await.unwrap();

        let first = store.append(session.id, &[started(1)]).await.unwrap();
        let second = store
            .append(
                session.id,
                &[
                    SessionEventKind::TurnEnded {
                        turn_id: TurnId(1),
                        reason: crate::agent::TurnEndReason::Done,
                        steps: 1,
                        error: None,
                    },
                    completed(Stage::HrScreen, 1),
                ],
            )
            .await
            .unwrap();

        assert_eq!(first[0].seq, 1);
        assert_eq!(second[0].seq, 2);
        assert_eq!(second[1].seq, 3);

        let loaded = store.load(session.id).await.unwrap();
        assert_eq!(loaded.stage, Stage::Exam);
        assert_eq!(loaded.last_seq, 3);
        assert_eq!(loaded.last_turn_id, TurnId(1));
    }

    #[tokio::test]
    async fn append_rejects_session_meta() {
        let store = MemorySessionStore::new();
        let session = store.create(Stage::HrScreen).await.unwrap();

        let error = store
            .append(
                session.id,
                &[SessionEventKind::SessionMeta {
                    session_id: session.id,
                    stage: Stage::Exam,
                }],
            )
            .await
            .unwrap_err();
        assert!(matches!(error, StoreError::InvalidEvent(_)));
    }

    #[tokio::test]
    async fn append_rejects_invalid_stage_completion() {
        let store = MemorySessionStore::new();
        let session = store.create(Stage::HrScreen).await.unwrap();

        let not_last = store
            .append(session.id, &[completed(Stage::HrScreen, 1), started(1)])
            .await
            .unwrap_err();
        assert!(matches!(not_last, StoreError::InvalidEvent(_)));

        let wrong_stage = store
            .append(session.id, &[completed(Stage::Exam, 1)])
            .await
            .unwrap_err();
        assert!(matches!(wrong_stage, StoreError::InvalidEvent(_)));

        let review = store.create(Stage::Review).await.unwrap();
        let final_stage = store
            .append(review.id, &[completed(Stage::Review, 1)])
            .await
            .unwrap_err();
        assert!(matches!(final_stage, StoreError::InvalidEvent(_)));
    }

    #[tokio::test]
    async fn concurrent_appends_do_not_share_seq() {
        let store = Arc::new(MemorySessionStore::new());
        let session = store.create(Stage::HrScreen).await.unwrap();

        let first = {
            let store = store.clone();
            let id = session.id;
            tokio::spawn(async move { store.append(id, &[started(1)]).await })
        };
        let second = {
            let store = store.clone();
            let id = session.id;
            tokio::spawn(async move { store.append(id, &[started(2)]).await })
        };

        let mut seqs = vec![
            first.await.unwrap().unwrap()[0].seq,
            second.await.unwrap().unwrap()[0].seq,
        ];
        seqs.sort_unstable();
        assert_eq!(seqs, vec![1, 2]);

        let loaded = store.load(session.id).await.unwrap();
        assert_eq!(loaded.last_seq, 2);
        assert_eq!(loaded.last_turn_id, TurnId(2));
    }
}
