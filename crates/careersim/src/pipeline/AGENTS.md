# AGENTS.md — pipeline 模块

> 适用于 `src/pipeline.rs` 与 `src/pipeline/`。

- pipeline 是纯领域模块，不读取数据库、环境变量或网络，不持有外部服务。
- `StagePosition` 是阶段顺序定位。阶段结构使用 `position`、`kind`、`title` 和规则数据。没有阶段键字段，`title` 只用于展示。
- `PipelineDefinition::parse` 和 `validate.rs` 统一检查位置连续性、阶段字段、rubric、Offer 唯一性和末位约束。catalog、agent 和 application 不重复实现这些规则。
- `plan_transition` 只根据已验证的 pipeline 和当前位置计算结果，不写入 Application 或 Session。
- `rubric.rs` 只负责评分值对象、维度权重和阶段评估。评分范围、权重总和和文本长度限制集中在这里。
- `build_offer_terms` 只负责 Offer 条款校验和构造。模型输出或 HTTP 输入先转换为对应领域输入。
- 测试覆盖空流程、位置断裂、Offer 位置、无效 rubric、评分边界和转移边界。
