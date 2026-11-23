# AGENTS.md — store 模块

> 适用于 `src/store.rs` 与 `src/store/`。

- store 只负责 Session 行、事件信封和存储状态，不解释 `SessionEventKind` 的业务语义。
- `create` 在同一事务中写入 session 行和 `seq = 0` 的元事件。`append` 在事务内锁行、执行 `last_seq` CAS、连续分配 seq、插入事件并更新缓存字段。
- 空批次直接返回。一个批次最多一个 `StageCompleted`，且必须是最后一个事件。`SessionMeta` 不能作为追加事件。
- `load` 使用一致性事务校验首事件、最大 seq、`last_turn_id` 和事件版本。异常返回 `Corrupt`，不能静默修复。
- `list` 只读取会话摘要，`delete` 依靠外键级联删除事件。PostgreSQL 和内存实现必须保持同一端口语义。
- SQL 使用参数化查询，驱动错误映射为 `StoreError` 并保留 source。事件版本变化必须兼容旧数据读取或配套迁移。
- 存储层不新增 Application 阶段字段。当前流程位置由 Application 持久化，Session 只保留历史阶段事件。
- 测试覆盖 CAS 冲突、seq 连续性、事务失败、损坏日志、删除级联和内存实现语义一致性。
