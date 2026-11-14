# AGENTS.md — crates/careersim（Rust 服务端）

> 本目录是服务端主体；仓库级规则、信息优先级与测试禁止类型见根 `AGENTS.md`，
> 本文件只写 Rust 侧约定与实现细节。

---

## 1. 模块地图

```text
src/
├── main.rs            # 进程入口：初始化日志后交给组合根 app::run
├── lib.rs             # 库入口：模块导出与 postgres_session_service 工厂
├── app.rs             # 组合根：配置 → 连接池 → 迁移 → 适配器 → Router → 服务
├── config.rs          # AppConfig / DbConfig / LlmConfig / HttpConfig / Argon2Config
├── error.rs           # ConfigError 与 error_chain
├── testing.rs         # crate 级测试装配：账户/catalog/resume 端口替身 + Router/oneshot（cfg(test)）
├── account.rs         # account 模块入口与公开类型聚合
├── account/
│   ├── model.rs       # UserId/Username/Password/User/ProfilePatch 与字段边界
│   ├── password.rs    # PasswordHasher trait 与 Argon2id 实现
│   ├── session.rs     # 会话令牌、LoginSession 与空闲/绝对超时规则
│   ├── service.rs     # AccountService：注册/登录/注销/资料/改密
│   ├── store.rs       # UserStore / SessionStore trait 与 AccountError
│   ├── postgres.rs    # PostgreSQL 适配器
│   └── tests.rs       # 仅放该模块单测
├── pipeline.rs        # pipeline 模块入口（纯领域，无 IO）
├── pipeline/
│   ├── model.rs       # StageKey/StageKind/StageDefinition/PipelineDefinition/Score
│   ├── rubric.rs      # EvaluationRubric 与加权评估
│   ├── validate.rs    # 纯校验函数与 PipelineError
│   └── tests.rs
├── catalog.rs         # catalog 模块入口与公开类型聚合
├── catalog/
│   ├── model.rs       # 公司/项目/流水线/岗位实体、可见性作用域与分页游标
│   ├── store.rs       # CatalogStore trait 与 CatalogError
│   ├── service.rs     # CatalogService：所有权校验、可见性判定与 CRUD 编排
│   ├── template.rs    # CompanyTemplateV1 模板解析与 dry-run 校验
│   ├── postgres.rs    # PostgreSQL 适配器（可见性 SQL、软删除、唯一冲突）
│   └── tests.rs       # 仅放该模块单测
├── resume.rs          # resume 模块入口与公开类型聚合
├── resume/
│   ├── model.rs       # Resume/ResumeKind/ResumeAttachment/NewResume/ResumePatch 与限额
│   ├── content.rs     # ResumeContent 结构化简历与字段校验
│   ├── parse.rs       # ResumeTextExtractor、附件类型检测与解析草稿构造
│   ├── service.rs     # ResumeService：CRUD、上传解析、附件打开（≤3 份、默认提升）
│   ├── store.rs       # ResumeStore trait 与 ResumeStoreError
│   ├── storage.rs     # AttachmentStore trait（键值字节存储）；实现：LocalFs / S3 兼容
│   ├── postgres.rs    # PostgreSQL 适配器
│   └── tests.rs       # 仅放该模块单测
├── http.rs            # http 模块入口
├── http/
│   ├── router.rs      # Router 装配、AppState、顶层合并各业务路由（自带分组限流）
│   ├── auth.rs        # AuthContext 提取器与 Origin 校验
│   ├── error.rs       # ApiError、ErrorCode 枚举与统一错误体映射
│   ├── account_routes.rs # 注册/登录/注销/me/改密路由
│   ├── catalog_routes.rs # 公司/项目/流水线/岗位路由与模板导入
│   └── resume_routes.rs # 在线简历 CRUD/设默认路由与附件上传/下载
├── agent.rs           # agent 模块入口与公开类型聚合
├── agent/
│   ├── agent_loop.rs  # 回合循环、per-session 互斥、取消、阶段推进
│   ├── context.rs     # 事件日志 → 模型上下文投影
│   ├── message.rs     # 模型消息类型（system / user / assistant / tool）
│   ├── role.rs        # 招聘角色枚举与系统提示词加载
│   ├── role/prompts/  # 角色提示词（Markdown，include_str! 嵌入）
│   ├── session.rs     # Session、事件类型、SessionService、折叠与修复
│   ├── stage.rs       # 阶段枚举、Stage::role()、Stage::next()
│   ├── tool.rs        # Tool trait、ToolSpec、ToolRegistry、执行语义
│   └── tool/testing.rs# 测试用 FakeTool（cfg(test)）
├── llm.rs             # llm 模块入口与公开类型聚合
├── llm/
│   ├── client.rs      # LlmClient trait、ChatRequest/ChatResponse、LlmError
│   ├── provider.rs    # 供应商枚举、base URL 与 API Key 变量名
│   ├── openai_compat.rs # async-openai 适配与取消
│   └── mock.rs        # MockLlmClient（cfg(test)）
└── store.rs + store/
    ├── postgres.rs    # PostgreSQL 事件存储：事务、行锁、CAS、版本
    └── memory.rs      # 内存实现（cfg(test)）
```

不创建 `mod.rs`。模块文件使用 `foo.rs` + `foo/bar.rs` 的形式；测试默认放在
`foo/tests.rs`，由 `foo.rs` 末尾的 `#[cfg(test)] mod tests;` 引入。只有极小的纯函数
模块可以在文件内直接写 `mod tests`（现有 `error.rs` 属于这种情况）。

---

## 2. 依赖方向

```text
main            → app::run（组合根）
app             → config / account / catalog / resume / http / llm / store / lib 工厂
http            → account::{service, model} / catalog::{service, model, store, template} / resume::{service, model} / pipeline（类型）
account         → 无业务依赖（除 error）
catalog         → account（UserId 等类型）/ pipeline（PipelineDefinition）
resume          → account（UserId 等类型）
pipeline        → 无业务依赖（纯领域）
agent_loop      → context / session / stage / tool / llm::{LlmClient} / error
session         → stage / role / tool / llm::client / store（crate 内）
context         → message / role / session / stage / llm::client
stage           → role
role            → 无业务依赖
llm::client     → agent::{Message, ToolCall, ToolSpec}（仅数据类型）
llm::openai_compat → config::LlmConfig
store           → 不依赖 agent 领域类型
config          → error / llm::provider
```

约束：

- `agent` 与 `llm` 在数据类型上互相引用是刻意的：`llm` 只使用消息/工具的数据形状，
  `agent` 只使用 `LlmClient` trait 与请求/响应类型。**任何一方都不得引用对方的实现**
  （`AgentLoop`、`OpenAIClient`、具体 store）。
- `store` 只认不透明 JSON 与信封字段，不解释事件内容；领域校验与投影留在 `agent`。
- `config` 只在启动时读取环境变量；模块内部不得自行读 `std::env`。
- 出现新的双向依赖时，先重划模块边界或抽 trait，不要新增 `shared` / `common` /
  `util` 模块。

---

## 3. 分层与所有权

- **公开缝**：`SessionService` 是 `AgentLoop` 依赖的会话持久化接口；`SessionStore` 是
  crate 内部的事件存储接口，由 `StoreSessionService` 使用，具体实现由 `lib.rs` 的
  工厂构造。
- **唯一工厂**：外部只能通过 `postgres_session_service(pool)` 获得 PostgreSQL 会话
  服务；`store` 模块本身是 `pub(crate)`。
- **聚合导出**：对外符号通过 `lib.rs`、`agent.rs`、`llm.rs` 的 `pub use` 暴露；不要在
  子模块里随手扩大公开面。
- **具体实现只在装配点**：`main.rs` 构造 `OpenAiCompatibleClient`、`DefaultContextBuilder`、
  `ToolRegistry`、`StoreSessionService`，再注入 `AgentLoop`。
- **类型归属**：只在本模块使用的类型留在本模块；跨模块共享的类型由拥有者导出，消费方
  直接使用，不复制定义。

---

## 4. Rust 代码约定

### 4.1 格式与静态检查

- 格式以 `rustfmt.toml` 为准，不手工重排。
- Clippy 必须零警告（`-D warnings`）；命令见 §8。
- 新增 `#[allow(...)]` 必须说明原因并优先重构。
- 不新增 `unsafe`；确需使用时必须由用户明确要求，并在注释中写明安全前提。

### 4.2 命名与可见性

- 类型/枚举/trait：`UpperCamelCase`；函数/变量/模块：`snake_case`；常量：
  `SCREAMING_SNAKE_CASE`。
- 默认私有；crate 内部用 `pub(crate)`；只有跨 crate 使用才 `pub`。
- 领域层 ID 使用新类型（`SessionId`、`TurnId`），不要到处传裸 `Uuid` / `u64`；
  store 边界按现有实现转换。
- 枚举序列化统一 `#[serde(rename_all = "snake_case")]`；需要稳定字符串时同时提供
  `as_str()` 或 `FromStr`，避免两套映射漂移。

### 4.3 错误

- 每个边界一个 `thiserror` 枚举；用 `#[source]` 保留因果链。
- 在边界把驱动/SDK 错误映射为领域错误：`sqlx::Error`、`async_openai` 错误不得出现在
  公开签名或业务层。
- 日志使用 `error::error_chain` 输出完整链；不要只打印最外层消息。
- 错误文本同样受根 §10 的密钥红线约束；`ConfigError` 只包含变量名与非法值。
- 可恢复与不可恢复要能区分：例如 `SessionBusy`、`NoNextStage`、`Conflict`、
  `Corrupt` 各有独立变体，不要合并成字符串。
- HTTP 错误码是 `ErrorCode` 枚举，wire 字符串由 serde 推导；只能通过 `ApiError`
  的具名构造函数产生错误，禁止手写错误码字面量，状态码由 `ErrorCode::status`
  的穷尽 match 唯一推导。

### 4.4 异步、并发与取消

- 不在 async 上下文中做阻塞 I/O。
- `std::sync::Mutex` 只用于不跨 `.await` 的短临界区（现有 `session_locks` 查找/插入）；
  跨 `.await` 的互斥使用 `tokio::sync::Mutex`。
- 持有锁时不要执行模型调用或工具调用；`run_turn` / `finish_stage` 用 `try_lock`，
  失败立即返回 `SessionBusy`，不排队。
- 模型与工具调用必须接收并检查 `CancellationToken`；SDK 不支持取消时使用
  `tokio::select!` 竞争 `cancel.cancelled()` 与请求 future。
- 取消按 `TurnEndReason::Interrupted` 收尾，不写成 `Error`；未执行的工具调用写
  `aborted` 结果。
- 不擅自增加重试、退避、超时兜底或后台任务；确需增加时必须有终止条件与生命周期出口。

### 4.5 序列化与事件

- 对外 DTO 字段统一 `camelCase`（plan/00 S-H01）；每个含非 camelCase 字段名的 DTO
  自带 `#[serde(rename_all = "camelCase")]`——`#[serde(flatten)]` 的内容不继承外层
  rename，嵌套 DTO 不得依赖外层声明。
- 事件信封与事件类型分离：`v`、`seq`、`at` 由 store 分配，调用方不构造。
- 事件枚举使用内部标签（`#[serde(tag = "type")]`），变体字段为 `snake_case`。
- 可选字段使用 `skip_serializing_if`；空集合按现有约定省略。
- 修改事件结构或语义时，必须考虑 `EVENT_VERSION`、旧数据读取与迁移；不能只改新写入。
- `Session` 不保存第二份消息快照；`Message` 只在投影时构造。

### 4.6 配置与密钥

- 应用配置（`DATABASE_*`、`MODEL_*`、`MAX_STEPS`）只在 `config.rs` 读取；日志过滤
  `RUST_LOG` 由 tracing 订阅器读取。缺失必填项、非法值或未知供应商时启动失败。
- `LlmConfig`、`DbConfig` 刻意不实现 `Debug`；不要为了日志方便补上。
- `main.rs` 启动失败日志必须脱敏；除根 §10 的密钥红线外，完整简历内容也不得进入
  日志。
- 回合日志带 `session_id`、`turn_id`、`step`、`role` 等定位字段。
- `.env.example` 是变量模板；新增配置项时同步更新，但不写真实密钥。

### 4.7 注释与文档

- 模块用 `//!` 说明职责；公开项用 `///` 说明含义与约束。
- 注释写法（只写为什么、一个块最多 3 行）见根 `AGENTS.md` §6.2。
- 设计细节优先写进源码注释；跨文件的结构性规则才写进 `AGENTS.md`。

### 4.8 禁止事项

- 生产路径使用 `unwrap()` / `expect()` / `panic!()` / `todo!()` / `unimplemented!()`；
  必须处理 `Option` / `Result`（`unwrap_or` / `unwrap_or_else` 等返回值写法不受限）。
- 在没有用户要求时注册生产工具。

---

## 5. 会话与存储不变量

### 5.1 存储层

附件二进制存储（`AttachmentStore`，键 `[A-Za-z0-9_-]{1,128}`）由
`RESUME_STORAGE_BACKEND` 选择：`local`（默认，`RESUME_STORAGE_DIR`）或 `s3`
（S3 兼容：`S3_ENDPOINT`/`S3_REGION`/`S3_BUCKET`/`S3_ACCESS_KEY_ID`/
`S3_SECRET_ACCESS_KEY`，path-style，适配 MinIO/SeaweedFS/R2 等）。

- `create`：同一事务写入 `sessions` 行与 `seq = 0` 的 `session_meta`；UUID 冲突有限次
  重试；`append` 拒绝 `SessionMeta`。
- `append`：空批次直接返回；事务内 `SELECT ... FOR UPDATE` 锁住会话行；用期望的
  `stage` 与 `last_seq` 做 CAS，不匹配返回 `Conflict`；`seq` 从 `last_seq + 1` 连续
  分配；批量插入事件后更新 `last_seq`、`last_turn_id`、`updated_at`（必要时 `stage`）。
- 一个批次最多一个 `StageCompleted`，且必须是最后一个事件；`Stage::Review` 不允许
  `StageCompleted`。
- `load`：只读 `REPEATABLE READ` 事务；校验首事件是 `session_meta`、`last_seq` 与最大
  `seq` 一致、`last_turn_id` 与事件中的最大回合号一致、折叠阶段与 `sessions.stage`
  一致；不满足返回 `Corrupt`。
- `list` 只查 `sessions` 表，按 `updated_at DESC, id DESC` 取前 `limit` 行。
- `delete` 删除会话行，事件由外键级联删除；不存在返回 `NotFound`。

### 5.2 会话服务与循环

- `start_turn` 先修复上一进程遗留的半截回合（补写 `interrupted`），再写入新回合的
  `turn_started`；回合号取 `last_turn_id + 1`。
- 每个回合恰好一个 `turn_ended`：正常路径由 `end_turn` 写入，半截回合修复与
  `finish_stage` 在批次中直接写入。
- `finish_stage` 与 `run_turn` 共用会话锁与半截回合修复；不调用模型。
- `finish_stage` 在 `Stage::Review` 上返回 `NoNextStage`，不写事件；之后仍可继续
  `run_turn`。
- 阶段推进事件与回合事件同批写入，`stage_completed` 是最后一个；`sessions.stage` 在
  同一事务内更新。
- `run_turn` 的 `max_steps` 是正常终态，返回 `Ok(TurnResult { reason: MaxSteps })`，
  不是 `Err`；取消返回 `Interrupted`；模型/工具错误写 `Error` 并保留已落盘事件。
- 助手内容为空且没有工具调用时，写入 `Error` 终态。
- 阶段由事件折叠推导，角色由阶段推导；不要新增可独立修改的角色字段。

---

## 6. 测试约定

- 单元测试位置：与源码同目录的 `tests.rs`（如 `http/auth/tests.rs`），`#[tokio::test]`；
  测试替身只在 `#[cfg(test)]` 下编译；跨模块共用的替身与请求装配集中在
  `src/testing.rs`，模块内专用替身就近放（如 `agent/tool/testing.rs`）。
- 首选现有替身：`crate::testing`（账户内存端口、FakeClock/FakeHasher、Router/oneshot）、
  `MemorySessionStore`、`MockLlmClient`、`FakeTool`；不要为测试在生产模块里加公开入口。
- 集成测试位置：`crates/careersim/tests/`，使用真实 PostgreSQL；未设置
  `DATABASE_URL` 时直接返回，不算验证通过。
- 覆盖重点：事件序列、`seq` 连续性、CAS 冲突、半截回合修复、取消路径、空响应、
  `max_steps`、阶段推进、`NoNextStage`、错误 source 链。
- HTTP 层覆盖重点：Origin 校验矩阵、Cookie 安全属性、AuthContext 提取、路由级
  状态码与错误体映射；路由测试走完整 Router + oneshot，不依赖 PostgreSQL。
- 单测不得访问真实网络或真实数据库；时间、随机、并发依赖必须可控。

---

## 7. 迁移与数据库

- 迁移文件位于 `crates/careersim/migrations/`，启动时由 `sqlx::migrate!` 执行。
- 只增不改：已提交的迁移不得编辑；结构变化新增递增编号文件。
- 使用 sqlx 运行时查询 API（`sqlx::query` / `query_scalar` / `query_as`）；构建阶段不
  要求可连接数据库，不使用编译期校验宏。
- 表结构或事件语义变化时，同步检查 `load` 的完整性校验与旧数据读取。

---

## 8. 常用命令

```bash
# 仓库根执行
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
DATABASE_URL=postgres://careersim:careersim@127.0.0.1:5432/careersim_test \
  cargo test --locked --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps
cargo build --locked --release
cargo deny check advisories bans licenses sources

# 收窄到相关模块
cargo test agent::agent_loop
cargo test store::memory
```

- 需要 PostgreSQL 集成测试时，先启动 `compose.yaml` 并设置 `DATABASE_URL`。

---

## 9. 变更落点

| 想做什么 | 去哪里 |
| --- | --- |
| 新增账户能力或改会话语义 | `account/service.rs` + `account/store.rs` + 测试 |
| 新增目录能力（公司/项目/流水线/岗位/模板） | `catalog/service.rs` + `catalog/store.rs` + `http/catalog_routes.rs` + 测试 |
| 新增简历能力（在线内容/附件/解析） | `resume/service.rs` + `resume/store.rs` + `http/resume_routes.rs` + 测试 |
| 新增 HTTP 路由 | `http/` 下新建路由文件 + `http/router.rs` 追加挂载行 |
| 新增事件类型 | `agent/session.rs` + `agent/context.rs` 投影 + 测试；必要时迁移 |
| 调整阶段顺序或新增阶段 | `agent/stage.rs` + `agent/role.rs` + 提示词 + 测试 |
| 新增角色 | `agent/role.rs` + `role/prompts/` + `Stage::role()` + 测试 |
| 新增工具 | `agent/tool.rs` 的 `Tool` 实现与注册；生产注册表当前为空 |
| 改模型映射 | `llm/openai_compat.rs` |
| 新增供应商 | `llm/provider.rs` + `config.rs` + `.env.example` |
| 改存储或 SQL | `store/postgres.rs` + 新迁移文件 |
| 改配置项 | `config.rs` + `.env.example` + `app.rs` 装配 |
| 改会话服务语义 | `agent/session.rs`（先确认存储不变量） |
