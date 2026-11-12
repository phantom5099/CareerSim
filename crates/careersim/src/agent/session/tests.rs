use std::sync::Arc;

use chrono::Utc;

use super::*;
use crate::pipeline::DimensionScore;
use crate::pipeline::Score;
use crate::pipeline::StageEvaluation;
use crate::pipeline::StageKey;
use crate::pipeline::StageKind;
use crate::pipeline::StageOutcome;
use crate::store::EVENT_VERSION;
use crate::store::MemorySessionStore;

fn service() -> StoreSessionService {
    StoreSessionService::new(Arc::new(MemorySessionStore::new()))
}

fn stage_ref(key: &str, kind: StageKind) -> StageRef {
    StageRef {
        key: StageKey::parse(key).unwrap(),
        kind,
    }
}

fn evaluation(total: u8, passed: bool) -> StageEvaluation {
    StageEvaluation {
        dimensions: vec![DimensionScore {
            key: "match".to_owned(),
            score: Score::new(total).unwrap(),
            note: "依据".to_owned(),
        }],
        total: Score::new(total).unwrap(),
        passed,
        summary: "总评".to_owned(),
    }
}

fn binding() -> SessionBinding {
    SessionBinding::ApplicationStage {
        application_id: ApplicationId::from_uuid(uuid::Uuid::new_v4()),
    }
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

#[test]
fn application_id_roundtrips() {
    let id = ApplicationId::from_uuid(uuid::Uuid::new_v4());
    assert_eq!(id.to_string().parse::<ApplicationId>().unwrap(), id);
}

#[tokio::test]
async fn create_records_binding_and_initial_stage() {
    let sessions = service();
    let app_id = ApplicationId::from_uuid(uuid::Uuid::new_v4());
    let session = sessions
        .create(
            SessionBinding::ApplicationStage {
                application_id: app_id,
            },
            stage_ref("hr_screen", StageKind::HrScreen),
        )
        .await
        .unwrap();

    assert_eq!(session.binding.application_id(), app_id);
    assert_eq!(session.current_stage.key.as_str(), "hr_screen");
    assert_eq!(session.current_stage.kind, StageKind::HrScreen);
    assert_eq!(session.role(), Role::HrScreener);

    let loaded = sessions.load(session.id).await.unwrap();
    assert_eq!(loaded.binding, session.binding);
    assert_eq!(loaded.current_stage, session.current_stage);
}

#[tokio::test]
async fn append_rejects_invalid_batches() {
    let sessions = service();
    let mut session = sessions
        .create(binding(), stage_ref("hr_screen", StageKind::HrScreen))
        .await
        .unwrap();

    let current_id = session.id;
    let meta = sessions
        .append(
            &mut session,
            &[SessionEventKind::SessionMeta {
                session_id: current_id,
                binding: binding(),
                initial_stage: stage_ref("exam", StageKind::Exam),
            }],
        )
        .await
        .unwrap_err();
    assert!(matches!(meta, SessionError::InvalidEvent(_)));

    // StageCompleted 必须是批次最后。
    let not_last = sessions
        .append(
            &mut session,
            &[
                SessionEventKind::StageCompleted {
                    turn_id: TurnId(1),
                    stage_key: StageKey::parse("hr_screen").unwrap(),
                    stage_kind: StageKind::HrScreen,
                    evaluation: evaluation(80, true),
                    outcome: StageOutcome::Advance {
                        next_stage_key: StageKey::parse("exam").unwrap(),
                        next_stage_kind: StageKind::Exam,
                    },
                },
                SessionEventKind::TurnStarted { turn_id: TurnId(1) },
            ],
        )
        .await
        .unwrap_err();
    assert!(matches!(not_last, SessionError::InvalidEvent(_)));

    // stage_key 必须等于当前阶段。
    let wrong_stage = sessions
        .append(
            &mut session,
            &[SessionEventKind::StageCompleted {
                turn_id: TurnId(1),
                stage_key: StageKey::parse("exam").unwrap(),
                stage_kind: StageKind::HrScreen,
                evaluation: evaluation(80, true),
                outcome: StageOutcome::Advance {
                    next_stage_key: StageKey::parse("exam").unwrap(),
                    next_stage_kind: StageKind::Exam,
                },
            }],
        )
        .await
        .unwrap_err();
    assert!(matches!(wrong_stage, SessionError::InvalidEvent(_)));

    // stage_kind 必须等于当前阶段。
    let wrong_kind = sessions
        .append(
            &mut session,
            &[SessionEventKind::StageCompleted {
                turn_id: TurnId(1),
                stage_key: StageKey::parse("hr_screen").unwrap(),
                stage_kind: StageKind::Exam,
                evaluation: evaluation(80, true),
                outcome: StageOutcome::Advance {
                    next_stage_key: StageKey::parse("exam").unwrap(),
                    next_stage_kind: StageKind::Exam,
                },
            }],
        )
        .await
        .unwrap_err();
    assert!(matches!(wrong_kind, SessionError::InvalidEvent(_)));

    // Advance 的下一阶段不能是当前阶段。
    let self_advance = sessions
        .append(
            &mut session,
            &[SessionEventKind::StageCompleted {
                turn_id: TurnId(1),
                stage_key: StageKey::parse("hr_screen").unwrap(),
                stage_kind: StageKind::HrScreen,
                evaluation: evaluation(80, true),
                outcome: StageOutcome::Advance {
                    next_stage_key: StageKey::parse("hr_screen").unwrap(),
                    next_stage_kind: StageKind::HrScreen,
                },
            }],
        )
        .await
        .unwrap_err();
    assert!(matches!(self_advance, SessionError::InvalidEvent(_)));
}

#[tokio::test]
async fn complete_stage_writes_batch_in_order_and_advances() {
    let sessions = service();
    let mut session = sessions
        .create(binding(), stage_ref("hr_screen", StageKind::HrScreen))
        .await
        .unwrap();

    let completion = sessions
        .complete_stage(
            &mut session,
            CompleteStageBatch {
                turn_id: TurnId(1),
                steps: 2,
                assistant_content: "评估结论".to_owned(),
                assistant_tool_calls: vec![ToolCall {
                    call_id: "call-1".to_owned(),
                    name: "complete_stage".to_owned(),
                    arguments: "{}".to_owned(),
                }],
                usage: None,
                control_call_id: "call-1".to_owned(),
                record: StageEvaluationRecord {
                    turn_id: TurnId(1),
                    stage_key: StageKey::parse("hr_screen").unwrap(),
                    stage_kind: StageKind::HrScreen,
                    evaluation: evaluation(80, true),
                },
                outcome: StageOutcome::Advance {
                    next_stage_key: StageKey::parse("exam").unwrap(),
                    next_stage_kind: StageKind::Exam,
                },
            },
        )
        .await
        .unwrap();

    assert_eq!(completion.turn_id, TurnId(1));
    assert_eq!(completion.record.stage_key.as_str(), "hr_screen");

    let kinds = session
        .events
        .iter()
        .map(|event| match &event.kind {
            SessionEventKind::AssistantMessage { .. } => "assistant",
            SessionEventKind::ToolResult { .. } => "tool_result",
            SessionEventKind::TurnEnded { .. } => "turn_ended",
            SessionEventKind::StageCompleted { .. } => "stage_completed",
            _ => "other",
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            "other",
            "assistant",
            "tool_result",
            "turn_ended",
            "stage_completed"
        ]
    );

    // Advance 后当前阶段推进，行缓存同步为下一阶段键。
    assert_eq!(session.current_stage.key.as_str(), "exam");
    let loaded = sessions.load(session.id).await.unwrap();
    assert_eq!(loaded.current_stage.key.as_str(), "exam");
    assert_eq!(loaded.current_stage.kind, StageKind::Exam);

    // 控制动作的工具结果为 ok。
    let tool_result = session
        .events
        .iter()
        .find_map(|event| match &event.kind {
            SessionEventKind::ToolResult {
                tool_call_id,
                status,
                output,
                ..
            } => Some((tool_call_id.clone(), *status, output.clone())),
            _ => None,
        })
        .unwrap();
    assert_eq!(tool_result.0, "call-1");
    assert_eq!(tool_result.1, ToolStatus::Ok);
    assert_eq!(tool_result.2, "{\"accepted\":true}");
}

#[tokio::test]
async fn rejected_and_offer_pending_keep_current_stage() {
    let sessions = service();
    let mut session = sessions
        .create(binding(), stage_ref("hr_screen", StageKind::HrScreen))
        .await
        .unwrap();

    sessions
        .complete_stage(
            &mut session,
            CompleteStageBatch {
                turn_id: TurnId(1),
                steps: 1,
                assistant_content: String::new(),
                assistant_tool_calls: Vec::new(),
                usage: None,
                control_call_id: "call-1".to_owned(),
                record: StageEvaluationRecord {
                    turn_id: TurnId(1),
                    stage_key: StageKey::parse("hr_screen").unwrap(),
                    stage_kind: StageKind::HrScreen,
                    evaluation: evaluation(40, false),
                },
                outcome: StageOutcome::Rejected,
            },
        )
        .await
        .unwrap();

    assert_eq!(session.current_stage.key.as_str(), "hr_screen");
    let loaded = sessions.load(session.id).await.unwrap();
    assert_eq!(loaded.current_stage.key.as_str(), "hr_screen");
    assert_eq!(loaded.current_stage.kind, StageKind::HrScreen);
}

#[tokio::test]
async fn stale_projection_is_rejected_as_conflict() {
    let sessions = service();
    let mut stale = sessions
        .create(binding(), stage_ref("hr_screen", StageKind::HrScreen))
        .await
        .unwrap();
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

#[tokio::test]
async fn round_counters_span_stage_boundaries() {
    let sessions = service();
    let mut session = sessions
        .create(binding(), stage_ref("hr_screen", StageKind::HrScreen))
        .await
        .unwrap();

    for turn in 1..=2 {
        sessions.start_turn(&mut session).await.unwrap();
        sessions
            .record_user_message(&mut session, TurnId(turn), format!("第 {turn} 问"))
            .await
            .unwrap();
        sessions
            .record_assistant_message(
                &mut session,
                TurnId(turn),
                format!("第 {turn} 答"),
                Vec::new(),
                None,
            )
            .await
            .unwrap();
        sessions
            .end_turn(&mut session, TurnId(turn), TurnEndReason::Done, 1, None)
            .await
            .unwrap();
    }
    assert_eq!(assistant_count_in_stage(&session), 2);
    assert_eq!(user_round_in_stage(&session), 2);

    // 阶段完成后计数归零。
    sessions
        .complete_stage(
            &mut session,
            CompleteStageBatch {
                turn_id: TurnId(2),
                steps: 1,
                assistant_content: String::new(),
                assistant_tool_calls: Vec::new(),
                usage: None,
                control_call_id: "call-2".to_owned(),
                record: StageEvaluationRecord {
                    turn_id: TurnId(2),
                    stage_key: StageKey::parse("hr_screen").unwrap(),
                    stage_kind: StageKind::HrScreen,
                    evaluation: evaluation(80, true),
                },
                outcome: StageOutcome::Advance {
                    next_stage_key: StageKey::parse("exam").unwrap(),
                    next_stage_kind: StageKind::Exam,
                },
            },
        )
        .await
        .unwrap();
    assert_eq!(assistant_count_in_stage(&session), 0);
    assert_eq!(user_round_in_stage(&session), 0);
    assert_eq!(stage_started_at(&session), session.events.len());
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
            binding: binding(),
            initial_stage: stage_ref("hr_screen", StageKind::HrScreen),
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

/// 事件 v1 直接拒绝；错误信息固定为 `unsupported event version 1`（plan/02 §10.1）。
#[test]
fn event_version_one_is_rejected() {
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
    let legacy = StoredEvent {
        v: 1,
        seq: 0,
        at,
        kind: serde_json::json!({
            "type": "session_meta",
            "session_id": id.to_string(),
            "stage": "hr_screen"
        }),
    };

    let error = fold_session(row, vec![legacy]).unwrap_err();
    let SessionError::Corrupt(message) = &error else {
        panic!("expected corrupt, got {error:?}")
    };
    assert!(message.contains("unsupported event version 1"), "{message}");
}
