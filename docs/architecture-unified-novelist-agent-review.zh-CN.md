# NovelGainker 统一 Novelist Agent 会话——架构重审

> 状态：统一 Novelist Agent 架构、GA0 与 GA1 已获产品批准；AM2 已完成 GA2 技术提交，等待 GA2 独立产品验收；Sprint 6 未授权
>
> 日期：2026-08-29
>
> BMAD 数据包：`architecture-packet`
>
> 触发原因：Windows release 包验证发现“普通聊天无工具”与目标使用体验根本不一致

## 1. 修正后的产品目标

NovelGainker 不再提供一个与项目工作流割裂的“普通 AI 对话”。打开项目后，中栏聊天即为项目级小说 Agent 会话：

1. 每条用户消息都进入统一 Agent Runtime。
2. Agent 持续理解当前小说项目、会话目标和已经完成的修改。
3. Agent 按意图选择 Novelist 的人物、设定、素材、剧情、叙事、评审、视觉或出版能力。
4. Agent 可以通过受限工具读取、创建和更新项目内 Markdown。
5. 一次对话可以连续影响多个相关 Markdown，而不要求用户复制粘贴或重新填写独立工作流表单。
6. UI 持续展示 Agent 正在读取什么、调用什么能力、修改了哪些文件以及如何撤销。

产品不再把“聊天”和“工作流”视为两个入口。工作流/Skill 是聊天背后的执行能力，不是用户必须离开对话后手工进入的表单。

## 2. 对当前问题的结论

release 中 DeepSeek 回复“无法直接操作或修改项目文件”不是 Provider 偶发问题，而是现架构的确定性结果：

- 普通对话固定为无工具、无项目 Context、零 Canon 写入。
- Router 与普通对话相互独立，只能返回一个 Workflow ID 和置信度。
- Router 高置信度也只打开空表单，不会交接聊天内容或继续执行。
- 当前 Manager 是确定性工作流状态机，不是持续的项目 Agent。
- Runtime 明确禁止读取或运行 Novelist 插件、Skill、Agent YAML 与 Python 脚本。

因此，修改普通对话 Prompt、增加“整理到项目”按钮或扩展 S4-Q006 都不能实现新的目标。

## 3. Novelist 0.2.0 的实际形态

当前安装的 Novelist 插件提供九项宿主可调用 Skill：

- `create-novel-project`
- `create-setting`
- `create-character`
- `create-material`
- `create-plot`
- `novel-story-telling`
- `analytical-review`
- `create-visual-asset`
- `publish-novel`

每个 `agents/openai.yaml` 只提供展示名称、简述和默认启动 Prompt；它们不是九个常驻、并行自治的 Agent。真正的行为来自：

- 宿主对 Skill 的持续选择与调用；
- `SKILL.md` 中的工作规则；
- 项目 Markdown、模板和参考资料；
- Skill 自带的文件、连续性检查和发布脚本；
- 宿主提供的文件、图片、命令或其他受控工具。

因此，本重审建议的目标不是引入多 Agent 并行，而是让 NovelGainker 成为一个可持续运行 Novelist Skill 语义的项目 Agent Host。

## 4. 需要重审的既有架构决策

### 4.1 必须替换

| 当前决策        | 当前含义                                                | 新方向                                                                                    |
| --------------- | ------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| ADR-006         | 只运行 NovelGainker 原生 Workflow Pack，不加载 Novelist | 改为内置、版本锁定、经审查的 Novelist-compatible Skill Pack                               |
| ADR-008         | 所有 Canon 写入均先预览、逐次人工确认                   | 改为可配置授权策略；常规 Markdown 更新可在会话授权内执行，并提供逐轮 Diff、事务记录与撤销 |
| Sprint 2 NG-205 | 普通聊天无项目、无工具、不写文件                        | 废止普通聊天模式；替换为项目 Agent 会话                                                   |
| Sprint 4 NG-406 | Router 独立于聊天，只分类并打开表单                     | 替换为会话内 Skill Router 与工具执行循环                                                  |

### 4.2 需要改写但可保留核心原则

| 当前决策 | 可保留部分                                                     | 需要改变                                                          |
| -------- | -------------------------------------------------------------- | ----------------------------------------------------------------- |
| ADR-005  | App 掌握编排与权限，不把安全交给 Provider SDK                  | 编排对象从固定表单工作流扩展为持续 Agent/Skill/Tool Loop          |
| ADR-007  | 路径校验、事务写入、格式校验继续由 Rust Core 执行              | 不再要求所有 Skill 行为都原生重写为固定 Rust 工作流               |
| ADR-009  | 保持一个前台项目 Agent、禁止并行 Canon Writer                  | “Manager”改为单 Agent Runtime；不需要多 Agent 才能实现 Skill 体系 |
| ADR-011  | 保留 DeepSeek/OpenAI、固定 Provider 和禁止跨 Provider Fallback | 两家 Provider 必须支持同一内部 Agent Action 协议                  |
| ADR-013  | 保留三栏与中栏主交互                                           | 中栏从普通聊天/表单集合改为统一 Agent Timeline                    |

### 4.3 建议继续保持

- Markdown 和 assets 是唯一项目 Canon。
- 文件访问限制在已打开项目的不透明会话与项目相对路径。
- API Key 只保存在 Windows Credential Manager。
- Provider 不获得任意文件系统、Shell、URL 或绝对路径。
- Rust 继续独占路径验证、原子写入、源 Hash 检查、恢复与回读校验。
- 同一时刻最多一个 Canon Writer；禁止跨 Provider 自动 Fallback。
- 项目文本和 Provider 输出始终是不可信数据，不能提升权限。

## 5. 推荐目标架构

### 5.1 单一项目 Agent 会话

项目打开后创建一个 `ProjectAgentSession`，持有：

- 当前项目的不透明 Session ID；
- 当前 Provider/Preset/Model；
- 对话消息与可压缩的会话摘要；
- 当前目标、已调用 Skill、已读取来源和最近修改结果；
- 当前授权策略；
- 唯一活动 Turn 与唯一 Canon Writer 租约。

项目关闭、Provider 切换、取消或崩溃恢复均有明确状态转换。不同项目不得共享正文、摘要、工具结果或授权。

### 5.2 Novelist-compatible Skill Registry

发行包内置一个经过许可与安全审查的固定 Skill Pack，而不是运行用户目录或 Codex Plugin Cache：

- Skill ID、版本、触发说明、规则、模板、参考资料和允许工具在构建期固定并 Hash。
- Release 不依赖用户另行安装 Codex、Novelist 插件或 Python 环境。
- 每个 Skill 声明允许读取的目录、允许的操作、输出契约和风险级别。
- Skill 更新通过 NovelGainker 自己的版本迁移、变更记录和回归 Gate 发布。

这可以实现 Novelist 的使用体验，同时避免 release 读取任意插件缓存导致不可复现或供应链漂移。

### 5.3 会话内 Skill Router

每轮消息经过同一个 Agent Loop：

1. 读取当前会话状态和有界项目目录摘要。
2. 选择一个或多个适用 Skill；普通讨论也由通用小说协作 Skill 处理，不存在无工具旁路。
3. 形成明确的读取/搜索/写入 Action Plan。
4. App Policy 校验计划，拒绝越界路径、未注册工具和不允许的操作。
5. 执行受控读取并把结果作为不可信 Tool Result 返回 Agent。
6. Agent 生成文件 Patch 或新的 Action Plan。
7. Rust 校验并事务性应用允许的修改。
8. Timeline 展示本轮使用的 Skill、来源、修改文件、Diff、诊断和撤销入口。

一次 Turn 可以顺序修改多个 Markdown，但必须作为有界事务计划执行，禁止并行 Writer。

### 5.4 受控文件工具

建议的第一版 Agent 工具：

- `project.list_files`
- `project.read_markdown`
- `project.search_markdown`
- `project.create_markdown`
- `project.patch_markdown`
- `project.validate_markdown`
- `project.get_diff`
- `project.revert_turn`

工具只接受不透明项目会话、项目相对路径、源 Hash、结构化 Patch 和 Turn ID。Provider 不能提交绝对路径、Shell 命令、任意 URL、最终权限决定或绕过校验的完整磁盘操作。

### 5.5 持续更新与授权策略

为符合“聊天内容持续更新进各种 Markdown”的目标，建议提供三种项目级策略：

1. `review-every-write`：每次写入前确认，适合谨慎用户。
2. `auto-apply-safe`：推荐默认。会话内自动创建或局部 Patch 普通 Markdown；每轮结束展示 Diff 和一键撤销。
3. `plan-only`：只讨论和规划，不落盘。

以下高风险动作在任何策略下都仍需单独确认：

- 删除、移动、重命名或覆盖非 NovelGainker 管理的文件；
- 大范围替换、整章/全稿覆盖和跨多个章节的批量改写；
- 章节状态提升、Story Ledger 关键状态变更；
- 发布、外部导出、图片导入或项目外读写；
- 修改项目规则、Skill Pack、权限或安全配置。

`auto-apply-safe` 的安全基础从“每文件事前确认”转为“会话授权 + 路径/Schema/Hash 门禁 + 原子事务 + 逐轮 Diff + 可恢复撤销”。

### 5.6 会话持久化

为了“持续”工作，需要重新审查当前禁止持久化聊天与正文摘要的规则：

- 项目 Canon 继续只存在 Markdown。
- 会话消息与 Agent 摘要属于可删除的本机工作状态，不是 Canon。
- 默认只持久化有界会话摘要、消息索引、Skill/Tool 元数据和 Turn 事务记录。
- 是否持久化完整聊天正文必须由产品单独决定，并提供清空、保留期和隐私说明。
- 所有持久化内容必须与项目 ID 隔离，不含 API Key、绝对路径或未授权项目正文副本。

## 6. 两种实现路线

### 路线 A——内置 Novelist-compatible Skill Pack（建议）

- 把选定版本的 Novelist Skill 语义、模板和参考资料经过许可审查后固化进发行包。
- Rust 实现受控工具和事务层；Provider 通过统一内部 Agent Action Schema 使用工具。
- Python 脚本不直接进入 release；需要的确定性行为移植到 Rust，或改成声明式工具调用。
- DeepSeek/OpenAI 共享同一 Agent 状态机与本地 Validator。

优点：release 自包含、可测试、支持两家 Provider、权限可控。代价：需要维护兼容层和 Skill 升级流程。

### 路线 B——依赖外部 Codex/Novelist Host

- NovelGainker 启动或连接已安装的 Codex/Novelist 环境，由外部 Host 负责 Skill 与文件工具。

优点：最接近插件原生行为。代价：需要 Codex 登录/安装、Process/Shell、外部版本与权限管理；DeepSeek 路径不再等价，Windows release 也不再自包含。

第一版建议拒绝路线 B。

## 7. 受影响的 Sprint 与 Gate

### 必须纳入迁移回归

G0～G5 继续作为 as-built 历史，不倒退或重开原 Gate；以下能力必须作为 Agent Migration Program 的迁移输入和回归基线：

- Architecture Review：ADR-005～ADR-009、Agent Runtime、Workflow Runtime、数据持久化与安全模型已由 Amendment A-001 更新。
- Sprint 2：替换 NG-205 的普通对话产品语义，同时保持 Provider/Vault 回归。
- Sprint 3：迁移 NG-301～NG-304 的 Workflow Pack、Manager、Prompt/Context、Proposal/Apply 能力。
- Sprint 4：迁移 NG-402～NG-406 的表单工作流、Router 与单文件执行模型。
- Sprint 5：把 Context Builder、Storytelling、连续性、章节 Patch 与 Ledger 接入会话 Skill/Tool，并保持 Canon 契约。

### 暂停

- Sprint 6 Story Packet Review Gate。
- NG-601 及后续实现。

Sprint 6 的评审、视觉和出版能力可以保留产品目标，但必须先重新表达为统一 Agent 会话中的 Skill，而不是独立工作流卡。

## 8. 新的端到端验收场景

1. 打开一个已有 Markdown 小说项目。
2. 用户只在中栏聊天：“把刚才确定的海禁规则补进世界观，并让主剧情中的第三次转折与它一致。”
3. Agent 自动选择 setting 与 plot Skill，读取相关 Markdown 和项目规则。
4. Agent 顺序 Patch `world/...md` 与 `plot/...md`，不要求用户复制聊天内容或打开表单。
5. UI 在同一 Turn 中展示读取来源、Skill、两个文件的 Diff、校验结果和撤销入口。
6. 用户继续说：“再根据这个变化调整下一章计划。”
7. Agent 继承当前会话目标与刚完成的文件状态，选择 storytelling Skill 并更新对应 outline/计划 Markdown。
8. 用户关闭并重开项目后，可以从有界会话摘要继续；项目 Canon 完全可由 Markdown 独立读取。
9. 用户撤销上一 Turn，两个文件安全恢复到各自的源 Hash 版本。
10. 恶意项目文本、恶意 Provider 输出、越界路径、并发写入和过期 Hash 均被本地 Policy 阻断。

## 9. 已确认架构决策

### AR-Q001——统一入口

建议：删除“普通 AI 对话”产品概念；打开项目后的聊天全部进入统一项目 Agent。未打开项目时只允许创建/打开项目和无写入的产品帮助。

状态：**已接受**（2026-08-28）。

### AR-Q002——Novelist 集成方式

建议：选择路线 A，发行包内置版本锁定、经审查的 Novelist-compatible Skill Pack；不依赖用户的 Codex Plugin Cache 或外部 Agent Host。

状态：**已接受**（2026-08-28）。

### AR-Q003——自动写入策略

建议：默认 `auto-apply-safe`；常规项目内 Markdown create/patch 在项目会话授权后自动执行，每轮提供完整 Diff 与一键撤销。高风险操作始终单独确认。

状态：**已接受**（2026-08-28）。

### AR-Q004——多文件 Turn

建议：允许一个 Turn 顺序修改多个 Markdown，以 Turn 事务记录绑定全部源 Hash、结果 Hash、校验和恢复信息；仍保持唯一 Writer，不并行 Apply。

状态：**已接受**（2026-08-28）。

### AR-Q005——会话持久化

建议：默认持久化有界摘要、Turn/Skill/Tool 元数据和撤销记录，不默认持久化完整聊天正文；用户可显式选择保留完整聊天。

状态：**已接受**（2026-08-28）。

### AR-Q006——现有实现处置

建议：保留 Rust 项目会话、路径验证、原子写入、Hash、恢复、Provider/Vault 与现有领域 Validator；重构 Conversation、Router、Workflow Manager 和确认 UI，不从零重写整个应用。

状态：**已接受**（2026-08-28）。

## 10. Architecture Review Gate

Review Gate 结论：**通过**（2026-08-28）。产品负责人批准统一 Novelist Agent 会话方向，暂停 Sprint 6，并只授权修订 ADR 与 Sprint Plan；本次不授权运行时代码实现。

- [x] AR-Q001～AR-Q006 获产品确认。
- [x] 已新增/替换 ADR，明确 Novelist Skill Pack 的许可、版本与升级策略。
- [x] 冻结 Agent Action/Tool/Turn/Undo 的本地契约。
- [x] 冻结 `auto-apply-safe` 的低风险与高风险操作矩阵。
- [x] 冻结会话持久化、清空、项目隔离与隐私策略。
- [x] 已授权修订 Sprint Plan，先安排统一 Agent Runtime 迁移，再恢复 Sprint 6。
- [x] 已授权同步 `CLAUDE.md` 的当前架构与实施禁令；运行时代码仍须等待独立 Story Packet。
- [x] AM0 Story Packet 已按 BMAD 顺序完成；NG-A001～NG-A004 与 GA0-R001～GA0-R003 均通过，GA0 已于 2026-08-28 获产品批准。
- [x] 产品负责人统一授权并完成 AM1-Q001～Q007、AM1 Story Packet Review Gate、NG-A101～NG-A104 与 GA1 技术提交，随后独立批准 GA1 产品验收。
- [x] AM1 建成统一 Timeline、9 项 Skill、4 个只读工具、App State v9、56 条 Command 与双 Provider 严格 Action Loop；该阶段端到端 Agent 项目写入数为 0。
- [x] 产品负责人统一授权 AM2-Q001～Q007、AM2 Story Packet Review Gate、NG-A201～NG-A206、完整质量门与 GA2 技术提交。
- [x] AM2 已实现 4 读 + 2 写工具、三种授权策略、八文件原子事务、Hash 绑定确认、DPAPI Undo、App State v10 与 60 条 Command；旧产品入口不可达。
- [x] GA2 技术提交已生成且无技术阻断；`ga2Approved=false`、`sprint6Authorized=false`，当前停止并等待 GA2 独立产品验收。

批准记录：产品负责人明确回复“AR-Q001～AR-Q006 按建议接受，批准统一 Novelist Agent 会话方向；暂停 Sprint 6，授权修订 ADR 与 Sprint Plan，不授权开始运行时代码实现。”

后续授权记录：产品负责人先批准 GA0 并授权完整 AM1，随后回复“批准 GA1 产品验收；授权AM2 后续相关的所有内容”。该后续授权已执行到 GA2 技术提交为止，不构成 GA2 产品批准或 Sprint 6 授权。
