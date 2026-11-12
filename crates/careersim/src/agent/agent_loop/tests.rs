//! AgentLoop 测试：绑定与首阶段、阶段推进、评估轮、修复与取消（plan/02 §6/§7/§8）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::AgentLoop;
use super::LoopConfig;
use super::LoopError;
use crate::agent::DefaultContextBuilder;
use crate::agent::Message;
use crate::agent::Session;
use crate::agent::SessionBinding;
use crate::agent::SessionEventKind;
use crate::agent::SessionService;
use crate::agent::StageBrief;
use crate::agent::StageRef;
use crate::agent::StoreSessionService;
use crate::agent::ToolCall;
use crate::agent::ToolRegistry;
use crate::agent::ToolStatus;
use crate::agent::TurnContext;
use crate::agent::TurnEndReason;
use crate::agent::TurnId;
use crate::agent::session::event_turn_id;
use crate::agent::tool::testing::FakeTool;
use crate::llm::ChatResponse;
use crate::llm::TokenUsage;
use crate::llm::ToolChoice;
use crate::llm::mock::MockLlmClient;
use crate::pipeline::EvaluationRubric;
use crate::pipeline::OfferPolicy;
use crate::pipeline::PipelineDefinition;
use crate::pipeline::RubricDimension;
use crate::pipeline::Score;
use crate::pipeline::StageDefinition;
use crate::pipeline::StageKey;
use crate::pipeline::StageKind;
use crate::pipeline::StageOutcome;
use crate::store::MemorySessionStore;

const APPLICATION_ID: &str = "11111111-1111-1111-1111-111111111111";

struct Harness {
    agent: AgentLoop,
    sessions: Arc<dyn SessionService>,
    llm: Arc<MockLlmClient>,
}

fn harness(responses: Vec<ChatResponse>) -> Harness {
    harness_with(responses, ToolRegistry::new(), 8)
}

fn harness_with(responses: Vec<ChatResponse>, tools: ToolRegistry, max_steps: usize) -> Harness {
    let llm = Arc::new(MockLlmClient::new(responses));
    let sessions = Arc::new(StoreSessionService::new(
        Arc::new(MemorySessionStore::new()),
    ));
    let agent = AgentLoop::new(
        llm.clone(),
        Arc::new(DefaultContextBuilder),
        Arc::new(tools),
        sessions.clone(),
        LoopConfig { max_steps },
    );
    Harness {
        agent,
        sessions,
        llm,
    }
}

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

fn initial() -> StageRef {
    stage_ref("hr_screen", StageKind::HrScreen)
}

fn offer_initial() -> StageRef {
    stage_ref("offer", StageKind::Offer)
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
fn pipeline(min_rounds: u8, max_rounds: u8) -> PipelineDefinition {
    PipelineDefinition::parse(vec![
        stage("hr_screen", StageKind::HrScreen, min_rounds, max_rounds),
        stage("offer", StageKind::Offer, 1, 2),
    ])
    .unwrap()
}

fn offer_policy() -> OfferPolicy {
    OfferPolicy {
        city: "杭州".to_owned(),
        salary_min: Some(10_000),
        salary_max: Some(30_000),
        months: Some(12),
        deadline_days: 7,
    }
}

fn turn_ctx(pipeline: PipelineDefinition, key: &str) -> TurnContext {
    let key = StageKey::parse(key).unwrap();
    let stage = pipeline.get(&key).unwrap().clone();
    TurnContext {
        stage,
        pipeline,
        brief: StageBrief::default(),
        prior: Vec::new(),
        offer_policy: offer_policy(),
    }
}

fn valid_args() -> String {
    json!({
        "dimensions": [
            {"key": "clarity", "score": 70, "note": "表达清晰"},
            {"key": "match", "score": 80, "note": "匹配良好"}
        ],
        "summary": "候选人整体表现不错"
    })
    .to_string()
}

fn invalid_args() -> String {
    json!({
        "dimensions": [
            {"key": "clarity", "score": 70, "note": "表达清晰"}
        ],
        "summary": "缺少 match 维度"
    })
    .to_string()
}

fn valid_offer_args() -> String {
    json!({
        "dimensions": [
            {"key": "clarity", "score": 70, "note": "表达清晰"},
            {"key": "match", "score": 80, "note": "匹配良好"}
        ],
        "summary": "双方达成一致",
        "offer": {"baseSalary": 20_000, "months": 12, "bonus": "年终两个月"}
    })
    .to_string()
}

fn reply(content: &str) -> ChatResponse {
    ChatResponse {
        content: content.to_owned(),
        tool_calls: Vec::new(),
        usage: Some(TokenUsage {
            input_tokens: 12,
            output_tokens: 4,
        }),
    }
}

fn tool_reply(call_id: &str, name: &str, arguments: &str) -> ChatResponse {
    ChatResponse {
        content: String::new(),
        tool_calls: vec![ToolCall {
            call_id: call_id.to_owned(),
            name: name.to_owned(),
            arguments: arguments.to_owned(),
        }],
        usage: None,
    }
}

fn turn_kinds(session: &Session, turn_id: TurnId) -> Vec<&SessionEventKind> {
    session
        .events
        .iter()
        .filter(|event| event_turn_id(&event.kind) == Some(turn_id))
        .map(|event| &event.kind)
        .collect()
}

async fn wait_for_requests(llm: &MockLlmClient, count: usize) {
    for _ in 0..200 {
        if llm.requests().len() >= count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("mock model was not called");
}

#[tokio::test]
async fn creates_sessions_with_binding_and_initial_stage() {
    let harness = harness(Vec::new());
    let session = harness.sessions.create(binding(), initial()).await.unwrap();

    assert_eq!(session.binding, binding());
    assert_eq!(session.current_stage, initial());
    assert_eq!(session.events.len(), 1);
    assert!(matches!(
        &session.events[0].kind,
        SessionEventKind::SessionMeta {
            session_id,
            binding: event_binding,
            initial_stage,
        } if *session_id == session.id
            && *event_binding == binding()
            && *initial_stage == initial()
    ));
}

#[tokio::test]
async fn stage_mismatch_is_rejected_without_writing_events() {
    let harness = harness(vec![reply("不应被调用")]);
    let session = harness.sessions.create(binding(), initial()).await.unwrap();
    // 上下文声称处于 offer 阶段，而会话仍在 hr_screen。
    let ctx = turn_ctx(pipeline(1, 4), "offer");

    let result = harness
        .agent
        .run_turn(session.id, "你好", &ctx, CancellationToken::new())
        .await;

    assert!(matches!(result, Err(LoopError::StageMismatch { .. })));
    assert!(harness.llm.requests().is_empty());

    let session = harness.sessions.load(session.id).await.unwrap();
    assert_eq!(session.events.len(), 1);
    assert_eq!(session.current_stage, initial());
}

#[tokio::test]
async fn stage_kind_change_on_same_key_is_rejected() {
    let harness = harness(vec![reply("不应被调用")]);
    let session = harness.sessions.create(binding(), initial()).await.unwrap();
    // 流水线版本把同名阶段 hr_screen 从 hr_screen 改成 exam：
    // key 相同但 kind 不同，事件 kind 必须与实际所用 rubric 一致，必须拒绝。
    let changed = PipelineDefinition::parse(vec![
        stage("hr_screen", StageKind::Exam, 1, 4),
        stage("offer", StageKind::Offer, 1, 2),
    ])
    .unwrap();
    let ctx = turn_ctx(changed, "hr_screen");

    let result = harness
        .agent
        .run_turn(session.id, "你好", &ctx, CancellationToken::new())
        .await;

    assert!(matches!(result, Err(LoopError::StageMismatch { .. })));
    assert!(harness.llm.requests().is_empty());
}

#[tokio::test]
async fn plain_turn_records_lifecycle_usage_and_context() {
    let harness = harness(vec![reply("你好，请介绍一下你的项目经历。")]);
    let session = harness.sessions.create(binding(), initial()).await.unwrap();
    let ctx = turn_ctx(pipeline(1, 4), "hr_screen");

    let result = harness
        .agent
        .run_turn(session.id, "你好", &ctx, CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::Done);
    assert_eq!(result.steps, 1);
    assert_eq!(result.output, "你好，请介绍一下你的项目经历。");
    assert!(result.error.is_none());
    assert!(result.stage_completion.is_none());

    let requests = harness.llm.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    // 无业务工具时，工具列表只有控制动作。
    assert_eq!(
        request
            .tools
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<Vec<_>>(),
        vec!["complete_stage"]
    );
    assert_eq!(request.tool_choice, ToolChoice::Auto);
    assert_eq!(request.temperature, None);
    assert_eq!(request.max_tokens, None);

    // 系统提示完全静态：含轮次规则，不含“当前为第”。
    match &request.messages[0] {
        Message::System { content } => {
            assert!(content.contains("轮次规则"));
            assert!(!content.contains("当前为第"));
        }
        _ => panic!("first message should be the system prompt"),
    }
    // 用户消息尾部附轮次注。
    match request.messages.last().unwrap() {
        Message::User { content } => {
            assert!(content.starts_with("你好"));
            assert!(content.ends_with("系统提示：当前为第 1 轮；本阶段至少 1 轮、至多 4 轮。"));
        }
        _ => panic!("last message should be the annotated user message"),
    }

    let session = harness.sessions.load(session.id).await.unwrap();
    assert_eq!(
        turn_kinds(&session, TurnId(1)),
        vec![
            &SessionEventKind::TurnStarted { turn_id: TurnId(1) },
            &SessionEventKind::UserMessage {
                turn_id: TurnId(1),
                content: "你好".to_owned(),
                source: crate::agent::UserMessageSource::User,
            },
            &SessionEventKind::AssistantMessage {
                turn_id: TurnId(1),
                content: "你好，请介绍一下你的项目经历。".to_owned(),
                tool_calls: Vec::new(),
                usage: Some(TokenUsage {
                    input_tokens: 12,
                    output_tokens: 4,
                }),
            },
            &SessionEventKind::TurnEnded {
                turn_id: TurnId(1),
                reason: TurnEndReason::Done,
                steps: 1,
                error: None,
            },
        ]
    );
    assert_eq!(session.last_seq, 4);
    assert!(session.dangling_turn().is_none());
}

#[tokio::test]
async fn evaluation_succeeds_and_appends_batch_in_order() {
    let harness = harness(vec![tool_reply(
        "call_eval",
        "complete_stage",
        &valid_args(),
    )]);
    let session = harness.sessions.create(binding(), initial()).await.unwrap();
    let ctx = turn_ctx(pipeline(1, 4), "hr_screen");

    let result = harness
        .agent
        .run_turn(session.id, "你好", &ctx, CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::Done);
    let completion = result.stage_completion.expect("stage should complete");
    assert_eq!(completion.turn_id, TurnId(1));
    assert_eq!(completion.record.stage_key, initial().key);
    assert_eq!(completion.record.stage_kind, StageKind::HrScreen);
    assert_eq!(completion.record.evaluation.total.value(), 75);
    assert!(completion.record.evaluation.passed);
    assert_eq!(
        completion.outcome,
        StageOutcome::Advance {
            next_stage_key: StageKey::parse("offer").unwrap(),
            next_stage_kind: StageKind::Offer,
        }
    );

    let session = harness.sessions.load(session.id).await.unwrap();
    assert_eq!(session.current_stage, stage_ref("offer", StageKind::Offer));
    assert_eq!(
        turn_kinds(&session, TurnId(1)),
        vec![
            &SessionEventKind::TurnStarted { turn_id: TurnId(1) },
            &SessionEventKind::UserMessage {
                turn_id: TurnId(1),
                content: "你好".to_owned(),
                source: crate::agent::UserMessageSource::User,
            },
            &SessionEventKind::AssistantMessage {
                turn_id: TurnId(1),
                content: String::new(),
                tool_calls: vec![ToolCall {
                    call_id: "call_eval".to_owned(),
                    name: "complete_stage".to_owned(),
                    arguments: valid_args(),
                }],
                usage: None,
            },
            &SessionEventKind::ToolResult {
                turn_id: TurnId(1),
                tool_call_id: "call_eval".to_owned(),
                status: ToolStatus::Ok,
                output: "{\"accepted\":true}".to_owned(),
            },
            &SessionEventKind::TurnEnded {
                turn_id: TurnId(1),
                reason: TurnEndReason::Done,
                steps: 1,
                error: None,
            },
            &SessionEventKind::StageCompleted {
                turn_id: TurnId(1),
                stage_key: StageKey::parse("hr_screen").unwrap(),
                stage_kind: StageKind::HrScreen,
                evaluation: completion.record.evaluation.clone(),
                outcome: StageOutcome::Advance {
                    next_stage_key: StageKey::parse("offer").unwrap(),
                    next_stage_kind: StageKind::Offer,
                },
            },
        ]
    );

    // 对话轮请求保持自动模式（本回合未达到 max_rounds）。
    let requests = harness.llm.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].tool_choice, ToolChoice::Auto);
    assert_eq!(requests[0].temperature, None);
}

#[tokio::test]
async fn min_rounds_blocks_then_lets_the_model_continue() {
    let harness = harness(vec![
        tool_reply("call_1", "complete_stage", &valid_args()),
        reply("我们继续聊聊你的项目吧。"),
    ]);
    let session = harness.sessions.create(binding(), initial()).await.unwrap();
    let ctx = turn_ctx(pipeline(2, 4), "hr_screen");

    let result = harness
        .agent
        .run_turn(session.id, "直接给我结论", &ctx, CancellationToken::new())
        .await
        .unwrap();

    // stage_not_ready 引导模型继续对话，回合正常结束。
    assert_eq!(result.reason, TurnEndReason::Done);
    assert_eq!(result.output, "我们继续聊聊你的项目吧。");
    assert!(result.stage_completion.is_none());

    let session = harness.sessions.load(session.id).await.unwrap();
    assert_eq!(session.current_stage, initial());
    assert!(
        !session
            .events
            .iter()
            .any(|event| matches!(event.kind, SessionEventKind::StageCompleted { .. }))
    );
    assert!(session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::ToolResult {
            status: ToolStatus::Error,
            output,
            ..
        } if output.contains("stage_not_ready")
    )));

    let requests = harness.llm.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].tool_choice, ToolChoice::Auto);
    assert_eq!(requests[0].temperature, None);
}

#[tokio::test]
async fn invalid_output_consumes_one_repair_then_succeeds() {
    let harness = harness(vec![
        tool_reply("call_1", "complete_stage", &invalid_args()),
        tool_reply("call_2", "complete_stage", &valid_args()),
    ]);
    let session = harness.sessions.create(binding(), initial()).await.unwrap();
    let ctx = turn_ctx(pipeline(1, 4), "hr_screen");

    let result = harness
        .agent
        .run_turn(session.id, "你好", &ctx, CancellationToken::new())
        .await
        .unwrap();

    assert!(result.stage_completion.is_some());
    assert_eq!(result.steps, 2);

    let session = harness.sessions.load(session.id).await.unwrap();
    assert_eq!(session.current_stage, stage_ref("offer", StageKind::Offer));
    assert!(session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::ToolResult {
            tool_call_id,
            status: ToolStatus::Error,
            output,
            ..
        } if tool_call_id == "call_1" && output.contains("invalid_evaluation")
    )));
    assert!(session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::ToolResult {
            tool_call_id,
            status: ToolStatus::Ok,
            ..
        } if tool_call_id == "call_2"
    )));
}

#[tokio::test]
async fn second_invalid_output_ends_turn_without_stage_completed() {
    let harness = harness(vec![
        tool_reply("call_1", "complete_stage", &invalid_args()),
        tool_reply("call_2", "complete_stage", &invalid_args()),
    ]);
    let session = harness.sessions.create(binding(), initial()).await.unwrap();
    let ctx = turn_ctx(pipeline(1, 4), "hr_screen");

    let result = harness
        .agent
        .run_turn(session.id, "你好", &ctx, CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::Error);
    assert_eq!(result.error.as_deref(), Some("evaluation_failed"));
    assert_eq!(result.steps, 2);
    assert!(result.stage_completion.is_none());

    let session = harness.sessions.load(session.id).await.unwrap();
    assert_eq!(session.current_stage, initial());
    let error_calls = session
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            SessionEventKind::ToolResult {
                tool_call_id,
                status: ToolStatus::Error,
                ..
            } => Some(tool_call_id.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(error_calls, vec!["call_1".to_owned(), "call_2".to_owned()]);
    assert!(
        !session
            .events
            .iter()
            .any(|event| matches!(event.kind, SessionEventKind::StageCompleted { .. }))
    );
    assert!(matches!(
        &session.events.last().unwrap().kind,
        SessionEventKind::TurnEnded {
            turn_id: TurnId(1),
            reason: TurnEndReason::Error,
            steps: 2,
            error: Some(message),
        } if message == "evaluation_failed"
    ));
}

#[tokio::test]
async fn mixed_control_and_business_calls_are_rejected_without_executing() {
    let mut tools = ToolRegistry::new();
    tools
        .register(Arc::new(FakeTool::new("search", "搜索结果")))
        .unwrap();
    let mixed = ChatResponse {
        content: String::new(),
        tool_calls: vec![
            ToolCall {
                call_id: "call_stage".to_owned(),
                name: "complete_stage".to_owned(),
                arguments: valid_args(),
            },
            ToolCall {
                call_id: "call_search".to_owned(),
                name: "search".to_owned(),
                arguments: "{}".to_owned(),
            },
        ],
        usage: None,
    };
    let harness = harness_with(
        vec![
            mixed,
            tool_reply("call_fix", "complete_stage", &valid_args()),
        ],
        tools,
        8,
    );
    let session = harness.sessions.create(binding(), initial()).await.unwrap();
    let ctx = turn_ctx(pipeline(1, 4), "hr_screen");

    let result = harness
        .agent
        .run_turn(session.id, "查一下顺便提交", &ctx, CancellationToken::new())
        .await
        .unwrap();

    // 修复后仍可完成阶段。
    assert!(result.stage_completion.is_some());

    let session = harness.sessions.load(session.id).await.unwrap();
    // 整条响应被拒绝：两个调用都得到错误结果。
    assert!(session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::ToolResult {
            tool_call_id,
            status: ToolStatus::Error,
            output,
            ..
        } if tool_call_id == "call_stage" && output.contains("control_action_must_be_alone")
    )));
    assert!(session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::ToolResult {
            tool_call_id,
            status: ToolStatus::Error,
            output,
            ..
        } if tool_call_id == "call_search" && output.contains("control_action_must_be_alone")
    )));
    // 业务工具从未执行。
    assert!(!session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::ToolResult { output, .. } if output == "搜索结果"
    )));
}

#[tokio::test]
async fn max_rounds_forces_evaluation_turn() {
    let harness = harness(vec![
        reply("第一轮对话。"),
        tool_reply("call_eval", "complete_stage", &valid_args()),
    ]);
    let session = harness.sessions.create(binding(), initial()).await.unwrap();
    let ctx = turn_ctx(pipeline(1, 1), "hr_screen");

    let first = harness
        .agent
        .run_turn(session.id, "你好", &ctx, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(first.reason, TurnEndReason::Done);
    assert!(first.stage_completion.is_none());

    // 已有 1 条助手消息：下一请求达到 max_rounds，进入强制评估轮。
    let second = harness
        .agent
        .run_turn(session.id, "继续", &ctx, CancellationToken::new())
        .await
        .unwrap();
    assert!(second.stage_completion.is_some());

    let requests = harness.llm.requests();
    assert_eq!(requests.len(), 2);
    // 对话轮：自动模式、无采样覆盖。
    assert_eq!(requests[0].tool_choice, ToolChoice::Auto);
    assert_eq!(requests[0].temperature, None);
    // 评估轮：只给控制动作、强制函数调用、低温度。
    assert_eq!(
        requests[1]
            .tools
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<Vec<_>>(),
        vec!["complete_stage"]
    );
    assert_eq!(
        requests[1].tool_choice,
        ToolChoice::Function {
            name: "complete_stage".to_owned()
        }
    );
    assert_eq!(requests[1].temperature, Some(0.2));
}

#[tokio::test]
async fn offer_stage_builds_terms_from_policy() {
    let harness = harness(vec![tool_reply(
        "call_offer",
        "complete_stage",
        &valid_offer_args(),
    )]);
    let session = harness
        .sessions
        .create(binding(), offer_initial())
        .await
        .unwrap();
    let ctx = turn_ctx(pipeline(1, 2), "offer");

    let result = harness
        .agent
        .run_turn(session.id, "期望月薪 20k", &ctx, CancellationToken::new())
        .await
        .unwrap();

    let completion = result.stage_completion.expect("offer should complete");
    let StageOutcome::OfferPending { offer } = completion.outcome else {
        panic!("expected OfferPending, got {:?}", completion.outcome);
    };
    assert_eq!(offer.base_salary, 20_000);
    assert_eq!(offer.months, 12);
    assert_eq!(offer.city, "杭州");
    assert_eq!(offer.bonus, "年终两个月");
    // 截止日期 = 签发日 + 策略天数；city 与 deadline 均由服务端生成。
    assert_eq!(
        (offer.deadline - offer.issued_at.date_naive()).num_days(),
        7
    );

    // OfferPending 不推进阶段。
    let session = harness.sessions.load(session.id).await.unwrap();
    assert_eq!(session.current_stage, offer_initial());
    assert!(matches!(
        &session.events.last().unwrap().kind,
        SessionEventKind::StageCompleted {
            stage_key,
            outcome: StageOutcome::OfferPending { .. },
            ..
        } if stage_key.as_str() == "offer"
    ));
}

#[tokio::test]
async fn offer_stage_without_terms_is_repaired() {
    let harness = harness(vec![
        tool_reply("call_1", "complete_stage", &valid_args()),
        tool_reply("call_2", "complete_stage", &valid_offer_args()),
    ]);
    let session = harness
        .sessions
        .create(binding(), offer_initial())
        .await
        .unwrap();
    let ctx = turn_ctx(pipeline(1, 2), "offer");

    let result = harness
        .agent
        .run_turn(session.id, "可以谈薪资了", &ctx, CancellationToken::new())
        .await
        .unwrap();

    assert!(result.stage_completion.is_some());

    let session = harness.sessions.load(session.id).await.unwrap();
    assert!(session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::ToolResult {
            tool_call_id,
            status: ToolStatus::Error,
            output,
            ..
        } if tool_call_id == "call_1" && output.contains("invalid_arguments")
    )));
    assert!(session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::ToolResult {
            tool_call_id,
            status: ToolStatus::Ok,
            ..
        } if tool_call_id == "call_2"
    )));
}

#[tokio::test]
async fn cancellation_before_start_is_interrupted() {
    let harness = harness(Vec::new());
    let session = harness.sessions.create(binding(), initial()).await.unwrap();
    let ctx = turn_ctx(pipeline(1, 4), "hr_screen");
    let cancel = CancellationToken::new();
    cancel.cancel();

    let result = harness
        .agent
        .run_turn(session.id, "不会发送", &ctx, cancel)
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::Interrupted);
    assert_eq!(result.steps, 0);
    assert!(result.stage_completion.is_none());
    assert!(harness.llm.requests().is_empty());

    let session = harness.sessions.load(session.id).await.unwrap();
    assert_eq!(session.events.len(), 3);
    assert!(matches!(
        &session.events[1].kind,
        SessionEventKind::TurnStarted { turn_id: TurnId(1) }
    ));
    assert!(matches!(
        &session.events[2].kind,
        SessionEventKind::TurnEnded {
            turn_id: TurnId(1),
            reason: TurnEndReason::Interrupted,
            steps: 0,
            error: None,
        }
    ));
}

#[tokio::test]
async fn max_steps_is_a_normal_result() {
    let mut tools = ToolRegistry::new();
    tools
        .register(Arc::new(FakeTool::new("lookup", "42")))
        .unwrap();
    let harness = harness_with(vec![tool_reply("call_1", "lookup", "{}")], tools, 1);
    let session = harness.sessions.create(binding(), initial()).await.unwrap();
    let ctx = turn_ctx(pipeline(1, 4), "hr_screen");

    let result = harness
        .agent
        .run_turn(session.id, "查", &ctx, CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::MaxSteps);
    assert_eq!(result.steps, 1);
    assert_eq!(harness.llm.requests().len(), 1);

    let session = harness.sessions.load(session.id).await.unwrap();
    assert!(matches!(
        &session.events.last().unwrap().kind,
        SessionEventKind::TurnEnded {
            reason: TurnEndReason::MaxSteps,
            steps: 1,
            ..
        }
    ));
}

#[tokio::test]
async fn dangling_turn_is_repaired_on_next_turn() {
    let harness = harness(vec![reply("好的，我们继续。")]);
    let mut session = harness.sessions.create(binding(), initial()).await.unwrap();
    harness
        .sessions
        .append(
            &mut session,
            &[SessionEventKind::TurnStarted { turn_id: TurnId(1) }],
        )
        .await
        .unwrap();
    let ctx = turn_ctx(pipeline(1, 4), "hr_screen");

    let result = harness
        .agent
        .run_turn(session.id, "继续", &ctx, CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.turn_id, TurnId(2));

    let session = harness.sessions.load(session.id).await.unwrap();
    assert!(session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::TurnEnded {
            turn_id: TurnId(1),
            reason: TurnEndReason::Interrupted,
            steps: 0,
            error: None,
        }
    )));
    assert_eq!(session.last_turn_id, TurnId(2));
    assert!(session.dangling_turn().is_none());
}

#[tokio::test]
async fn busy_session_rejects_second_turn() {
    let llm = Arc::new(MockLlmClient::wait_for_cancel());
    let sessions = Arc::new(StoreSessionService::new(
        Arc::new(MemorySessionStore::new()),
    ));
    let agent = Arc::new(AgentLoop::new(
        llm.clone(),
        Arc::new(DefaultContextBuilder),
        Arc::new(ToolRegistry::new()),
        sessions.clone(),
        LoopConfig { max_steps: 8 },
    ));
    let session = sessions.create(binding(), initial()).await.unwrap();
    let ctx = Arc::new(turn_ctx(pipeline(1, 4), "hr_screen"));
    let cancel = CancellationToken::new();

    let handle = tokio::spawn({
        let agent = agent.clone();
        let ctx = ctx.clone();
        let cancel = cancel.clone();
        let id = session.id;
        async move { agent.run_turn(id, "占住会话", &ctx, cancel).await }
    });

    // 第一次模型调用已经在进行中，会话锁由这个真实回合持有。
    wait_for_requests(&llm, 1).await;
    assert!(matches!(
        agent
            .run_turn(session.id, "并发", &ctx, CancellationToken::new())
            .await,
        Err(LoopError::SessionBusy)
    ));

    cancel.cancel();
    let result = handle.await.unwrap().unwrap();
    assert_eq!(result.reason, TurnEndReason::Interrupted);
    assert_eq!(result.steps, 0);
}

#[tokio::test]
async fn model_error_keeps_partial_events() {
    let llm = Arc::new(MockLlmClient::from_results(vec![Err(
        crate::llm::LlmError::api("boom"),
    )]));
    let sessions = Arc::new(StoreSessionService::new(
        Arc::new(MemorySessionStore::new()),
    ));
    let agent = AgentLoop::new(
        llm,
        Arc::new(DefaultContextBuilder),
        Arc::new(ToolRegistry::new()),
        sessions.clone(),
        LoopConfig { max_steps: 8 },
    );
    let session = sessions.create(binding(), initial()).await.unwrap();
    let ctx = turn_ctx(pipeline(1, 4), "hr_screen");

    let result = agent
        .run_turn(session.id, "你好", &ctx, CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::Error);
    assert_eq!(result.steps, 0);
    assert!(result.error.as_deref().unwrap().contains("boom"));
    assert!(result.stage_completion.is_none());

    let session = sessions.load(session.id).await.unwrap();
    assert!(matches!(
        &session.events[1].kind,
        SessionEventKind::TurnStarted { .. }
    ));
    assert!(matches!(
        &session.events[2].kind,
        SessionEventKind::UserMessage { .. }
    ));
    assert!(matches!(
        &session.events[3].kind,
        SessionEventKind::TurnEnded {
            reason: TurnEndReason::Error,
            steps: 0,
            ..
        }
    ));
}
