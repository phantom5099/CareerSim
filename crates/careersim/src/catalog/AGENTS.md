# AGENTS.md — catalog 模块

> 适用于 `src/catalog.rs` 与 `src/catalog/`。

- catalog 管理公司、招聘项目、流程模板、岗位及模板导入。`service.rs` 的 `Create*Input`、`Update*Input` 完成输入校验后转换为 `*Record`，store 和 PostgreSQL 适配器不接收 HTTP 类型。
- `pipeline_stages` 的持久化阶段使用 `position`、`kind`、`title`、rubric 和轮次约束。阶段没有持久化阶段键，`title` 只用于展示，`position + kind` 是阶段权威身份。
- 模板 JSON 中的 `key` 只用于一次导入内的公司、项目、流程和岗位相互引用。它不是数据库阶段身份，也不得写入 `pipeline_stages`。
- `PipelineDefinition` 在进入 store 前必须通过 `pipeline` 的纯校验。位置从零开始连续，Offer 只能有一个且位于末位，阶段类型和 rubric 约束由领域校验统一处理。
- 公司、项目和流程名称的归一化键只服务于对应实体的数据库唯一性。岗位名称不建立去重语义，不能将岗位标题归一化后当作岗位身份。
- 列表和详情查询必须带可见性、归属和软删除条件。删除使用现有软删除语义，写入与模板导入遵守事务边界。
- `template.rs` 负责模板解析、集合内引用和 dry-run 校验。它不直接访问数据库，导入由 service/store 组合完成。
- PostgreSQL 适配器统一把唯一冲突、外键冲突、可见性失败和软删除状态映射为 `CatalogError`。
- 测试覆盖模板引用错误、阶段位置校验、权重总和、重复岗位标题允许、可见性和软删除过滤。
