# AGENTS.md — application 模块

> 适用于 `src/application.rs` 与 `src/application/`。

- `Application` 表示岗位投递记录，并与一条完整 `Session` 建立一对一关系。模块只负责投递持久化和查询，不负责回合或阶段推进。
- `Application.stage_position` 是当前流程游标的持久化事实来源。读取展示时通过投递关联的 `job_id` 找到 `pipeline_id`，再按 `pipeline_stages.position` 读取权威阶段的 `position`、`kind` 和展示名。
- 不增加独立的阶段键、阶段缓存或基于数组下标重新推算当前阶段的字段。阶段展示名不是身份。`position + kind` 是流程阶段的权威数据。
- `CreateApplicationInput` 属于 service，`CreateApplicationRecord` 属于 store。service 只负责输入转换，store 负责会话存在性、岗位流程初始位置和持久化约束。
- `create`、`list_views`、`get_view` 和 `update_progress` 的 SQL 必须以当前用户归属为条件。更新游标时校验该位置属于岗位当前关联的 pipeline，并在同一事务内完成校验和更新。
- 查询视图可以批量缓存同一 `pipeline_id` 的阶段列表，但缓存只存在于单次查询，不改变数据库权威关系。
- `ApplicationView` 供 HTTP 转换使用，不向 HTTP 泄漏 store 适配器或 SQL 行类型。
- `stage_position` 的更新不修改 Session 事件历史。会话事件只记录已经发生的交互，流程位置由 Application 读取。
- Offer 列表只查询当前认证用户且 `Application.status = offer` 的记录。Offer 条款从关联 Session 的 `offer_pending` 事件读取，Offer 回复状态使用 Application 的 `offer_response_status` 持久化。
- Offer 回复只能更新属于当前用户且流程状态为 `offer` 的 Application，不能把 `accepted` 或 `declined` 写入流程状态字段。
- 测试覆盖一对一约束、初始位置、无效位置、岗位归属、会话不存在、更新冲突和视图阶段读取。
