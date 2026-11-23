# AGENTS.md — llm 模块

> 适用于 `src/llm.rs` 与 `src/llm/`。

- `client.rs` 定义 `LlmClient`、`ChatRequest`、`ChatResponse`、工具选择和 `LlmError`，只表达模型端口与数据形状。
- `openai_compat.rs` 是供应商 SDK 的唯一适配边界，负责消息、工具、错误和取消转换。业务模块不直接依赖 `async_openai`。
- `provider.rs` 维护供应商枚举、基础地址和密钥变量名。密钥只从配置进入客户端，不进入 Debug、日志或错误正文。
- 所有模型请求都接收取消令牌并保留结构化耗时和结果分类观测。请求正文、提示词和工具参数必须跳过日志与指标。
- `mock.rs` 仅用于测试，行为需覆盖成功、错误、取消和空响应，不进入生产装配。
