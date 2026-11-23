# AGENTS.md — crate 级入口与装配

> 本文件只保留 Rust crate 的跨模块规则。源码模块的通用规则在 `src/AGENTS.md`，业务边界在各模块目录的 `AGENTS.md`。

## 1. 目录结构

```text
src/
├── *.rs                 # 模块入口或 crate 入口
├── account/             # 账户、登录会话与密码
├── application/         # 岗位投递与 Application/Session 关联
├── catalog/             # 公司、项目、流程模板与岗位
├── resume/              # 简历、附件与解析
├── http/                # HTTP 路由、鉴权和错误映射
├── agent/               # Session、事件、回合、上下文和工具
├── pipeline/            # 纯流程领域模型与校验
├── llm/                 # 模型端口和供应商适配
└── store/               # Session 事件存储
```

模块使用 `foo.rs` 加 `foo/bar.rs`，不使用 `mod.rs`。`src/AGENTS.md` 统一覆盖入口文件和子目录，各子目录规则只写模块差异。

## 2. 跨模块依赖

- `main → app`。`app` 负责配置、连接池、迁移、适配器、Router 和服务装配。
- `http → account/catalog/resume/application service`。HTTP 不直接依赖 PostgreSQL 适配器。
- `account` 不依赖其他业务模块。`catalog` 可使用账户 ID 和 pipeline 领域类型。`resume` 使用账户 ID。`application` 使用岗位、简历和 Session ID。
- `agent` 只依赖上下文、阶段、角色、工具和 `llm` 端口。`llm` 只依赖消息和工具数据形状，不引用 AgentLoop 或具体存储实现。
- `pipeline` 无业务依赖。`store` 不依赖 agent 领域类型，只处理不透明事件 JSON 和存储状态。
- 发现新的双向依赖时先调整边界，不新增无职责的共享模块。

## 3. 公开缝与装配

- `postgres_account_service`、`postgres_catalog_service`、`postgres_resume_service`、`postgres_application_service` 和 `postgres_session_service` 是适配器进入业务层的工厂。
- 具体 PostgreSQL、文件系统、S3 和供应商实现保持 crate 内可见，由 `app.rs` 或模块工厂装配。
- `lib.rs` 只导出稳定模块入口和服务工厂。`main.rs` 只负责启动、错误脱敏和退出码。
- 环境变量只在 `config.rs` 读取。观测只在 `telemetry.rs` 集中初始化。

## 4. 事件与流程位置

- Session 事件是交互历史的事实来源。Application 的 `stage_position` 是当前投递流程游标的持久化事实来源。
- 流程展示通过岗位关联的 `pipeline_id` 读取 `pipeline_stages.position`、`kind` 和展示名。不要新增阶段键或 Session 当前阶段缓存。
- 事件信封由存储层分配版本、序号和时间。阶段历史和上下文投影由 agent 处理。

## 5. 测试与变更落点

- 跨模块测试装配在 `src/testing.rs`。模块单测放各模块 `tests.rs`，集成测试放 crate 的 `tests/`。
- 改账户能力看 `account/`，改投递持久化看 `application/`，改流程模板和岗位看 `catalog/`，改简历看 `resume/`，改 HTTP 看 `http/`，改回合和事件看 `agent/`、`store/`，改评分和流程校验看 `pipeline/`，改模型供应商看 `llm/`。
- 迁移只增不改，表结构或事件语义变化必须同步检查旧数据读取和完整性校验。
- 验证命令以仓库根 `AGENTS.md` 和 CI workflow 为准。本文件不维护第二份命令清单。

## 6. 规则维护

- 本文件不维护函数清单、字段字典或重复的测试矩阵。具体实现约束放在对应模块 `AGENTS.md` 或源码注释中。
- 规则发生变化时只修改受影响层级的规则文件。
