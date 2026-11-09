//! AgentLoop 的会话、阶段、角色、上下文与工具。

pub mod agent_loop;
pub mod context;
pub mod message;
pub mod role;
pub mod session;
pub mod stage;
pub mod tool;

pub use agent_loop::{AgentLoop, LoopConfig, LoopError, StageCompletion, TurnResult};
pub use context::{BuildOptions, ContextBuilder, ContextError, DefaultContextBuilder};
pub use message::Message;
pub use role::Role;
pub use session::{
    Session, SessionEvent, SessionEventKind, SessionId, SessionSummary, ToolStatus, TurnEndReason,
    TurnId, UserMessageSource,
};
pub use stage::{ParseStageError, Stage};
pub use tool::{Tool, ToolCall, ToolContext, ToolError, ToolOutput, ToolRegistry, ToolSpec};
