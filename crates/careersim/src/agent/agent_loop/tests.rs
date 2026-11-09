use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::agent::BuildOptions;
use crate::agent::ContextBuilder;
use crate::agent::ContextError;
use crate::agent::DefaultContextBuilder;
use crate::agent::Message;
use crate::agent::Role;
use crate::agent::Session;
use crate::agent::SessionEventKind;
use crate::agent::Tool;
use crate::agent::ToolCall;
use crate::agent::ToolContext;
use crate::agent::ToolError;
use crate::agent::ToolOutput;
use crate::agent::ToolRegistry;
use crate::agent::ToolSpec;
use crate::agent::ToolStatus;
use crate::agent::TurnEndReason;
use crate::agent::TurnId;
use crate::agent::UserMessageSource;
use crate::agent::session::event_turn_id;
use crate::agent::tool::FakeTool;
use crate::llm::ChatResponse;
use crate::llm::LlmError;
use crate::llm::TokenUsage;
use crate::llm::mock::MockLlmClient;
use crate::store::MemorySessionStore;

struct Harness {
    agent: AgentLoop,
    store: Arc<MemorySessionStore>,
    llm: Arc<MockLlmClient>,
}

fn harness(responses: Vec<ChatResponse>) -> Harness {
    harness_with(responses, ToolRegistry::new(), 8)
}

fn harness_with(responses: Vec<ChatResponse>, tools: ToolRegistry, max_steps: usize) -> Harness {
    let llm = Arc::new(MockLlmClient::new(responses));
    let store = Arc::new(MemorySessionStore::new());
    let agent = AgentLoop::new(
        llm.clone(),
        Arc::new(DefaultContextBuilder),
        Arc::new(tools),
        store.clone(),
        LoopConfig { max_steps },
    );
    Harness { agent, store, llm }
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
async fn creates_sessions_with_distinct_ids() {
    let harness = harness(Vec::new());
    let first = harness.agent.create_session(Stage::HrScreen).await.unwrap();
    let second = harness.agent.create_session(Stage::HrScreen).await.unwrap();

    assert_ne!(first.id, second.id);
    assert_eq!(first.events.len(), 1);
    assert_eq!(first.events[0].seq, 0);
    assert!(matches!(
        first.events[0].kind,
        SessionEventKind::SessionMeta { session_id, .. } if session_id == first.id
    ));
}

#[tokio::test]
async fn plain_turn_records_lifecycle_and_usage() {
    let harness = harness(vec![reply("你好，先做个自我介绍吧。")]);
    let session = harness.agent.create_session(Stage::HrScreen).await.unwrap();

    let result = harness
        .agent
        .run_turn(session.id, "你好", CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::Done);
    assert_eq!(result.steps, 1);
    assert_eq!(result.output, "你好，先做个自我介绍吧。");
    assert!(result.error.is_none());

    let session = harness.agent.load_session(session.id).await.unwrap();
    assert_eq!(
        turn_kinds(&session, TurnId(1)),
        vec![
            &SessionEventKind::TurnStarted { turn_id: TurnId(1) },
            &SessionEventKind::UserMessage {
                turn_id: TurnId(1),
                content: "你好".to_owned(),
                source: UserMessageSource::User,
            },
            &SessionEventKind::AssistantMessage {
                turn_id: TurnId(1),
                content: "你好，先做个自我介绍吧。".to_owned(),
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
    assert_eq!(session.last_turn_id, TurnId(1));
    assert!(session.dangling_turn().is_none());
}

#[tokio::test]
async fn tool_calls_loop_until_text_reply() {
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(FakeTool::new("lookup", "42")));
    let harness = harness_with(
        vec![tool_reply("call_1", "lookup", "{}"), reply("结果是 42。")],
        tools,
        8,
    );
    let session = harness.agent.create_session(Stage::Exam).await.unwrap();

    let result = harness
        .agent
        .run_turn(session.id, "查一下", CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::Done);
    assert_eq!(result.steps, 2);
    assert_eq!(result.output, "结果是 42。");

    let session = harness.agent.load_session(session.id).await.unwrap();
    assert!(session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::ToolResult {
            tool_call_id,
            status: ToolStatus::Ok,
            output,
            ..
        } if tool_call_id == "call_1" && output == "42"
    )));

    let requests = harness.llm.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].messages.iter().any(|message| {
        matches!(
            message,
            Message::Tool {
                tool_call_id,
                content,
            } if tool_call_id == "call_1" && content == "42"
        )
    }));
}

#[tokio::test]
async fn unknown_tool_becomes_error_result() {
    let harness = harness(vec![tool_reply("call_x", "missing", "{}"), reply("好的")]);
    let session = harness
        .agent
        .create_session(Stage::TechInterview)
        .await
        .unwrap();

    let result = harness
        .agent
        .run_turn(session.id, "试试", CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::Done);
    let session = harness.agent.load_session(session.id).await.unwrap();
    assert!(session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::ToolResult {
            tool_call_id,
            status: ToolStatus::Error,
            output,
            ..
        } if tool_call_id == "call_x" && output.contains("unknown tool")
    )));
}

#[tokio::test]
async fn max_steps_is_a_normal_result() {
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(FakeTool::new("lookup", "42")));
    let harness = harness_with(vec![tool_reply("call_1", "lookup", "{}")], tools, 1);
    let session = harness.agent.create_session(Stage::Exam).await.unwrap();

    let result = harness
        .agent
        .run_turn(session.id, "查", CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::MaxSteps);
    assert_eq!(result.steps, 1);
    assert_eq!(harness.llm.requests().len(), 1);

    let session = harness.agent.load_session(session.id).await.unwrap();
    assert!(matches!(
        session.events.last().unwrap().kind,
        SessionEventKind::TurnEnded {
            reason: TurnEndReason::MaxSteps,
            steps: 1,
            ..
        }
    ));
}

#[tokio::test]
async fn cancelled_before_turn_skips_model_and_user_message() {
    let harness = harness(Vec::new());
    let session = harness.agent.create_session(Stage::HrScreen).await.unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();

    let result = harness
        .agent
        .run_turn(session.id, "不会发送", cancel)
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::Interrupted);
    assert_eq!(result.steps, 0);
    assert!(harness.llm.requests().is_empty());

    let session = harness.agent.load_session(session.id).await.unwrap();
    assert_eq!(session.events.len(), 3);
    assert!(matches!(
        session.events[1].kind,
        SessionEventKind::TurnStarted { turn_id: TurnId(1) }
    ));
    assert!(matches!(
        session.events[2].kind,
        SessionEventKind::TurnEnded {
            turn_id: TurnId(1),
            reason: TurnEndReason::Interrupted,
            steps: 0,
            ..
        }
    ));
}

#[tokio::test]
async fn cancellation_during_model_call_ends_turn_as_interrupted() {
    let llm = Arc::new(MockLlmClient::wait_for_cancel());
    let store = Arc::new(MemorySessionStore::new());
    let agent = Arc::new(AgentLoop::new(
        llm.clone(),
        Arc::new(DefaultContextBuilder),
        Arc::new(ToolRegistry::new()),
        store.clone(),
        LoopConfig { max_steps: 8 },
    ));
    let session = agent.create_session(Stage::HrScreen).await.unwrap();
    let cancel = CancellationToken::new();

    let handle = tokio::spawn({
        let agent = agent.clone();
        let cancel = cancel.clone();
        let id = session.id;
        async move { agent.run_turn(id, "你好", cancel).await }
    });

    wait_for_requests(&llm, 1).await;
    cancel.cancel();

    let result = handle.await.unwrap().unwrap();
    assert_eq!(result.reason, TurnEndReason::Interrupted);
    assert_eq!(result.steps, 0);

    let session = agent.load_session(session.id).await.unwrap();
    assert!(matches!(
        session.events.last().unwrap().kind,
        SessionEventKind::TurnEnded {
            reason: TurnEndReason::Interrupted,
            ..
        }
    ));
}

#[tokio::test]
async fn empty_response_is_an_error() {
    let harness = harness(vec![reply("")]);
    let session = harness.agent.create_session(Stage::HrScreen).await.unwrap();

    let result = harness
        .agent
        .run_turn(session.id, "你好", CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::Error);
    assert_eq!(result.steps, 1);
    assert_eq!(result.error.as_deref(), Some("empty response"));

    let session = harness.agent.load_session(session.id).await.unwrap();
    assert_eq!(
        session.events.last().unwrap().kind,
        SessionEventKind::TurnEnded {
            turn_id: TurnId(1),
            reason: TurnEndReason::Error,
            steps: 1,
            error: Some("empty response".to_owned()),
        }
    );
}

#[tokio::test]
async fn model_error_keeps_partial_events() {
    let llm = Arc::new(MockLlmClient::from_results(vec![Err(LlmError::Api(
        "boom".to_owned(),
    ))]));
    let store = Arc::new(MemorySessionStore::new());
    let agent = AgentLoop::new(
        llm,
        Arc::new(DefaultContextBuilder),
        Arc::new(ToolRegistry::new()),
        store,
        LoopConfig { max_steps: 8 },
    );
    let session = agent.create_session(Stage::HrScreen).await.unwrap();

    let result = agent
        .run_turn(session.id, "你好", CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.reason, TurnEndReason::Error);
    assert_eq!(result.steps, 0);
    assert!(result.error.as_deref().unwrap().contains("boom"));

    let session = agent.load_session(session.id).await.unwrap();
    assert!(matches!(
        session.events[1].kind,
        SessionEventKind::TurnStarted { .. }
    ));
    assert!(matches!(
        session.events[2].kind,
        SessionEventKind::UserMessage { .. }
    ));
    assert!(matches!(
        session.events[3].kind,
        SessionEventKind::TurnEnded {
            reason: TurnEndReason::Error,
            steps: 0,
            ..
        }
    ));
}

#[tokio::test]
async fn busy_session_rejects_second_turn_and_finish_stage() {
    let harness = harness(vec![reply("ok"), reply("ok")]);
    let first = harness.agent.create_session(Stage::HrScreen).await.unwrap();
    let second = harness.agent.create_session(Stage::HrScreen).await.unwrap();

    let lock = harness.agent.session_lock(first.id);
    let guard = lock.lock().await;

    assert!(matches!(
        harness
            .agent
            .run_turn(first.id, "hi", CancellationToken::new())
            .await,
        Err(LoopError::SessionBusy)
    ));
    assert!(matches!(
        harness.agent.finish_stage(first.id).await,
        Err(LoopError::SessionBusy)
    ));

    // 不同会话使用不同锁，可以继续推进。
    let result = harness
        .agent
        .run_turn(second.id, "hi", CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.reason, TurnEndReason::Done);

    drop(guard);
    harness
        .agent
        .run_turn(first.id, "hi", CancellationToken::new())
        .await
        .unwrap();
}

#[tokio::test]
async fn finish_stage_advances_without_model_call() {
    let harness = harness(vec![reply("HR 回复"), reply("笔试回复")]);
    let session = harness.agent.create_session(Stage::HrScreen).await.unwrap();
    harness
        .agent
        .run_turn(session.id, "你好", CancellationToken::new())
        .await
        .unwrap();
    let requests_before = harness.llm.requests().len();

    let completion = harness.agent.finish_stage(session.id).await.unwrap();
    assert_eq!(completion.stage, Stage::Exam);
    assert_eq!(completion.turn_id, TurnId(2));
    assert_eq!(harness.llm.requests().len(), requests_before);

    let session = harness.agent.load_session(session.id).await.unwrap();
    assert_eq!(session.stage, Stage::Exam);
    assert_eq!(session.role(), Role::ExamAuthor);
    assert_eq!(
        turn_kinds(&session, TurnId(2)),
        vec![
            &SessionEventKind::TurnStarted { turn_id: TurnId(2) },
            &SessionEventKind::TurnEnded {
                turn_id: TurnId(2),
                reason: TurnEndReason::Done,
                steps: 0,
                error: None,
            },
            &SessionEventKind::StageCompleted {
                turn_id: TurnId(2),
                stage: Stage::HrScreen,
            },
        ]
    );

    let result = harness
        .agent
        .run_turn(session.id, "开始笔试", CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.turn_id, TurnId(3));

    let requests = harness.llm.requests();
    let last = requests.last().unwrap();
    assert_eq!(
        last.messages[0],
        Message::System {
            content: Role::ExamAuthor.system_prompt().to_owned(),
        }
    );
    // 完整历史跨阶段保留。
    assert_eq!(last.messages.len(), 4);
}

#[tokio::test]
async fn review_stage_cannot_advance_but_can_chat() {
    let harness = harness(vec![reply("复盘回复")]);
    let session = harness.agent.create_session(Stage::Review).await.unwrap();
    let events_before = session.events.len();

    assert!(matches!(
        harness.agent.finish_stage(session.id).await,
        Err(LoopError::NoNextStage)
    ));

    let session = harness.agent.load_session(session.id).await.unwrap();
    assert_eq!(session.stage, Stage::Review);
    assert_eq!(session.events.len(), events_before);

    let result = harness
        .agent
        .run_turn(session.id, "总结一下", CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.reason, TurnEndReason::Done);
    assert_eq!(
        harness.llm.requests()[0].messages[0],
        Message::System {
            content: Role::Coach.system_prompt().to_owned(),
        }
    );
}

#[tokio::test]
async fn dangling_turn_is_repaired_before_new_turn() {
    let harness = harness(vec![reply("ok")]);
    let session = harness.agent.create_session(Stage::HrScreen).await.unwrap();
    harness
        .store
        .append(
            session.id,
            &[SessionEventKind::TurnStarted { turn_id: TurnId(1) }],
        )
        .await
        .unwrap();

    let result = harness
        .agent
        .run_turn(session.id, "继续", CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.turn_id, TurnId(2));

    let session = harness.agent.load_session(session.id).await.unwrap();
    assert!(session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::TurnEnded {
            turn_id: TurnId(1),
            reason: TurnEndReason::Interrupted,
            steps: 0,
            ..
        }
    )));
    assert_eq!(session.last_turn_id, TurnId(2));
    assert!(session.dangling_turn().is_none());
}

#[tokio::test]
async fn context_fills_missing_tool_results_and_ignores_orphans() {
    let store = MemorySessionStore::new();
    let session = store.create(Stage::HrScreen).await.unwrap();
    store
        .append(
            session.id,
            &[
                SessionEventKind::TurnStarted { turn_id: TurnId(1) },
                SessionEventKind::UserMessage {
                    turn_id: TurnId(1),
                    content: "查两件事".to_owned(),
                    source: UserMessageSource::User,
                },
                SessionEventKind::AssistantMessage {
                    turn_id: TurnId(1),
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
                    turn_id: TurnId(1),
                    tool_call_id: "call_1".to_owned(),
                    status: ToolStatus::Ok,
                    output: "first".to_owned(),
                },
                SessionEventKind::ToolResult {
                    turn_id: TurnId(1),
                    tool_call_id: "orphan".to_owned(),
                    status: ToolStatus::Ok,
                    output: "ignored".to_owned(),
                },
            ],
        )
        .await
        .unwrap();

    let session = store.load(session.id).await.unwrap();
    let request = DefaultContextBuilder
        .build(&session, &[], BuildOptions::default())
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
    assert_eq!(
        request.messages[4],
        Message::Tool {
            content: "aborted".to_owned(),
            tool_call_id: "call_2".to_owned(),
        }
    );

    assert_eq!(
        DefaultContextBuilder
            .build(
                &session,
                &[],
                BuildOptions {
                    max_prompt_tokens: Some(100),
                },
            )
            .await,
        Err(ContextError::BudgetUnsupported)
    );
}

struct BlockingTool {
    spec: ToolSpec,
    entered: Arc<tokio::sync::Notify>,
}

#[async_trait]
impl Tool for BlockingTool {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    async fn call(
        &self,
        _arguments: serde_json::Value,
        ctx: &ToolContext<'_>,
    ) -> Result<ToolOutput, ToolError> {
        self.entered.notify_one();
        ctx.cancel.cancelled().await;
        Err(ToolError::Cancelled)
    }
}

#[tokio::test]
async fn in_flight_turn_rejects_same_session_but_other_sessions_run() {
    let llm = Arc::new(MockLlmClient::wait_once_then(vec![reply("第二个会话")]));
    let store = Arc::new(MemorySessionStore::new());
    let agent = Arc::new(AgentLoop::new(
        llm.clone(),
        Arc::new(DefaultContextBuilder),
        Arc::new(ToolRegistry::new()),
        store,
        LoopConfig { max_steps: 8 },
    ));
    let busy = agent.create_session(Stage::HrScreen).await.unwrap();
    let other = agent.create_session(Stage::HrScreen).await.unwrap();
    let cancel = CancellationToken::new();

    let handle = tokio::spawn({
        let agent = agent.clone();
        let cancel = cancel.clone();
        let id = busy.id;
        async move { agent.run_turn(id, "占住", cancel).await }
    });

    // 第一次模型调用已经在进行中，会话锁由这次真实回合持有。
    wait_for_requests(&llm, 1).await;
    assert!(matches!(
        agent
            .run_turn(busy.id, "并发", CancellationToken::new())
            .await,
        Err(LoopError::SessionBusy)
    ));
    assert!(matches!(
        agent.finish_stage(busy.id).await,
        Err(LoopError::SessionBusy)
    ));

    // 不同会话不受影响，仍在同一时刻完成。
    let result = agent
        .run_turn(other.id, "并行", CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.reason, TurnEndReason::Done);
    assert_eq!(result.output, "第二个会话");

    cancel.cancel();
    let result = handle.await.unwrap().unwrap();
    assert_eq!(result.reason, TurnEndReason::Interrupted);
}

#[tokio::test]
async fn api_error_after_cancel_is_reported_as_interrupted() {
    let llm = Arc::new(MockLlmClient::wait_for_cancel_api());
    let agent = Arc::new(AgentLoop::new(
        llm.clone(),
        Arc::new(DefaultContextBuilder),
        Arc::new(ToolRegistry::new()),
        Arc::new(MemorySessionStore::new()),
        LoopConfig { max_steps: 8 },
    ));
    let session = agent.create_session(Stage::HrScreen).await.unwrap();
    let cancel = CancellationToken::new();

    let handle = tokio::spawn({
        let agent = agent.clone();
        let cancel = cancel.clone();
        let id = session.id;
        async move { agent.run_turn(id, "你好", cancel).await }
    });

    wait_for_requests(&llm, 1).await;
    cancel.cancel();

    let result = handle.await.unwrap().unwrap();
    assert_eq!(result.reason, TurnEndReason::Interrupted);
    assert_eq!(result.steps, 0);
    assert!(result.error.is_none());

    let session = agent.load_session(session.id).await.unwrap();
    assert_eq!(
        session.events.last().unwrap().kind,
        SessionEventKind::TurnEnded {
            turn_id: TurnId(1),
            reason: TurnEndReason::Interrupted,
            steps: 0,
            error: None,
        }
    );
}

#[tokio::test]
async fn finish_stage_repairs_dangling_turn_before_advancing() {
    let harness = harness(Vec::new());
    let session = harness.agent.create_session(Stage::HrScreen).await.unwrap();
    harness
        .store
        .append(
            session.id,
            &[SessionEventKind::TurnStarted { turn_id: TurnId(1) }],
        )
        .await
        .unwrap();

    let completion = harness.agent.finish_stage(session.id).await.unwrap();
    assert_eq!(completion.stage, Stage::Exam);
    assert_eq!(completion.turn_id, TurnId(2));
    assert!(harness.llm.requests().is_empty());

    let session = harness.agent.load_session(session.id).await.unwrap();
    assert_eq!(
        session.events[session.events.len() - 4..]
            .iter()
            .map(|event| event.kind.clone())
            .collect::<Vec<_>>(),
        vec![
            SessionEventKind::TurnEnded {
                turn_id: TurnId(1),
                reason: TurnEndReason::Interrupted,
                steps: 0,
                error: None,
            },
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
        ]
    );
}

#[tokio::test]
async fn finish_stage_repairs_dangling_turn_at_review_but_does_not_advance() {
    let harness = harness(Vec::new());
    let session = harness.agent.create_session(Stage::Review).await.unwrap();
    harness
        .store
        .append(
            session.id,
            &[SessionEventKind::TurnStarted { turn_id: TurnId(1) }],
        )
        .await
        .unwrap();

    assert!(matches!(
        harness.agent.finish_stage(session.id).await,
        Err(LoopError::NoNextStage)
    ));

    let session = harness.agent.load_session(session.id).await.unwrap();
    assert_eq!(session.stage, Stage::Review);
    assert!(session.dangling_turn().is_none());
    assert_eq!(
        session.events.last().unwrap().kind,
        SessionEventKind::TurnEnded {
            turn_id: TurnId(1),
            reason: TurnEndReason::Interrupted,
            steps: 0,
            error: None,
        }
    );
    assert!(
        !session
            .events
            .iter()
            .any(|event| matches!(event.kind, SessionEventKind::StageCompleted { .. }))
    );
}

#[tokio::test]
async fn cancellation_during_tool_wins_over_max_steps() {
    let llm = Arc::new(MockLlmClient::new(vec![tool_reply("call_1", "slow", "{}")]));
    let entered = Arc::new(tokio::sync::Notify::new());
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(BlockingTool {
        spec: ToolSpec {
            name: "slow".to_owned(),
            description: "waits for cancellation".to_owned(),
            parameters: serde_json::json!({"type": "object"}),
        },
        entered: entered.clone(),
    }));
    let agent = Arc::new(AgentLoop::new(
        llm.clone(),
        Arc::new(DefaultContextBuilder),
        Arc::new(tools),
        Arc::new(MemorySessionStore::new()),
        LoopConfig { max_steps: 1 },
    ));
    let session = agent.create_session(Stage::Exam).await.unwrap();
    let cancel = CancellationToken::new();

    let handle = tokio::spawn({
        let agent = agent.clone();
        let cancel = cancel.clone();
        let id = session.id;
        async move { agent.run_turn(id, "慢工具", cancel).await }
    });

    wait_for_requests(&llm, 1).await;
    entered.notified().await;
    cancel.cancel();

    let result = handle.await.unwrap().unwrap();
    assert_eq!(result.reason, TurnEndReason::Interrupted);
    assert_eq!(result.steps, 1);

    let session = agent.load_session(session.id).await.unwrap();
    assert!(session.events.iter().any(|event| matches!(
        &event.kind,
        SessionEventKind::ToolResult {
            tool_call_id,
            status: ToolStatus::Error,
            output,
            ..
        } if tool_call_id == "call_1" && output == "aborted"
    )));
    assert!(matches!(
        session.events.last().unwrap().kind,
        SessionEventKind::TurnEnded {
            reason: TurnEndReason::Interrupted,
            ..
        }
    ));
}
