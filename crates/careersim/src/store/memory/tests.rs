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
