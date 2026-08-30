# NovelGainker Agent Migration AM2 Story Packet

> 状态：已由 2026-08-30 RB0/AM-R1 路线取代；仅作为 V1 写入 Kernel 与回归迁移证据，等待对应 Gate 按清单移除
>
> 日期：2026-08-29
>
> BMAD 数据包：`execution-packet`
>
> Program：Agent Migration AM2——安全写入与核心 Skill 迁移
>
> 范围：NG-A201～NG-A206、GA2 技术提交

## 1. BMAD Routing Brief

- Packet type：`brownfield-resume-packet` → `execution-packet`
- Project type：Windows-first Tauri desktop app
- Project level：4
- Runtime context：Codex / local repository
- Confidence：high
- Current phase：review
- Recommended next move：独立审查 GA2 Manifest 与 Review Report；未获产品批准前停止
- Route-out：现有 Agent Runtime、AM0 Governance 契约与 G3～G5 Canon/Recovery 实现
- 不做：GA2 产品批准、Sprint 6、删除/移动/重命名、任意文件系统/Shell、图片生成、EPUB、第三家 Provider、完整聊天持久化

理由：GA1 已获产品批准；统一只读 Agent、9 项 Skill、严格 Action Loop、App State v9 和零写入证据均已形成。当前最小真实增量是把 AM0 已冻结的写入治理契约接入同一 Timeline，而不是恢复独立工作流入口或提前进入 Sprint 6。

## 2. AM2 目标

AM2 让作者在同一项目 Agent 会话中通过自然语言持续创建和更新受支持 Markdown。模型只能提出严格 Tool Request；App 独占路径、变更形态、风险、Diff、确认、事务、Undo 与最终写入决定。

AM2 完成后必须具备：

1. 运行时注册现有四个只读工具和 `project.create_markdown`、`project.patch_markdown` 两个受治理写工具。
2. create/patch 在 App 内生成严格验证后的目标内容、来源 Hash、逐文件 Diff、风险和事务计划。
3. 每 Turn 最多一个、最多八文件的原子写事务；全部前置校验通过前项目零写入。
4. 支持 `plan_only`、`review_every_write`、`auto_apply_safe`；混合风险 Turn 采用最高风险且不允许部分提交。
5. `always-confirm` 永远等待绑定当前 Project/Session/Turn/Plan/Source/Policy Hash 的一次性用户确认。
6. 提交前生成 Windows 用户上下文保护的有界 Undo Blob；撤销遇到后续人工修改必须拒绝覆盖。
7. 项目、人物、设定、素材、剧情、章节、连续性、章节状态与 Story Ledger 均从统一 Timeline 进入对应 Skill。
8. DeepSeek/OpenAI 共用同一写入路径；旧普通聊天、独立 Router 和表单不再构成可达产品入口。

## 3. 授权边界

### 3.1 已授权

- 创建、审查并按本节推荐值冻结本 Packet。
- 连续实施 NG-A201～NG-A206、Story 级证据、完整质量门与 GA2 技术提交。
- 新增 AM2 Runtime Schema、写入预览/确认/拒绝/策略/撤销 IPC、最小 Capability 和 Timeline UI。
- App State 迁移以保存 Pending/事务/Undo 的脱敏有界元数据；写入正文不得进入 SQLite。
- 在 App State 专用目录保存 Windows 用户上下文保护的 Undo Blob；项目目录不保存 Undo 数据。
- 复用 AM0 授权/事务/撤销契约和现有 ProjectHandle、Validator、单 Writer、Recovery、Provider/Vault 实现。
- 在替代路径证据通过后退役旧产品入口；历史后端源码可仅为 G3～G5 回归保留，但不得重新挂载或调用。

### 3.2 未授权

- GA2 产品批准、Sprint 6、NG-601～NG-606 或公开发布。
- 删除、移动、重命名、递归操作、任意绝对路径、项目外读取/导入、Shell、Process、Opener 或浏览器网络。
- Agent 写入 `AGENTS.md`、`CLAUDE.md`、Skill/安全配置、`published/` 或非 Markdown 文件。
- 自动覆盖整文件/整章、提升章节状态、更新 `materials/000.*`、关键 `project.md` 元数据或规则文件而不单独确认。
- 完整聊天正文、API Key、隐藏推理、Provider 原始响应、Undo 明文或绝对路径进入 SQLite、日志、事件或证据。
- 自动跨 Provider Fallback、后台继续写入、后台重试或静默撤销。

## 4. 已确认决策

### AM2-Q001——写入 Action 与 Patch 格式

建议：保持 AM0 `AgentActionPayloadV1` 和 `ToolArgumentsV1` 不变；AM2 Runtime Schema 开放两项已注册写工具。create 的 `content` 是完整 UTF-8 Markdown；patch 的字符串必须是严格 `novelgainker.patch-v1` JSON Envelope，只允许精确唯一匹配的局部替换或显式高风险整文件/整章/状态/Ledger 替换。未知字段、模糊匹配、空结果或无变化全部拒绝。

状态：**已接受**（包含在完整 AM2 授权中）。

### AM2-Q002——目录与验证边界

建议：普通 write-safe 仅覆盖 `characters/`、`materials/`（不含 `000.*`）、`plot/`、`world/` 与 Draft `chapters/` 下的 Markdown；`project.md`、整文件/整章、章节状态和 Story Ledger 归 `always-confirm`。规则文件、隐藏路径、`published/`、资产、项目外路径和任意文件操作保持阻断。所有目标在 Preview 和 Commit 时都运行路径、符号链接、UTF-8、大小、Schema/Validator 与来源 Hash 校验。

状态：**已接受**。

### AM2-Q003——Turn 与事务边界

建议：单 Turn 仍最多 8 Agent Steps、16 Tool Calls、24 个唯一文件、8 个修改文件、一个写事务和 96 KiB Provider 请求。一个 Action 可提出最多 4 个工具；一旦出现写入批次，该批次冻结为唯一事务，不再向 Provider 回传已提交正文。任何前置失败、混合风险待确认或取消都保持零写入。

状态：**已接受**。

### AM2-Q004——三种授权策略

建议：默认 `auto_apply_safe`；write-safe 在 Undo Record 已准备且全部前置校验通过时自动提交，always-confirm 等待确认。`review_every_write` 的所有写入都等待确认；`plan_only` 只展示 Diff 且不能确认提交。策略只允许无活动 Turn、无 Pending 写入时显式切换，并进入下一 Turn 冻结快照。

状态：**已接受**。

### AM2-Q005——确认与 Pending 生命周期

建议：Pending Plan 只保存在当前 App 进程内，UI 展示完整逐文件 Diff、风险、来源 Hash 和事务 Hash。确认 IPC 只提交不透明 Pending ID、绑定 Hash 与 `confirmed=true`，不接受正文；拒绝、项目关闭、Provider 切换、Session 清空或重启均丢弃 Pending 且项目零写入。

状态：**已接受**。

### AM2-Q006——Undo 与恢复

建议：提交前把源集合序列化为最小 Undo Payload，并用 Windows DPAPI 当前用户上下文保护；每 Turn 最大 2 MiB、每项目最多 20 条/20 MiB、保留 30 天。Undo 只在每个目标当前 Hash 等于该事务结果 Hash 时可用；否则标记冲突并拒绝覆盖。写事务使用组 Journal；崩溃后只能完成安全回滚或进入 `recovery_required`，不得静默部分成功。

状态：**已接受**。

### AM2-Q007——连续实施与旧入口退役

建议：按 NG-A201 → NG-A206 依赖顺序实施并自动进入下一 Story；A204/A205 替代路径通过后，A206 固定 App 中栏只挂载 Agent Timeline，并由边界门禁止旧 Conversation/Router/表单重新导入或调用。历史后端仅保留到不影响 G3～G5 回归的最小范围。完成后只提交 GA2 技术审查并停止。

状态：**已接受**（产品负责人明确授权 AM2 后续相关的所有内容）。

## 5. Stories

### NG-A201——受限 Markdown create/patch/validate/diff 工具

- [x] AM2 Runtime Schema 严格开放 4 读 + 2 写工具，双 Provider 使用同一 Schema。
- [x] create/patch 只产生 App 验证后的 Preview，不直接接受任意文件操作。
- [x] Patch Envelope、路径、来源 Hash、Markdown、大小、无变化和敏感内容负面矩阵通过。
- [x] 每个 Preview 显示目标、操作、变更形态、风险、来源/结果 Hash 与完整 Diff。

### NG-A202——多文件 Turn 事务与撤销

- [x] 一个写批次形成一个最多八文件的 `MultiFileTransactionPlanV1`。
- [x] 全部源 Hash、目标不存在、Policy、分类和 Undo 前置校验通过前写入数为 0。
- [x] 同一 Writer 下组提交成功或恢复到完整源集合；故障注入与重启恢复通过。
- [x] Undo Blob 受 Windows 用户上下文保护；目录/SQLite/事件/日志不含明文正文。
- [x] Undo 不覆盖后续人工修改，过期/清空/冲突/解密失败均稳定可见。

### NG-A203——授权策略与高风险门

- [x] 三种策略可见、可切换并冻结到下一 Turn。
- [x] `auto_apply_safe` 仅自动提交 write-safe + Ready Undo 的完整事务。
- [x] `review_every_write` 与 always-confirm 在确认前零写入，确认绑定不可复用或跨项目。
- [x] `plan_only`、forbidden-v1、取消、过期和来源变化不能通过确认绕过。

### NG-A204——项目与实体 Skill 写入迁移

- [x] 小说项目、人物、设定、素材和剧情意图从同一输入选择对应 Skill。
- [x] 一个 Turn 可安全创建/局部更新一个或多个关联 Markdown，并展示事务 Diff/Undo。
- [x] ID、Frontmatter、路径、Schema、`000.*` 保护和 G3～G5 Canon 格式回归通过。

### NG-A205——Storytelling、连续性、状态与 Ledger 迁移

- [x] 写章、续写、修订、连续性与下一章衔接进入 `novel-story-telling`。
- [x] Draft 章节创建可 write-safe；整章替换、章节状态、Ledger 更新分别保持 always-confirm。
- [x] 状态与 Ledger 不因正文提交自动变化；混合事务不允许部分 Apply。
- [x] 章节手动编辑、来源过期、blocking 连续性和 Ledger 失败不覆盖 Canon。

### NG-A206——双 Provider E2E 与旧入口退役

- [x] DeepSeek/OpenAI Loopback 共用从自然语言到 read/write Tool、Diff、确认、提交与 Undo 的完整状态机。
- [x] App 中栏只挂载 Agent Timeline；边界门禁止旧 Conversation/Router/表单重新成为入口。
- [x] App State、Command Manifest、Permissions、Capability、TypeScript Client、UI 与 Evidence 同步。
- [x] Agent 启动会在没有真实 Provider Run 时精确终止并落盘迁移遗留的孤儿 `Generating` 状态；拒绝发送时保留输入、移除未发送气泡并展示可访问恢复提示。
- [x] 启动后已归类为 `RecoveryRequired` 的旧状态保留为惰性兼容历史，但不阻塞项目/Provider、不在当前 UI 展示、不自动放弃且不写 Canon。
- [x] JSON-object Provider 获得精确 `AgentActionPayloadV1` 字段形状；异步失败在 Session Reload 后仍可见，保留稳定错误代码并恢复原输入。
- [x] Agent Provider 完成请求在响应头与正文阶段均可中断；取消后不重试、不启动新工具或写入，Timeline 和窗口关闭复用同一取消状态。
- [x] 真实 Provider Live Smoke 未获额外授权时保持不执行且非阻断。

## 6. Exit Gate GA2

- [x] 人物、设定、素材、剧情与章节可在同一项目会话中持续创建或更新。
- [x] 多 Skill、多文件 Turn 满足路径、Schema、Hash、Diff、风险、确认、事务、恢复和 Undo 契约。
- [x] DeepSeek/OpenAI 共用完整路径，禁止自动 Fallback；项目与 Provider 隔离通过。
- [x] 旧入口不可达，离线编辑、Provider、G1～G5 Canon 与 Recovery 回归不退化。
- [x] 完整 `pnpm quality`、Debug/Release Build、桌面 Smoke 与 AM0/AM1 锁全部通过。
- [x] GA2 Manifest 的 `ga2Approved` 保持 `false`，等待独立产品批准。

### GA2 技术结果

- AM2 Runtime Contract Set Hash：`812b8d9208f46e70899ebb64681878f13a5a9854baf64f7afabd473e4dd64fa7`；AM1 Hash 保持 `a46ace17642e407f0f2b10c0579ddc7044352e2cea59fb2e7915d0813aeaa425`。
- 生产路径注册 9 项 Skill、4 个读取工具与 2 个受治理写工具；三种授权策略、八文件事务上限、Hash 绑定 Pending 和 DPAPI Undo 已接入统一 Timeline。
- App State 已迁移到 v10；Tauri 登记 60 条命令，其中 9 条为 Agent IPC。Pending 写入正文只存在内存，完整聊天正文仍不持久化。
- DeepSeek/OpenAI Loopback 共用严格 Agent Action 与写事务路径；旧 Conversation/Router/表单入口不可达，历史后端只为 G3～G5 回归保留。
- 完整 `pnpm quality` 通过，包括 29 个前端测试文件/161 项测试、Workspace Rust 测试、严格 Clippy、AM0/AM1/AM2 契约锁、G1～G5 回归、Debug/Release 构建与桌面 Smoke。
- 真实 Provider Live Smoke 未执行；未读取真实凭据，也未发起真实网络请求。
- Story 证据：`target/qa/ng-a201/`～`target/qa/ng-a206/`；GA2 Manifest：`target/qa/ga2/ga2-manifest.json`；Review Report：`target/qa/ga2/ga2-review-report.md`。
- GA2-R001 已在技术提交 revision 2 中关闭：迁移遗留的孤儿 Workflow `Generating` 状态不再永久阻断 Agent；真实 Provider Run 仍拥有更高优先级且不会被回收；失败输入保持可重试。
- GA2-R002 已在技术提交 revision 3 中消除原项目恢复死锁；其“只放行原项目并显式结束”的交互方案由后续 GA2-R003 产品决策取代。
- GA2-R003 按产品负责人明确授权实施：迁移遗留的 `RecoveryRequired` 作为惰性兼容历史保留，不自动删除、不写 Canon，但不再阻塞项目或 Provider 操作，也不在启动页和统一 Agent 中展示；真实运行态继续阻塞。
- GA2-R004 已在技术提交 revision 5 中关闭：JSON-object Provider 获得精确 `AgentActionPayloadV1` 字段形状；异步失败在 Session Reload 后仍显示可操作错误与稳定错误代码，提交内容恢复到输入框，非法输出执行零工具、零写入。
- GA2-R005 已在技术提交 revision 6 中关闭：Provider 完成请求在等待响应头和读取正文时都可由取消信号直接中断，取消不触发重试；响应、只读工具、写入预览和事务提交前均有取消安全点，Timeline 立即显示可访问反馈，窗口关闭沿用同一信号。
- 修订版 Release：`target/release/novelgainker-desktop.exe`，19,578,368 bytes，构建时间 `2026-08-29 21:59:41 +08:00`，SHA-256 `6416ae8dc383561f10265e7f46929b2f8bbe52dcc575bd4756d4f338ec2e9096`。
- GA2 技术审查结论为通过，但 `ga2Approved=false`、`sprint6Authorized=false`；产品批准是独立决定。

### GA2-R001——统一 Agent 通道被迁移前 Workflow 状态阻断

- 产品验收报告：Release 中发送统一会话请求时显示 `workflow.run_active`，并使用“不能同时启动普通对话请求”的迁移前文案。
- 根因：旧 Workflow Manager 可以保留 `Generating` 检查点，但对应 Provider Run 已不存在；统一 Agent 仍把该孤儿状态视为真实并发任务。旧入口已卸载，用户也没有可达的取消操作。前端同时在请求成功前清空输入并追加乐观消息，失败后未恢复。
- 后端修订：Agent Start 先保护真实 Provider Run；仅在没有 Provider Run 时选择精确 `active_record()`，把孤儿旧 Run 终止为 `Cancelled` 并先持久化，再继续统一 Agent。该兼容回收不写 Canon，也不取消真实运行。
- 前端修订：启动拒绝会撤销未发送的乐观气泡、恢复原输入，并在输入框关联的 `role="alert"` 中展示恢复建议与稳定错误代码。
- 回归证据：Rust 测试 `ga2_retires_only_orphaned_legacy_generation_before_agent_use`、React 测试 `keeps unsent input when a real workflow is still running`、边界门与 NG-A206 Evidence 均已纳入完整质量流程。
- 构建识别：截图中的旧文案已不在当前 Source 或 `apps/desktop/dist`；验收必须完全退出旧进程后启动上方精确 Release 路径，并按 SHA-256 排除旧副本。

### GA2-R002——`RecoveryRequired` 阻断原项目恢复入口

- 产品验收报告：重新启动 Release 后，右栏“打开现有项目”在目录选择前直接返回 `workflow.run_active`，用户无法进入原项目，也就无法到达统一 Agent 的恢复操作。
- 根因：Workflow Manager 启动时按既有安全规则把非终态 `Generating` 检查点恢复为 `RecoveryRequired`；项目扫描与打开又对所有活动旧状态一律阻断，形成“必须先打开项目才能处理状态、但状态又禁止打开项目”的死锁。
- revision 3 修订：项目入口曾只对 `RecoveryRequired` 且 Project ID 完全匹配的原项目放行，并在统一 Agent 中提供显式结束操作。
- 后续结论：该方案虽然消除了原项目死锁，仍会让迁移状态阻断其他项目并在无可操作上下文中制造错误；GA2-R003 因此取代该交互。

### GA2-R003——迁移恢复记录不应出现在当前工作流体验中

- 产品验收报告：未打开项目时选择项目仍出现“检测到迁移前的工作流状态”，产品负责人明确要求这种情况应忽略，并进一步授权完整惰性兼容范围。
- 后端修订：`RecoveryRequired` 仍保留在 App State/旧 Workflow Manager 中，但统一运行时的项目扫描、创建、打开、关闭以及 Provider Key/选择/连接测试不再把它视为占用；真实 `Generating`、Applying/Validating、Provider Run 等活动执行状态继续阻断。
- 前端修订：统一 Agent 不再查询或展示旧恢复记录，启动页只保留“打开或创建项目”的当前动作；真实 `workflow.run_active` 仍使用与输入框关联的可访问错误，失败输入继续保留。
- 安全边界：不自动调用 abandon、不删除兼容历史、不写 Canon、不取消真实 Provider Run，也不放宽任何 Agent 写入确认、事务或路径策略。
- 回归证据：Rust `ng_302_restored_recovery_is_inert_and_can_still_be_explicitly_abandoned`、React 启动页/真实运行态测试、静态边界门与 NG-A206/GA2 Revision 4 Evidence。

### GA2-R004——已路由到 Skill 但 Turn 在零工具调用前失败

- 产品验收报告：明末岭南故事设定已路由为 `novel-story-telling`，但终态直接显示 `failed`、0 个动作、0 次工具调用，界面没有呈现失败原因。
- 根因一：DeepSeek 的 `json_object` 模式只保证 JSON 对象，运行时 Prompt 却只引用固定 Schema 名称，没有把 `respond`、`request_tools`、`complete` 的必需字段形状交给模型；首次解析与一次修复均可能耗尽。
- 根因二：失败事件先设置错误，随后的 Session Reload 又无条件清空错误，导致稳定失败代码和恢复建议瞬间消失。
- 后端修订：每次 Agent 请求都发送冻结 Skill ID 与三种合法 `AgentActionPayloadV1` 对象的精确字段形状，继续禁止 Markdown 围栏、额外字段、跨 Provider Fallback 和未注册工具。
- 前端修订：异步失败跨 Session Reload 保留；终态本地化，失败气泡标记“Turn 失败 · 输入已恢复”，原提交内容回填输入框；安全的 `agent.*` / `agent-*` 子码按原值展示。
- 安全边界：格式无效时保持 0 工具、0 写入；未执行真实 Provider Live Smoke，也未读取凭据。
- 回归证据：Rust `storytelling_prompt_exposes_the_exact_respond_action_shape`、React 异步失败/输入恢复测试、App Error 子码测试、静态边界门与 NG-A206/GA2 Revision 5 Evidence。

### GA2-R005——取消 Turn 后 Provider 请求与窗口仍长时间不结束

- 产品验收报告：统一 Agent 的请求长时间运行；点击“取消 Turn”后界面持续显示取消中，后台仍像在运行，退出窗口也被拖延，必须先关闭项目才能顺利退出。
- 根因一：Agent 使用 `reqwest::blocking` 完成请求，取消令牌只在每次请求开始前检查；单次 120 秒 HTTP 等待和响应正文读取本身不可中断，重试与一次结构修复还会扩大最坏等待时间。
- 根因二：Provider 返回到 `Respond/Complete`、只读工具、写入预览和事务提交之间缺少取消安全点，因此取消与响应完成竞争时仍可能落盘为 `completed` 或继续后续动作。
- 后端修订：统一 Agent 切换到 Tokio 驱动的 `complete_cancellable`；等待响应头、读取正文和重试退避均用同一取消信号竞争，取消立即丢弃请求 Future 并返回稳定 `provider.cancelled`，且不会重试。Agent 在响应处理、工具启动、预览构建和 Writer/事务提交前再次检查取消；事务一旦真正开始仍保持原子完成语义。
- 前端与退出修订：点击取消立即进入 `cancelling` 状态，以 `aria-live` 显示“不会再启动新的工具或写入”；收到终态后释放 Turn。窗口关闭原有取消钩子现在可以真正中断 Agent HTTP Future，不再等待阻塞请求自然超时。
- 回归证据：双 Provider Loopback 测试分别覆盖响应头未返回和正文中途停滞，均验证快速取消且单请求零重试；React 覆盖即时取消反馈与终态释放；静态边界门、NG-A206 与 GA2 Revision 6 Evidence 同步校验。

## 7. Review Gate 与授权记录

AM2 Story Packet Review Gate：**通过**（2026-08-28）。

批准记录：产品负责人明确回复“批准 GA1 产品验收；授权AM2 后续相关的所有内容”。

授权解释：GA1 产品验收通过；允许按本 Packet 推荐决策连续实施 NG-A201～NG-A206、完整质量门与 GA2 技术提交。授权止于 AM2，不自动批准 GA2，不授权 Sprint 6。

执行规则：

1. Stories 仍按依赖顺序实施并分别形成可重放证据。
2. Story 技术通过后自动进入下一 Story，无需逐 Story 再等待产品授权。
3. 任一安全、事务、隐私或恢复阻断必须先修复，不得弱化 Gate。
4. NG-A206 完成后提交 GA2 技术审查并停止。

执行结论：NG-A201～NG-A206 已按依赖顺序完成；GA2-R001～GA2-R005 修复、完整质量门与 GA2 revision 6 技术提交均已通过。当前停止在 GA2 独立产品验收点；不推定 GA2 产品批准，不授权 Sprint 6，也不授权真实 Provider Live Smoke。
