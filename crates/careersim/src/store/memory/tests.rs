use std::sync::Arc;

use serde_json::json;
use uuid::Uuid;

use super::*;
use crate::store::ExpectedState;

fn started(turn_id: u64) -> serde_json::Value {
    json!({ "type": "turn_started", "turn_id": turn_id })
}

fn completed(turn_id: u64) -> serde_json::Value {
    json!({ "type": "stage_completed", "turn_id": turn_id })
}

#[tokio::test]
async fn create_load_list_delete_roundtrip() {
    let store = MemorySessionStore::new();
    let id = Uuid::new_v4();
    let (session, event) = store
        .create(id, json!({ "type": "session_meta" }))
        .await
        .unwrap();

    assert_eq!(session.id, id);
    assert_eq!(event.seq, 0);

    let (loaded, events) = store.load(id).await.unwrap();
    assert_eq!(loaded.id, id);
    assert_eq!(events.len(), 1);

    let summaries = store.list(10).await.unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, id);

    store.delete(id).await.unwrap();
    assert!(matches!(store.load(id).await, Err(StoreError::NotFound)));
    assert!(matches!(store.delete(id).await, Err(StoreError::NotFound)));
}

#[tokio::test]
async fn append_assigns_seq_and_updates_cache() {
    let store = MemorySessionStore::new();
    let id = Uuid::new_v4();
    store
        .create(id, json!({ "type": "session_meta" }))
        .await
        .unwrap();

    let first = store
        .append(
            id,
            AppendBatch {
                expected: ExpectedState { last_seq: 0 },
                events: &[started(1)],
                last_turn_id: 1,
            },
        )
        .await
        .unwrap();
    assert_eq!(first[0].seq, 1);

    let second = store
        .append(
            id,
            AppendBatch {
                expected: ExpectedState { last_seq: 1 },
                events: &[
                    json!({ "type": "turn_ended", "turn_id": 1, "reason": "done", "steps": 1 }),
                    completed(1),
                ],
                last_turn_id: 1,
            },
        )
        .await
        .unwrap();
    assert_eq!(second[0].seq, 2);
    assert_eq!(second[1].seq, 3);

    let (loaded, events) = store.load(id).await.unwrap();
    assert_eq!(loaded.last_seq, 3);
    assert_eq!(loaded.last_turn_id, 1);
    assert_eq!(events.len(), 4);
}

#[tokio::test]
async fn stale_expected_state_is_rejected() {
    let store = MemorySessionStore::new();
    let id = Uuid::new_v4();
    store
        .create(id, json!({ "type": "session_meta" }))
        .await
        .unwrap();

    store
        .append(
            id,
            AppendBatch {
                expected: ExpectedState { last_seq: 0 },
                events: &[started(1)],
                last_turn_id: 1,
            },
        )
        .await
        .unwrap();

    let stale_seq = store
        .append(
            id,
            AppendBatch {
                expected: ExpectedState { last_seq: 0 },
                events: &[started(2)],
                last_turn_id: 2,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(stale_seq, StoreError::Conflict));
}

#[tokio::test]
async fn concurrent_appends_only_one_wins() {
    let store = Arc::new(MemorySessionStore::new());
    let id = Uuid::new_v4();
    store
        .create(id, json!({ "type": "session_meta" }))
        .await
        .unwrap();

    let first = {
        let store = store.clone();
        tokio::spawn(async move {
            let events = [started(1)];
            store
                .append(
                    id,
                    AppendBatch {
                        expected: ExpectedState { last_seq: 0 },
                        events: &events,
                        last_turn_id: 1,
                    },
                )
                .await
        })
    };
    let second = {
        let store = store.clone();
        tokio::spawn(async move {
            let events = [started(2)];
            store
                .append(
                    id,
                    AppendBatch {
                        expected: ExpectedState { last_seq: 0 },
                        events: &events,
                        last_turn_id: 2,
                    },
                )
                .await
        })
    };

    let first = first.await.unwrap();
    let second = second.await.unwrap();
    assert!(matches!(
        (first.is_ok(), second.is_ok()),
        (true, false) | (false, true)
    ));

    let (loaded, events) = store.load(id).await.unwrap();
    assert_eq!(loaded.last_seq, 1);
    assert_eq!(events.len(), 2);
}
