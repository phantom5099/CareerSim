# AGENTS.md — frontend（Vue 前端）

> `frontend/` 是独立的 Vite 工程；仓库级规则、信息优先级与测试禁止类型见根
> `AGENTS.md`，本文件只写前端约定。
>
> 事实源：`package.json`、`bun.lock`、`tsconfig.json`、`vite.config.ts`、
> `src/styles/main.css`。

---

## 1. 技术栈

| 项 | 选型 | 事实源 |
| --- | --- | --- |
| 框架 | Vue 3 | `package.json` |
| 构建 | Vite | `package.json`、`vite.config.ts` |
| 语言 | TypeScript（严格模式） | `tsconfig.json` |
| 状态 | Pinia | `package.json`、`src/stores/` |
| 数据请求 | `@tanstack/vue-query` | `package.json` |
| 路由 | Vue Router（history 模式） | `src/router/index.ts` |
| 样式 | Tailwind CSS（`@theme` 变量） | `src/styles/main.css` |
| 包管理器 | Bun（`bun.lock`） | lockfile |

---

## 2. 目录职责

| 路径 | 内容 |
| --- | --- |
| `src/api/http.ts` | 请求客户端核心：`credentials: 'include'`、统一错误映射、非认证接口 401 时清空会话并跳登录 |
| `src/api/endpoints/` | 端点契约接口 + 当前绑定（`auth.ts` 为 HTTP Cookie 实现，其余为 mock） |
| `src/api/types.ts` | 接口 DTO：登录、认证会话、简历、行业等 |
| `src/api/error.ts` | 统一 `ApiError`（`status` + `code`） |
| `src/domain/types.ts` | 领域模型与阶段常量（`Application`、`Company`、`Job`、`StageKey`） |
| `src/mock/` | mock 实现与种子数据 |
| `src/pages/` | 路由页面 |
| `src/components/` | 可复用组件 |
| `src/layouts/` | 布局组件 |
| `src/stores/` | Pinia store 与初始化逻辑 |
| `src/router/` | 路由表、登录守卫、页面标题 |
| `src/styles/` | Tailwind 主题变量与全局样式 |
| `src/config/` | 演示配置 |
| `src/content/` | 品牌文案 |

---

## 3. 数据访问与契约（核心规则）

- **页面与 store 只通过 `@/api/endpoints/<module>` 暴露的绑定访问数据**，不得直接
  import `@/mock/*`。
- **契约先行**：新增或修改数据能力时，先在 `src/api/endpoints/<module>.ts` 定义接口，
  再在 `src/mock/<module>.ts` 提供同签名实现，最后在端点模块里完成绑定。
- 后端就绪后，只把绑定从 mock 换成 HTTP 实现，页面与 store 的调用方式不变。
- 不在页面里写「有后端 / 无后端」的条件分支，也不在页面里直接拼请求。
- 错误统一使用 `ApiError`；不要各页各写一套错误结构。
- mock 数据只存在内存中，刷新页面即重置；不要把它当成持久化数据源。
- 契约变更必须同步 mock 实现与调用方；不允许只改类型不改实现。已接入 HTTP 的模块
  （`auth.ts`）不再保留 mock，契约变更直接改 HTTP 实现与调用方。

当前端点模块：

| 模块 | 绑定 |
| --- | --- |
| `auth.ts` | HTTP 实现（`apiFetch` + Cookie 会话；无 mock） |
| `applications.ts` | `mockApplicationsApi` |
| `companies.ts` | `mockCompaniesApi` |
| `interviews.ts` | `mockInterviewsApi` |
| `offers.ts` | `mockOffersApi` |
| `resume.ts` | `mockResumeApi` |

---

## 4. 类型归属

- 领域模型与阶段常量放 `src/domain/types.ts`；接口请求/响应 DTO 放
  `src/api/types.ts`；端点方法签名放各自的 `api/endpoints/*.ts`。
- 跨文件引用类型一律使用 `import type`。
- 不为同一概念定义两份形状不同的类型；需要投影时在消费方显式转换并说明来源。
- 阶段顺序与标签以 `STAGE_ORDER`、`STAGE_LABELS` 为准，不在页面里重复枚举。

---

## 5. Vue 与 TypeScript 约定

- 组件使用 `<script setup lang="ts">`；组件文件使用 PascalCase。
- 保持类型检查 0 错误（命令见 §7）；不新增 `any`、`@ts-ignore`、
  `@ts-expect-error` 来绕过类型问题。
- 使用 `@/` 别名指向 `src/`，不写脆弱的深层相对路径。
- 页面只负责展示与交互；数据加载、初始化与缓存逻辑放 store 或端点层。
- 路由守卫只做认证与必要的数据初始化；业务逻辑不要写进守卫。
- 组件 props / emits 需要显式类型。

### Pinia

- 使用 setup store（`defineStore('name', () => { ... })`）。
- 异步初始化用单飞模式（现有 `initPromise`）避免重复请求；初始化失败时不要留下
  永久成功的假状态。
- store 不直接访问 `localStorage` / `sessionStorage`；当前也没有本地持久化约定，
  新增前先与用户确认键命名与隔离策略。

---

## 6. 样式

- 颜色、字体、背景等主题令牌在 `src/styles/main.css` 的 `@theme` 中定义，例如
  `--color-paper`、`--color-ink`、`--color-vermilion`、`--font-display`。
- 优先复用主题令牌与 Tailwind 工具类，不在组件里重复硬编码同一颜色值。
- 新增视觉风格时先扩展主题变量，再在组件中使用；避免样式分叉。

---

## 7. 命令

```bash
cd frontend
bun install       # 首次安装
bun run typecheck # vue-tsc --noEmit
bun run build     # 类型检查 + 生产构建
bun run dev       # http://localhost:5175
bun run preview   # 预览构建产物
```

- 开发服务器固定端口 5175（`strictPort`），`/api` 代理到 `http://127.0.0.1:3000`。

---

## 8. 当前缺口

- 没有 ESLint 配置与 lint 脚本。
- 没有测试框架与测试文件。
- 没有前端 CI job。
- 没有真实后端联调：所有端点当前绑定 mock。

新增 lint、测试或 HTTP 客户端前先与用户确认。

---

## 9. 验证要求

- UI 改动必须启动 `bun run dev`，在浏览器走一遍主路径；无法验证时明确说明。
- 至少覆盖：登录、公司列表与详情、投递、面试记录、Offer 列表，以及空数据与错误
  分支。
- 修改 mock 行为时，同时验证页面刷新后的初始状态与操作后的状态一致。
- 不把「类型检查通过」当成「功能正确」；类型只证明接口形状，不证明交互行为。
