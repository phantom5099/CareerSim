use std::sync::Arc;

use chrono::Utc;

use super::*;
use crate::store::EVENT_VERSION;
use crate::store::MemorySessionStore;

fn service() -> StoreSessionService {
    StoreSessionService::new(Arc::new(MemorySessionStore::new()))
}

#[test]
fn session_id_parses_its_display_form() {
    let id = SessionId::generate();
    assert_eq!(id.to_string().parse::<SessionId>().unwrap(), id);
}

#[test]
fn session_id_rejects_invalid_text() {
    assert!("not-a-uuid".parse::<SessionId>().is_err());
    assert!("".parse::<SessionId>().is_err());
}

#[tokio::test]
async fn append_rejects_invalid_batches() {
    let sessions = service();
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

    let wrong_stage = sessions
        .append(
            &mut session,
            &[SessionEventKind::StageCompleted {
                turn_id: TurnId(1),
                stage: Stage::Exam,
            }],
        )
        .await
        .unwrap_err();
    assert!(matches!(wrong_stage, SessionError::InvalidEvent(_)));

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
}

#[tokio::test]
async fn stale_projection_is_rejected_as_conflict() {
    let sessions = service();
    let mut stale = sessions.create(Stage::HrScreen).await.unwrap();
    let mut fresh = sessions.load(stale.id).await.unwrap();

    sessions
        .append(
            &mut fresh,
            &[SessionEventKind::TurnStarted { turn_id: TurnId(1) }],
        )
        .await
        .unwrap();

    let error = sessions
        .append(
            &mut stale,
            &[SessionEventKind::TurnStarted { turn_id: TurnId(1) }],
        )
        .await
        .unwrap_err();
    assert!(matches!(error, SessionError::Conflict));
}

#[test]
fn corrupt_log_is_rejected_on_fold() {
    let id = SessionId::generate();
    let at = Utc::now();
    let row = StoredSession {
        id: id.into_uuid(),
        stage: "hr_screen".to_owned(),
        created_at: at,
        updated_at: at,
        last_seq: 0,
        last_turn_id: 0,
    };
    let meta = StoredEvent {
        v: EVENT_VERSION,
        seq: 0,
        at,
        kind: serde_json::to_value(SessionEventKind::SessionMeta {
            session_id: id,
            stage: Stage::HrScreen,
        })
        .unwrap(),
    };
    assert!(fold_session(row.clone(), vec![meta.clone()]).is_ok());

    let mut bad_version = meta.clone();
    bad_version.v = EVENT_VERSION + 1;
    assert!(matches!(
        fold_session(row.clone(), vec![bad_version]),
        Err(SessionError::Corrupt(_))
    ));

    let gap = StoredEvent {
        v: EVENT_VERSION,
        seq: 2,
        at,
        kind: serde_json::to_value(SessionEventKind::TurnStarted { turn_id: TurnId(1) }).unwrap(),
    };
    assert!(matches!(
        fold_session(row, vec![meta, gap]),
        Err(SessionError::Corrupt(_))
    ));
}
