# AGENTS.md — agent 模块

> 适用于 `src/agent.rs` 与 `src/agent/`。

- `agent_loop.rs` 负责单回合生命周期、会话锁、取消、最大步数、模型调用和工具循环。锁只保护会话状态，不跨模型或工具调用持有。
- `session.rs` 负责 Session、事件类型、事件折叠和 `SessionService`。事件日志保存历史事实，不保存第二份消息快照，也不保存 Application 当前流程游标。
- 每个回合必须有一个 `turn_ended`。取消使用 `Interrupted`，达到最大步数使用 `MaxSteps`，模型或工具失败使用 `Error`。半截回合在下一次启动或完成阶段前修复。
- `context.rs` 只把事件和当前阶段数据投影为模型上下文。`message.rs` 只维护消息数据形状，不能承载模型供应商实现。
- `stage.rs` 只把 `StageKind` 映射到角色。阶段顺序和位置来自 `pipeline`，不要在 agent 内复制一套阶段表。
- `role.rs` 与 `role/prompts/` 维护角色枚举和提示词。提示词正文、简历正文、工具原始参数不进入日志。
- `tool.rs` 负责工具契约、参数校验、执行状态和工具注册表。生产工具注册必须有明确需求，工具取消和失败都要写可追踪结果。
- `agent` 不直接访问 PostgreSQL、文件系统或供应商 SDK 实现，只依赖端口和数据类型。
- 测试覆盖锁冲突、取消、空响应、未知工具、工具失败、半截回合、事件折叠和上下文投影。
