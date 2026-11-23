# AGENTS.md — account 模块

> 适用于 `src/account.rs` 与 `src/account/`。

- `account.rs` 只聚合账户领域类型、service 输入、store Record 和 PostgreSQL 服务工厂。
- `model.rs` 维护 `UserId`、`Username`、`Password`、`PasswordHash`、`User` 和账户领域错误。值对象在构造或显式校验时保证边界，密码和哈希的调试输出必须脱敏。
- `service.rs` 的 `RegisterInput`、`LoginInput` 和 `ChangePasswordInput`，以及 `model.rs` 中由 `update_profile` 接收的 `UpdateProfileInput`，共同构成 service 输入契约。service 负责校验、密码哈希、会话轮换和错误映射，再向 store 传递 `CreateUserRecord` 或 `CreateLoginSessionRecord`。
- `session.rs` 只维护登录会话标识、令牌哈希、过期判断和 `CreateLoginSessionRecord`。原始令牌只在登录响应路径短暂存在，不进入日志、事件或数据库。
- `store.rs` 定义 `UserStore` 与 `SessionStore` 端口及错误类型。端口不依赖 HTTP DTO。
- `postgres.rs` 只实现账户端口和 SQL 错误映射。账户状态变化涉及多条写入时保持事务边界。
- 账户模块不依赖 `catalog`、`resume`、`application` 或 `agent` 的业务实现。
- 测试使用 `FakeClock`、`FakeHasher` 和内存 store，覆盖密码错误、会话过期、注销、轮换和并发失败路径。
