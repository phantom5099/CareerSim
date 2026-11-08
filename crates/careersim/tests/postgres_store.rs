//! PostgreSQL 集成测试；未设置 `DATABASE_URL` 时跳过。

use std::sync::Arc;

use careersim::agent::SessionEventKind;
use careersim::agent::Stage;
use careersim::agent::TurnEndReason;
use careersim::agent::TurnId;
use careersim::agent::UserMessageSource;
use careersim::store::PostgresSessionStore;
use careersim::store::SessionStore;
use careersim::store::StoreError;
use sqlx::postgres::PgPoolOptions;

async fn store() -> Option<PostgresSessionStore> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return None;
    };
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("connect to PostgreSQL");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    Some(PostgresSessionStore::new(pool))
}

#[tokio::test]
async fn create_load_list_delete_roundtrip() {
    let Some(store) = store().await else {
        return;
    };
    let session = store.create(Stage::Exam).await.unwrap();

    let loaded = store.load(session.id).await.unwrap();
    assert_eq!(loaded.id, session.id);
    assert_eq!(loaded.stage, Stage::Exam);
    assert_eq!(loaded.events.len(), 1);
    assert_eq!(loaded.events[0].seq, 0);
    assert!(matches!(
        loaded.events[0].kind,
        SessionEventKind::SessionMeta { session_id, stage }
            if session_id == session.id && stage == Stage::Exam
    ));
    assert_eq!(loaded.last_seq, 0);
    assert_eq!(loaded.last_turn_id, TurnId(0));

    let summaries = store.list(100).await.unwrap();
    assert!(summaries.iter().any(|summary| summary.id == session.id));

    store.delete(session.id).await.unwrap();
    assert!(matches!(
        store.load(session.id).await,
        Err(StoreError::NotFound)
    ));
}

#[tokio::test]
async fn append_allocates_seq_and_advances_stage() {
    let Some(store) = store().await else {
        return;
    };
    let session = store.create(Stage::HrScreen).await.unwrap();

    let events = store
        .append(
            session.id,
            &[
                SessionEventKind::TurnStarted { turn_id: TurnId(1) },
                SessionEventKind::UserMessage {
                    turn_id: TurnId(1),
                    content: "你好".to_owned(),
                    source: UserMessageSource::User,
                },
                SessionEventKind::AssistantMessage {
                    turn_id: TurnId(1),
                    content: "欢迎".to_owned(),
                    tool_calls: Vec::new(),
                    usage: None,
                },
                SessionEventKind::TurnEnded {
                    turn_id: TurnId(1),
                    reason: TurnEndReason::Done,
                    steps: 1,
                    error: None,
                },
            ],
        )
        .await
        .unwrap();

    assert_eq!(
        events.iter().map(|event| event.seq).collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );

    let transition = store
        .append(
            session.id,
            &[
                SessionEventKind::TurnStarted { turn_id: TurnId(2) },
                SessionEventKind::TurnEnded {
                    turn_id: TurnId(2),
                    reason: TurnEndReason::Done,
                    steps: 0,
                    error: None,
                },
                SessionEventKind::StageCompleted {
                    turn_id: TurnId(2),
                    stage: Stage::HrScreen,
                },
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        transition.iter().map(|event| event.seq).collect::<Vec<_>>(),
        vec![5, 6, 7]
    );

    let loaded = store.load(session.id).await.unwrap();
    assert_eq!(loaded.stage, Stage::Exam);
    assert_eq!(loaded.last_seq, 7);
    assert_eq!(loaded.last_turn_id, TurnId(2));
    assert_eq!(loaded.events.len(), 8);

    // 旧阶段不能再被结束。
    let stale = store
        .append(
            session.id,
            &[SessionEventKind::StageCompleted {
                turn_id: TurnId(3),
                stage: Stage::HrScreen,
            }],
        )
        .await
        .unwrap_err();
    assert!(matches!(stale, StoreError::InvalidEvent(_)));

    let loaded = store.load(session.id).await.unwrap();
    assert_eq!(loaded.stage, Stage::Exam);

    store.delete(session.id).await.unwrap();
}

#[tokio::test]
async fn append_rejects_invalid_batches() {
    let Some(store) = store().await else {
        return;
    };
    let session = store.create(Stage::HrScreen).await.unwrap();

    let meta = store
        .append(
            session.id,
            &[SessionEventKind::SessionMeta {
                session_id: session.id,
                stage: Stage::Exam,
            }],
        )
        .await
        .unwrap_err();
    assert!(matches!(meta, StoreError::InvalidEvent(_)));

    let not_last = store
        .append(
            session.id,
            &[
                SessionEventKind::StageCompleted {
                    turn_id: TurnId(1),
                    stage: Stage::HrScreen,
                },
                SessionEventKind::TurnStarted { turn_id: TurnId(1) },
            ],
        )
        .await
        .unwrap_err();
    assert!(matches!(not_last, StoreError::InvalidEvent(_)));

    let review = store.create(Stage::Review).await.unwrap();
    let final_stage = store
        .append(
            review.id,
            &[SessionEventKind::StageCompleted {
                turn_id: TurnId(1),
                stage: Stage::Review,
            }],
        )
        .await
        .unwrap_err();
    assert!(matches!(final_stage, StoreError::InvalidEvent(_)));

    store.delete(session.id).await.unwrap();
    store.delete(review.id).await.unwrap();
}

#[tokio::test]
async fn concurrent_appends_receive_unique_contiguous_seq() {
    let Some(store) = store().await else {
        return;
    };
    let store = Arc::new(store);
    let session = store.create(Stage::HrScreen).await.unwrap();

    let first = {
        let store = store.clone();
        let id = session.id;
        tokio::spawn(async move {
            store
                .append(id, &[SessionEventKind::TurnStarted { turn_id: TurnId(1) }])
                .await
        })
    };
    let second = {
        let store = store.clone();
        let id = session.id;
        tokio::spawn(async move {
            store
                .append(id, &[SessionEventKind::TurnStarted { turn_id: TurnId(2) }])
                .await
        })
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

    store.delete(session.id).await.unwrap();
}
