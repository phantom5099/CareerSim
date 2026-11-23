# AGENTS.md — http 模块

> 适用于 `src/http.rs` 与 `src/http/`。

- 路由文件只处理 HTTP 参数提取、鉴权、调用 service 和响应转换。不得在 handler 中编写 PostgreSQL 查询或复制领域校验。
- 请求体、查询参数和路径参数使用私有 `*Request` 类型，响应使用私有 `*Response` 类型。统一在 HTTP 边界完成 `camelCase`、时间、ID 和枚举序列化。
- handler 只从 `AuthContext` 获取当前用户身份，客户端提交的 `userId`、`ownerId` 不参与资源归属判断。
- `router.rs` 负责 AppState、路由合并、请求 ID、trace 和限流层的装配。路由新增后必须在这里挂载。
- `auth.rs` 负责 Cookie 会话和 Origin 校验。`error.rs` 负责 `ApiError`、`ErrorCode`、状态码和统一错误体，禁止路由手写错误码字符串。
- HTTP 不得暴露 `sqlx`、store Record 或适配器错误。service 错误必须在统一错误边界映射。
- 上传附件和 JSON 请求都必须限制输入大小并保留失败观测。响应不得包含密码、令牌、连接串、完整简历正文或模型提示词。
- 路由测试使用完整 Router 和 `oneshot`，覆盖认证、Origin、Cookie 属性、状态码、错误体和边界输入。
