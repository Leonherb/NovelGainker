# NovelGainker Agent Migration AM1 Story Packet

> 状态：NG-A101～NG-A104 与完整质量门已通过；GA1 已于 2026-08-28 获产品批准，AM1 已关闭
>
> 日期：2026-08-28
>
> BMAD 数据包：`execution-packet`
>
> Program：Agent Migration AM1——统一会话与只读 Tool Loop
>
> 范围：NG-A101～NG-A104、GA1 技术提交

## 1. BMAD Routing Brief

- Packet type：`brownfield-resume-packet` → `execution-packet`
- Project type：Windows-first Tauri desktop app
- Project level：4
- Runtime context：Codex / local repository
- Current phase：closed
- Recommended next move：GA1 已获产品批准；后续执行与 GA2 状态见 AM2 Story Packet
- 不做：Canon 写工具、自动 Apply、撤销 Blob、旧 G3～G5 后端移除、AM2、Sprint 6

理由：GA0 契约和产品 Gate 已关闭；AM1 是最小可证明的生产运行时切片，先把聊天变成可读项目但固定零写入的 Agent，再进入安全写入迁移。

## 2. AM1 目标

AM1 把中栏普通聊天与独立 Router 产品入口替换为一个持续的 `ProjectAgentSession` Timeline。作者用自然语言提出人物、设定、剧情或叙事问题时，App 选择内置 Skill，串行执行受限只读工具，并把 Provider、Skill、Turn、工具、来源、取消和错误作为可见事件展示。

AM1 完成后必须具备：

1. 每个打开项目一个可恢复的 Agent Session，同一时刻最多一个活动 Turn。
2. 一个统一输入框；不存在绕过 Base Policy 的“普通聊天”产品路径。
3. `project.list_markdown`、`project.read_markdown`、`project.search_markdown`、`project.validate_markdown` 四个只读工具。
4. 人物、设定、剧情、叙事四类意图均能选择对应 Skill 并进行只读检查。
5. Tool Request 由 App 注入 Project/Session/Turn 绑定；模型不能授予权限或指定绝对路径。
6. 默认只持久化有界摘要、消息 Hash 索引和终态 Turn/Skill/Tool 元数据，不保存完整聊天正文或 Provider 原始响应。
7. DeepSeek 与 OpenAI 使用同一 Agent Action 契约、同一预算、同一安全停止条件；禁止自动跨 Provider Fallback。
8. GA1 证据证明整个 AM1 Agent 路径的项目写入数为 0。

## 3. 授权边界

### 3.1 已授权

- 创建、审查并按推荐值冻结本 Packet。
- 新增 `novelgainker-agent` 到生产 Rust 依赖，但只激活内置 Pack 的 AM1 只读运行时视图。
- 新增 Agent Session/Turn/Skill/Tool IPC、最小 Capability 与前端 Timeline。
- App State 升级以保存 AM0 已批准的有界 Session 摘要、消息索引和终态 Turn 元数据。
- 复用既有 ProjectHandle、路径校验、只读 Workspace/Search/Validator、Provider/Vault 与取消实现。
- 连续实施 NG-A101～NG-A104、Story 级测试、完整质量门及 GA1 技术提交。

### 3.2 未授权

- `project.create_markdown`、`project.patch_markdown`、文件删除/移动/重命名、任意文件系统或 Shell。
- Agent 发起任何 Canon 写入、写入确认、Diff Apply、Undo Blob 或写事务。
- 完整聊天正文持久化、API Key、隐藏推理、Provider 原始响应或绝对路径持久化。
- 运行时读取外部 Plugin Cache、用户 `SKILL.md`、远程 Skill、Python Sidecar 或外部 Agent Host。
- 删除旧 Workflow/Router/Provider 后端；AM1 只替换中栏产品入口，兼容后端保留到 NG-A206。
- GA1 产品批准、AM2、Sprint 6 或公开发布工作。

## 4. 已确认决策

### AM1-Q001——统一入口

建议：打开项目后中栏只显示 Agent Timeline；未打开项目时只提供打开/创建项目提示。旧普通聊天、Router 选择器和表单工作流不再作为中栏入口，但后端暂不删除。

状态：**已接受**（2026-08-28，包含在完整 AM1 授权中）。

### AM1-Q002——Skill 路由

建议：先用确定性本地词汇路由形成一个或多个候选 Skill，再要求 Provider 的每个 Action 使用同一候选集合；项目内容作为不可信数据不能改变选择或权限。无项目工作意图时允许零 Skill 直接回答。

状态：**已接受**。

### AM1-Q003——Provider Tool 模式

建议：AM1 生产路径统一使用严格 JSON `AgentActionPayloadV1`；AM0 保留原生 Tool Calling 等价性契约，后续可在不改变内部 Action 的前提下启用。每个 Turn 最多一次同 Provider 修复，禁止跨 Provider Fallback。

状态：**已接受**。

### AM1-Q004——只读工具预算

建议：继承 AM0 上限；单 Turn 最多 8 Agent Steps、16 Tool Calls、单个 Action 最多请求 4 个工具、24 个唯一项目文件、单请求 96 KiB。具体 Tool 输出进一步采用更小的项目索引、单文件、搜索结果与 UTF-8 截断上限。

状态：**已接受**。

### AM1-Q005——会话持久化

建议：App State 只保存一个项目 Session 的有界摘要、200 条消息 Hash 索引、100 个终态 Turn 元数据、Skill/Tool ID 与计数；完整聊天默认关闭且 AM1 不提供 Opt-in UI。

状态：**已接受**。

### AM1-Q006——Provider 切换与恢复

建议：只允许无活动 Turn 时显式切换；新 Provider 从 Canon、持久化摘要和可见元数据重建请求，不迁移隐藏 Provider 状态，不自动回退。

状态：**已接受**。

### AM1-Q007——连续实施授权

建议：仍按 NG-A101 → NG-A102 → NG-A103 → NG-A104 顺序和依赖验证，但本 Packet Review Gate 通过后无需逐 Story 等待产品批准；完成后只提交 GA1 技术审查并停止。

状态：**已接受**（产品负责人明确要求统一执行到 AM1 结束）。

## 5. Stories

### NG-A101——统一 Agent Timeline 与 Session 控制面

验收：

- [x] 中栏只有一个 Agent Timeline 与一个自然语言输入入口。
- [x] 项目打开后建立或恢复一个项目隔离 Session；项目关闭时取消活动 Turn。
- [x] Timeline 显示 Provider、Skill、Turn 状态、Tool、来源、取消和稳定错误。
- [x] Provider 未配置、未连接或离线时输入立即拒绝，不进入后台队列。
- [x] 旧 Conversation/Router/Workflow 后端保留但不再由统一入口调用。

证据：`target/qa/ng-a101/ng-a101-evidence.json`。

### NG-A102——受限只读项目工具

验收：

- [x] 只注册 list/read/search/validate 四个只读工具，写工具固定 Policy Blocked。
- [x] Tool Request 只接受不透明 Project/Session/Turn 绑定与项目相对路径。
- [x] 复用 Rust 路径、符号链接、UTF-8、文件类型、大小、索引、搜索与 Validator 限制。
- [x] Tool 输出有数量/字节上限、来源 Hash、截断原因和敏感字段拒绝。
- [x] 对合成项目执行全矩阵后 Canon 树 Hash 不变。

证据：`target/qa/ng-a102/ng-a102-evidence.json`。

### NG-A103——会话内 Skill Router 与 Tool Loop

验收：

- [x] 人物、设定、剧情、叙事意图可选择对应一个或多个内置 Skill。
- [x] Provider Action 经严格 Schema、同 Provider 单次修复、Action Chain 和 Turn Budget 校验。
- [x] Tool Request 由 App 绑定并串行执行；结果按不可信 Tool Data 回传。
- [x] 未注册 Skill/Tool、写工具、Prompt Injection、重复 Call、越界路径、循环超限和异常均安全停止。
- [x] 每个 Tool 调用及来源在 Timeline 可见，项目写入数为 0。

证据：`target/qa/ng-a103/ng-a103-evidence.json`。

### NG-A104——有界恢复与双 Provider 契约

验收：

- [x] App State 事务迁移保存有界摘要、消息索引与终态 Turn/Skill/Tool 元数据。
- [x] 数据库不含完整聊天、Key、隐藏推理、原始 Provider 响应、绝对路径或项目正文副本。
- [x] 重启后能恢复 Session 摘要与最近 Turn；活动 Turn 不重放，转换为可见失败/取消终态。
- [x] 显式 Provider 切换只更新冻结快照；DeepSeek/OpenAI 均从 Canon 和持久化可见状态重建。
- [x] 清空 Session 只删除 App State Agent 数据，Canon 写入数为 0。

证据：`target/qa/ng-a104/ng-a104-evidence.json`。

## 6. Exit Gate GA1

- [x] 人物、设定、剧情、叙事四类意图均完成 Skill 选择与只读项目检查。
- [x] 所有 Tool 调用可见、可取消、可追踪，端到端项目写入数固定为 0。
- [x] 重启与显式切换 Provider 后可从 Canon 与有界状态继续，无隐藏状态迁移。
- [x] 双 Provider Mock、无 Key/离线、Prompt Injection、路径逃逸、预算、取消、异常与敏感字段矩阵通过。
- [x] App State Migration、Command Manifest、Permissions、Capability、TypeScript Client 与 UI 同步。
- [x] `pnpm quality`、Debug/Release Build、桌面 Smoke 与 G1～G5 回归通过。
- [x] GA1 Manifest 的 `ga1Approved` 保持 `false`，等待独立产品批准。

### GA1 技术结果

- 运行时 Contract Set Hash：`a46ace17642e407f0f2b10c0579ddc7044352e2cea59fb2e7915d0813aeaa425`。
- App State 已迁移到 v9；Tauri 登记 56 条命令，其中 5 条为 Agent IPC。
- 运行时注册 9 项 Skill、4 个只读工具、0 个写工具；完整聊天持久化固定关闭。
- 29 个前端测试文件/155 项测试、Workspace Rust 测试、严格 Clippy、G1～G5 回归、Debug/Release 构建与桌面 Smoke 均通过。
- 完整 `pnpm quality` 以退出码 0 通过；最终输出为 `[quality] PASS NovelGainker local quality gate`。
- 真实 Provider Live Smoke 未执行；当前授权不包含读取真实凭据或发起真实网络请求。
- GA1 Manifest：`target/qa/ga1/ga1-manifest.json`；Review Report：`target/qa/ga1/ga1-review-report.md`；`ga1Approved=false`。

## 7. Review Gate 与授权记录

AM1 Story Packet Review Gate：**通过**（2026-08-28）。

批准记录：产品负责人明确回复“批准 GA0，授权后续 AM1 的所有，并且统一执行到 AM1 结束”。

授权执行规则：

1. NG-A101～NG-A104 仍按依赖顺序实施并分别形成可验证证据。
2. Story 技术通过后自动进入下一 Story，无需再次等待产品授权。
3. 任一安全或数据边界阻断必须先修复，不得通过弱化 Gate 继续。
4. NG-A104 完成后运行完整质量门并提交 GA1；此时停止，不进入 AM2。

执行结论：NG-A101～NG-A104 已按顺序完成，完整质量门与 GA1 技术提交通过。产品负责人于 2026-08-28 明确批准 GA1，并统一授权 AM2 后续相关内容；AM1 因此关闭。该后续授权不批准 GA2，也不授权 Sprint 6。
