//! 上下文投影测试：阶段隔离、轮次注、工具结果成对与预算阶梯（plan/02 §8.1/§9）。

use std::sync::Arc;

use super::BuildOptions;
use super::ContextBuilder;
use super::DefaultContextBuilder;
use super::StageBrief;
use super::TurnContext;
use crate::agent::Message;
use crate::agent::SessionBinding;
use crate::agent::SessionEventKind;
use crate::agent::SessionService;
use crate::agent::StageRef;
use crate::agent::StoreSessionService;
use crate::agent::ToolCall;
use crate::pipeline::EvaluationRubric;
use crate::pipeline::OfferPolicy;
use crate::pipeline::PipelineDefinition;
use crate::pipeline::RubricDimension;
use crate::pipeline::Score;
use crate::pipeline::StageDefinition;
use crate::pipeline::StageEvaluation;
use crate::pipeline::StageKey;
use crate::pipeline::StageKind;
use crate::pipeline::StageOutcome;
use crate::store::MemorySessionStore;

const APPLICATION_ID: &str = "22222222-2222-2222-2222-222222222222";

fn binding() -> SessionBinding {
    SessionBinding::ApplicationStage {
        application_id: APPLICATION_ID.parse().unwrap(),
    }
}

fn stage_ref(key: &str, kind: StageKind) -> StageRef {
    StageRef {
        key: StageKey::parse(key).unwrap(),
        kind,
    }
}

fn rubric() -> EvaluationRubric {
    EvaluationRubric {
        pass_score: Score::new(60).unwrap(),
        dimensions: vec![
            RubricDimension {
                key: "clarity".to_owned(),
                label: "表达".to_owned(),
                weight: 50,
                criteria: "表达是否清晰".to_owned(),
            },
            RubricDimension {
                key: "match".to_owned(),
                label: "匹配".to_owned(),
                weight: 50,
                criteria: "与岗位是否匹配".to_owned(),
            },
        ],
    }
}

fn stage(key: &str, kind: StageKind, min_rounds: u8, max_rounds: u8) -> StageDefinition {
    StageDefinition {
        key: StageKey::parse(key).unwrap(),
        kind,
        title: "测试阶段".to_owned(),
        min_rounds,
        max_rounds,
        rubric: rubric(),
    }
}

/// hr_screen + offer 两阶段流水线；offer 阶段固定 1..=2 轮。
fn pipeline() -> PipelineDefinition {
    PipelineDefinition::parse(vec![
        stage("hr_screen", StageKind::HrScreen, 1, 4),
        stage("offer", StageKind::Offer, 1, 2),
    ])
    .unwrap()
}

fn offer_policy() -> OfferPolicy {
    OfferPolicy {
        city: "杭州".to_owned(),
        salary_min: None,
        salary_max: None,
        months: None,
        deadline_days: 7,
    }
}

fn turn_ctx(key: &str, brief: StageBrief) -> TurnContext {
    let pipeline = pipeline();
    let key = StageKey::parse(key).unwrap();
    TurnContext {
        stage: pipeline.get(&key).unwrap().clone(),
        pipeline,
        brief,
        prior: Vec::new(),
        offer_policy: offer_policy(),
    }
}

fn service() -> StoreSessionService {
    StoreSessionService::new(Arc::new(MemorySessionStore::new()))
}

#[tokio::test]
async fn projects_only_current_stage_with_round_notes() {
    let sessions = service();
    let mut session = sessions
        .create(binding(), stage_ref("hr_screen", StageKind::HrScreen))
        .await
        .unwrap();

    // 第一阶段：一轮对话。
    sessions
        .append(
            &mut session,
            &[
                SessionEventKind::TurnStarted {
                    turn_id: crate::agent::TurnId(1),
                },
                SessionEventKind::UserMessage {
                    turn_id: crate::agent::TurnId(1),
                    content: "你好".to_owned(),
                    source: crate::agent::UserMessageSource::User,
                },
                SessionEventKind::AssistantMessage {
                    turn_id: crate::agent::TurnId(1),
                    content: "请介绍一下你的项目经历。".to_owned(),
                    tool_calls: Vec::new(),
                    usage: None,
                },
                SessionEventKind::TurnEnded {
                    turn_id: crate::agent::TurnId(1),
                    reason: crate::agent::TurnEndReason::Done,
                    steps: 1,
                    error: None,
                },
            ],
        )
        .await
        .unwrap();

    // 完成阶段并推进到 offer。
    sessions
        .append(
            &mut session,
            &[SessionEventKind::StageCompleted {
                turn_id: crate::agent::TurnId(1),
                stage_key: StageKey::parse("hr_screen").unwrap(),
                stage_kind: StageKind::HrScreen,
                evaluation: StageEvaluation {
                    dimensions: Vec::new(),
                    total: Score::new(75).unwrap(),
                    passed: true,
                    summary: "首轮通过".to_owned(),
                },
                outcome: StageOutcome::Advance {
                    next_stage_key: StageKey::parse("offer").unwrap(),
                    next_stage_kind: StageKind::Offer,
                },
            }],
        )
        .await
        .unwrap();

    // 第二阶段：一条用户消息。
    sessions
        .append(
            &mut session,
            &[
                SessionEventKind::TurnStarted {
                    turn_id: crate::agent::TurnId(2),
                },
                SessionEventKind::UserMessage {
                    turn_id: crate::agent::TurnId(2),
                    content: "我们进入下一阶段吧".to_owned(),
                    source: crate::agent::UserMessageSource::User,
                },
            ],
        )
        .await
        .unwrap();

    let session = sessions.load(session.id).await.unwrap();
    let mut ctx = turn_ctx("offer", StageBrief::default());
    ctx.prior = vec![super::PriorStageSummary {
        stage_key: StageKey::parse("hr_screen").unwrap(),
        stage_kind: StageKind::HrScreen,
        total: Score::new(75).unwrap(),
        passed: true,
        summary: "首轮通过".to_owned(),
    }];

    let request = DefaultContextBuilder
        .build(&session, &ctx, &[], BuildOptions::default())
        .await
        .unwrap();

    // 只投影当前阶段：系统提示 + 本阶段一条用户消息。
    assert_eq!(request.messages.len(), 2);
    assert!(matches!(&request.messages[0], Message::System { .. }));
    match &request.messages[1] {
        Message::User { content } => {
            assert!(content.starts_with("我们进入下一阶段吧"));
            // 轮次注按新阶段重新计数。
            assert!(content.ends_with("系统提示：当前为第 1 轮；本阶段至少 1 轮、至多 2 轮。"));
        }
        _ => panic!("second message should be the annotated user message"),
    }
    // 前一阶段的对话不出现。
    assert!(
        !request
            .messages
            .iter()
            .any(|message| message.text().contains("项目经历"))
    );
    // 前序阶段评估摘要注入系统提示。
    match &request.messages[0] {
        Message::System { content } => {
            assert!(content.contains("前序阶段评估"));
            assert!(content.contains("首轮通过"));
        }
        _ => unreachable!(),
    }
}

#[tokio::test]
async fn pairs_tool_results_and_drops_orphans() {
    let sessions = service();
    let mut session = sessions
        .create(binding(), stage_ref("hr_screen", StageKind::HrScreen))
        .await
        .unwrap();
    sessions
        .append(
            &mut session,
            &[
                SessionEventKind::TurnStarted {
                    turn_id: crate::agent::TurnId(1),
                },
                SessionEventKind::UserMessage {
                    turn_id: crate::agent::TurnId(1),
                    content: "查两件事".to_owned(),
                    source: crate::agent::UserMessageSource::User,
                },
                SessionEventKind::AssistantMessage {
                    turn_id: crate::agent::TurnId(1),
                    content: String::new(),
                    tool_calls: vec![
                        ToolCall {
                            call_id: "call_1".to_owned(),
                            name: "a".to_owned(),
                            arguments: "{}".to_owned(),
                        },
                        ToolCall {
                            call_id: "call_2".to_owned(),
                            name: "b".to_owned(),
                            arguments: "{}".to_owned(),
                        },
                    ],
                    usage: None,
                },
                SessionEventKind::ToolResult {
                    turn_id: crate::agent::TurnId(1),
                    tool_call_id: "call_1".to_owned(),
                    status: crate::agent::ToolStatus::Ok,
                    output: "first".to_owned(),
                },
                SessionEventKind::ToolResult {
                    turn_id: crate::agent::TurnId(1),
                    tool_call_id: "orphan".to_owned(),
                    status: crate::agent::ToolStatus::Ok,
                    output: "ignored".to_owned(),
                },
            ],
        )
        .await
        .unwrap();

    let session = sessions.load(session.id).await.unwrap();
    let ctx = turn_ctx("hr_screen", StageBrief::default());
    let request = DefaultContextBuilder
        .build(&session, &ctx, &[], BuildOptions::default())
        .await
        .unwrap();

    assert!(matches!(&request.messages[0], Message::System { .. }));
    assert!(matches!(&request.messages[1], Message::User { .. }));
    assert!(matches!(
        &request.messages[2],
        Message::Assistant { tool_calls, .. } if tool_calls.len() == 2
    ));
    assert_eq!(
        request.messages[3],
        Message::Tool {
            content: "first".to_owned(),
            tool_call_id: "call_1".to_owned(),
        }
    );
    // 缺失结果补 aborted；孤儿结果不进入上下文。
    assert_eq!(
        request.messages[4],
        Message::Tool {
            content: "aborted".to_owned(),
            tool_call_id: "call_2".to_owned(),
        }
    );
    assert_eq!(request.messages.len(), 5);
}

#[tokio::test]
async fn tiny_budget_returns_budget_too_small() {
    let sessions = service();
    let mut session = sessions
        .create(binding(), stage_ref("hr_screen", StageKind::HrScreen))
        .await
        .unwrap();
    sessions
        .append(
            &mut session,
            &[SessionEventKind::TurnStarted {
                turn_id: crate::agent::TurnId(1),
            }],
        )
        .await
        .unwrap();
    let session = sessions.load(session.id).await.unwrap();
    let ctx = turn_ctx("hr_screen", StageBrief::default());

    // 预算小到连系统提示都放不下（输出预留后为 0）。
    let result = DefaultContextBuilder
        .build(
            &session,
            &ctx,
            &[],
            BuildOptions {
                max_prompt_tokens: Some(1),
            },
        )
        .await;

    assert_eq!(result.unwrap_err(), super::ContextError::BudgetTooSmall);
}

#[tokio::test]
async fn budget_truncates_briefs_but_keeps_conversation() {
    let sessions = service();
    let mut session = sessions
        .create(binding(), stage_ref("hr_screen", StageKind::HrScreen))
        .await
        .unwrap();
    sessions
        .append(
            &mut session,
            &[
                SessionEventKind::TurnStarted {
                    turn_id: crate::agent::TurnId(1),
                },
                SessionEventKind::UserMessage {
                    turn_id: crate::agent::TurnId(1),
                    content: "你好".to_owned(),
                    source: crate::agent::UserMessageSource::User,
                },
            ],
        )
        .await
        .unwrap();
    let session = sessions.load(session.id).await.unwrap();

    let mut brief = StageBrief::default();
    brief.job.description = "长".repeat(1000);
    brief.company.description = "司".repeat(1000);
    brief.resume.summary = "历".repeat(1000);
    let ctx = turn_ctx("hr_screen", brief);

    let request = DefaultContextBuilder
        .build(
            &session,
            &ctx,
            &[],
            BuildOptions {
                max_prompt_tokens: Some(512 + 2600),
            },
        )
        .await
        .unwrap();

    // 对话保留；简报字段沿阶梯反复减半至下限（200 字符），系统提示重渲染但不截断。
    // 岗位描述是阶梯第一级，必然被截断到下限。
    match request.messages.last().unwrap() {
        Message::User { content } => {
            assert!(content.starts_with("你好"));
            assert!(content.contains("系统提示：当前为第 1 轮"));
        }
        _ => panic!("conversation should survive the budget ladder"),
    }
    match &request.messages[0] {
        Message::System { content } => {
            assert!(content.contains(&"长".repeat(200)));
            assert!(!content.contains(&"长".repeat(400)));
        }
        _ => unreachable!(),
    }
}

#[tokio::test]
async fn request_prefix_is_stable_across_rounds() {
    let sessions = service();
    let mut session = sessions
        .create(binding(), stage_ref("hr_screen", StageKind::HrScreen))
        .await
        .unwrap();
    sessions
        .append(
            &mut session,
            &[
                SessionEventKind::TurnStarted {
                    turn_id: crate::agent::TurnId(1),
                },
                SessionEventKind::UserMessage {
                    turn_id: crate::agent::TurnId(1),
                    content: "你好".to_owned(),
                    source: crate::agent::UserMessageSource::User,
                },
            ],
        )
        .await
        .unwrap();

    let ctx = turn_ctx("hr_screen", StageBrief::default());
    let session_before = sessions.load(session.id).await.unwrap();
    let first = DefaultContextBuilder
        .build(&session_before, &ctx, &[], BuildOptions::default())
        .await
        .unwrap();

    sessions
        .append(
            &mut session,
            &[SessionEventKind::AssistantMessage {
                turn_id: crate::agent::TurnId(1),
                content: "请介绍你的经历。".to_owned(),
                tool_calls: Vec::new(),
                usage: None,
            }],
        )
        .await
        .unwrap();

    let session_after = sessions.load(session.id).await.unwrap();
    let second = DefaultContextBuilder
        .build(&session_after, &ctx, &[], BuildOptions::default())
        .await
        .unwrap();

    // 系统提示与工具定义完全一致；只追加新消息。
    assert_eq!(first.messages[0], second.messages[0]);
    assert_eq!(first.tools, second.tools);
    assert_eq!(first.messages.len() + 1, second.messages.len());
    // 默认采样参数：自动模式、不覆盖温度与上限。
    assert!(matches!(first.tool_choice, crate::llm::ToolChoice::Auto));
    assert_eq!(first.temperature, None);
    assert_eq!(first.max_tokens, None);
}
