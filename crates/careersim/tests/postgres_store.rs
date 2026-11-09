//! PostgreSQL 集成测试；未设置 `DATABASE_URL` 时跳过。

use std::sync::Arc;

use careersim::agent::SessionError;
use careersim::agent::SessionEventKind;
use careersim::agent::SessionService;
use careersim::agent::Stage;
use careersim::agent::StoreSessionService;
use careersim::agent::TurnEndReason;
use careersim::agent::TurnId;
use careersim::agent::UserMessageSource;
use careersim::postgres_session_service;
use sqlx::postgres::PgPoolOptions;

async fn service() -> Option<StoreSessionService> {
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
    Some(postgres_session_service(pool))
}

#[tokio::test]
async fn create_load_list_delete_roundtrip() {
    let Some(sessions) = service().await else {
        return;
    };
    let session = sessions.create(Stage::Exam).await.unwrap();

    let loaded = sessions.load(session.id).await.unwrap();
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

    let summaries = sessions.list(100).await.unwrap();
    assert!(summaries.iter().any(|summary| summary.id == session.id));

    sessions.delete(session.id).await.unwrap();
    assert!(matches!(
        sessions.load(session.id).await,
        Err(SessionError::NotFound)
    ));
}

#[tokio::test]
async fn append_allocates_seq_and_advances_stage() {
    let Some(sessions) = service().await else {
        return;
    };
    let mut session = sessions.create(Stage::HrScreen).await.unwrap();

    sessions
        .append(
            &mut session,
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
        session
            .events
            .iter()
            .map(|event| event.seq)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );

    let mut stale = session.clone();
    sessions
        .append(
            &mut session,
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
        session.events[5..]
            .iter()
            .map(|event| event.seq)
            .collect::<Vec<_>>(),
        vec![5, 6, 7]
    );

    let loaded = sessions.load(session.id).await.unwrap();
    assert_eq!(loaded.stage, Stage::Exam);
    assert_eq!(loaded.last_seq, 7);
    assert_eq!(loaded.last_turn_id, TurnId(2));
    assert_eq!(loaded.events.len(), 8);

    // 旧投影尝试结束旧阶段，由 CAS 拒绝。
    let conflict = sessions
        .append(
            &mut stale,
            &[SessionEventKind::StageCompleted {
                turn_id: TurnId(3),
                stage: Stage::HrScreen,
            }],
        )
        .await
        .unwrap_err();
    assert!(matches!(conflict, SessionError::Conflict));

    let loaded = sessions.load(session.id).await.unwrap();
    assert_eq!(loaded.stage, Stage::Exam);

    sessions.delete(session.id).await.unwrap();
}

#[tokio::test]
async fn append_rejects_invalid_batches() {
    let Some(sessions) = service().await else {
        return;
    };
    let mut session = sessions.create(Stage::HrScreen).await.unwrap();
    let id = session.id;

    let meta = sessions
        .append(
            &mut session,
            &[SessionEventKind::SessionMeta {
                session_id: id,
                stage: Stage::Exam,
            }],
        )
        .await
        .unwrap_err();
    assert!(matches!(meta, SessionError::InvalidEvent(_)));

    let not_last = sessions
        .append(
            &mut session,
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
    assert!(matches!(not_last, SessionError::InvalidEvent(_)));

    let mut review = sessions.create(Stage::Review).await.unwrap();
    let final_stage = sessions
        .append(
            &mut review,
            &[SessionEventKind::StageCompleted {
                turn_id: TurnId(1),
                stage: Stage::Review,
            }],
        )
        .await
        .unwrap_err();
    assert!(matches!(final_stage, SessionError::InvalidEvent(_)));

    sessions.delete(session.id).await.unwrap();
    sessions.delete(review.id).await.unwrap();
}

#[tokio::test]
async fn concurrent_appends_receive_conflict_for_stale_writer() {
    let Some(sessions) = service().await else {
        return;
    };
    let sessions = Arc::new(sessions);
    let session = sessions.create(Stage::HrScreen).await.unwrap();

    let mut first_session = sessions.load(session.id).await.unwrap();
    let mut second_session = sessions.load(session.id).await.unwrap();

    let first = {
        let sessions = sessions.clone();
        tokio::spawn(async move {
            sessions
                .append(
                    &mut first_session,
                    &[SessionEventKind::TurnStarted { turn_id: TurnId(1) }],
                )
                .await
        })
    };
    let second = {
        let sessions = sessions.clone();
        tokio::spawn(async move {
            sessions
                .append(
                    &mut second_session,
                    &[SessionEventKind::TurnStarted { turn_id: TurnId(2) }],
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

    let loaded = sessions.load(session.id).await.unwrap();
    assert_eq!(loaded.last_seq, 1);
    assert_eq!(loaded.events.len(), 2);

    sessions.delete(session.id).await.unwrap();
}
