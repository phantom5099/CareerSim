# AGENTS.md — resume 模块

> 适用于 `src/resume.rs` 与 `src/resume/`。

- resume 管理在线简历、附件、文本解析和默认简历。`ResumeService` 是唯一用例编排入口。
- `CreateResumeInput`、`UpdateResumeInput`、`UploadResumeInput` 和 `AttachResumeInput` 属于 service。通过校验后转换为 `CreateResumeRecord`、`UpdateResumeRecord` 和 `CreateResumeAttachmentRecord`，store 不接收 HTTP 请求。
- `model.rs` 只放简历实体、附件实体、标识、解析状态和领域错误。内容字段校验在 `content.rs`，附件文本提取在 `parse.rs`。
- 附件对象键只交给 `AttachmentStore`，数据库记录与二进制存储的创建、删除和失败补偿必须保持一致。对象键、简历正文和解析正文不得进入日志。
- 默认简历变更、最多三份限制和附件关联必须使用现有事务或等价的原子端口操作。解析失败保留可读取的状态和错误码。
- `storage.rs` 隔离本地文件系统和 S3 兼容实现，service 不拼接文件路径或直接读取环境变量。
- 测试使用内存附件存储和静态文本提取器，覆盖大小限制、附件类型、解析失败、默认简历和删除清理。
