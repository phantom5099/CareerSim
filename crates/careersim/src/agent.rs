//! AgentLoop 的会话、角色、上下文与工具。

pub mod agent_loop;
pub mod context;
pub mod message;
pub mod role;
pub mod session;
pub mod stage;
pub mod tool;

pub use agent_loop::{AgentLoop, LoopConfig, LoopError, TurnResult};
pub use context::{
    BuildOptions, CandidateBrief, CompanyBrief, ContextBuilder, ContextError,
    DefaultContextBuilder, JobBrief, PriorStageSummary, ResumeBrief, StageBrief, TurnContext,
};
pub use message::Message;
pub use role::Role;
pub use session::{
    ApplicationId, CompleteStageBatch, Session, SessionBinding, SessionError, SessionEvent,
    SessionEventKind, SessionId, SessionService, SessionSummary, StageCompletion,
    StageEvaluationRecord, StageRef, StoreSessionService, ToolStatus, TurnEndReason, TurnId,
    UserMessageSource,
};
pub use tool::{Tool, ToolCall, ToolContext, ToolError, ToolOutput, ToolRegistry, ToolSpec};
