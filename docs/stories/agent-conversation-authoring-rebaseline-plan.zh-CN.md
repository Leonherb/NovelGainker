# NovelGainker 对话式小说创作重基线 Plan

> 状态：RB-Q001～RB-Q008、GRB0 与 Agent Protocol V2 已关闭；AM-R1 已完成；GAR1 技术提交就绪，等待独立产品评审
>
> 日期：2026-08-30
>
> BMAD 数据包：`brownfield-resume-packet` → `execution-packet`
>
> 范围：重新定义现有 AM1/AM2 之后的产品边界、目标架构、实施阶段与验收门

## 1. BMAD Routing Brief

- Packet type：`brownfield-resume-packet`
- Project type：Windows-first、local-first Tauri 桌面应用
- Project level：4
- Runtime context：Codex / local repository
- Confidence：high
- Current phase：GAR1 technical submission；NG-R101～NG-R106 与完整本地质量门已完成
- Recommended next move：独立评审 `target/qa/gar1/gar1-review-report.md` 与 `gar1-manifest.json`
- Route-out：只有 GAR1 获显式产品批准并形成新的 AM-R2 Story Packet，才能继续后续实现

为什么现在先做重基线：

1. 当前问题不是单一格式错误，而是“聊天、Skill、上下文、长内容生成、Canon 落盘”之间的职责边界错误。
2. 现有安全写入、Hash、事务和 Undo 有复用价值，但不能证明产品已经实现持续小说协作。
3. 继续在 `AgentActionPayloadV1` 上补 Prompt，只会反复暴露长内容、上下文丢失和路由脆弱问题。

## 2. 产品定义

NovelGainker 是一个以 AI 对话为唯一主工作流的小说创作 App。

作者不需要先学习文件结构、工作流表单或 Skill 名称。作者通过聊天构思、设定、规划、写章和修订；App 在后台调用内置 Novelist-compatible Skill Pack，读取项目 Canon，将稳定的小说事实写回结构化 Markdown，并在发生冲突时让作者决定哪个版本成为 Canon。

产品必须同时满足五个原则：

1. **Chat-first middle pane**：保留三栏常驻布局；中栏只承载可见对话和输入，项目、文件、Context、来源、工具活动、Diff 与冲突决议分别进入左右栏。
2. **Canon-first**：角色、世界规则、剧情事实和章节正文最终以项目 Markdown 为准；会话记忆不能成为隐藏事实库。
3. **Skill-complete**：运行时使用 Skill 的 instruction、模板、参考资料和确定性上下文能力，而不是只使用 Skill 名称和简短提示。
4. **Context-by-retrieval**：几十万字工程不能整本塞入 Prompt；必须通过索引、分层摘要、相关性检索和按需回读构造每轮上下文。
5. **Host-governed**：模型负责理解、创作和提出候选变更；App 负责路径、Schema、冲突、风险、Diff、事务、撤销和最终写入。

## 3. 用户故事重写

### US-R001——启动时选择项目

作为作者，我每次冷启动 App 时都会先看到项目选择弹窗，可以新建项目、打开已有项目或选择最近项目，而不是先进入一个空工作台。

验收：

- 冷启动不自动打开旧项目，先显示项目选择弹窗。
- “新建项目”和“打开现有工程”同等可见；最近项目只是快捷入口，不是唯一入口。
- 打开已有工程时只读识别结构并报告问题；失败不得修改项目。
- 取消弹窗后停留在无项目壳层，Agent 输入不可用。

### US-R002——干净的项目对话界面

作为作者，我打开项目后首先看到一个干净的对话界面，可以直接询问、构思、写作或修改现有内容。

验收：

- 保留三栏常驻布局：左栏承载项目、文件树和 Provider；中栏只承载用户/AI 对话与输入；右栏承载文件编辑、Context、来源、工具活动、Diff 和冲突决议。
- 左右栏可以折叠和调整宽度，但不能把工具卡、Diff、Context 卡或确认卡插入中栏对话流。
- AI 对已有内容的回答必须显示实际读取来源；不能假装已经读取工程。
- 普通讨论、项目问答和执行型请求都进入同一个项目 Agent，不存在无 Context 的旁路聊天。
- 完整可见聊天历史和结构化会话记忆保存在项目内，关闭、重开或移动工程后仍可恢复。

### US-R003——无前置设定的新工程

作为作者，我可以先创建一个空白小说工程，不必先填写角色、世界观、剧情或风格问卷。

验收：

- 新建只要求目标目录；项目名可以使用可编辑默认值。
- 创建标准目录、最小 `project.md`、默认项目规则和可重建元数据，不创建虚构角色或世界事实。
- 创建完成后直接进入对话，AI 可询问作者下一步，但不得强迫完成初始化向导。

### US-R004——可选的先设定后写作

作为作者，我也可以先通过聊天设定角色、世界观、剧情或写作风格，再开始写章节。

验收：

- 作者可以自由决定顺序，Skill 不要求固定流水线。
- 同一轮可以涉及多个领域，但每个领域的来源、候选事实和目标文件必须可追踪。

### US-R005——对话事实自动沉淀为 Markdown

作为作者，当聊天中形成了稳定的角色、世界、设定、素材或剧情事实时，App 会把它们沉淀到对应 Markdown，而不要求我复制粘贴。

验收：

- Agent 先生成 `CanonProposalV1`，声明事实类型、目标实体、来源消息、目标文件和置信度。
- 无冲突的新实体创建或安全局部补充可按项目策略自动写入，并显示 Diff 和撤销入口。
- 模糊、重复、低置信度或高风险候选不得静默写入。
- 写入成功后立即刷新 Context Index；后续回复以新 Canon 为准。

### US-R006——每轮都检查相关 Canon

作为作者，我希望 AI 在涉及角色、世界、设定、剧情或连续性时，始终检查相关项目内容。

“始终检查”不等于每轮读取所有文件，而是每轮必须经过 `Context Planner`：

- 先读取项目索引、实体别名、当前章节/剧情线程和最近修改；
- 再按请求和 Skill 选择必须读取的文件或片段；
- 写入前重新校验相关来源 Hash 与连续性；
- Timeline 展示本轮实际使用的来源，来源不足时继续读取或明确说明无法判断。

### US-R007——使用完整 Novelist Skill 语义

作为作者，当我规划、写章、续写、修订或评审时，AI 会按 Novelist Skill 的完整规则工作。

验收：

- Skill 运行时按需加载 instruction、模板和参考资料，而不是只注入一段缩写说明。
- 上游被排除的 `build_story_context`、continuity、story IO 和 Ledger 行为，经安全审查后移植为 Rust 或声明式工具；release 仍不运行外部 Python、用户 Skill 或插件缓存。
- Skill 选择基于语义和项目状态；固定关键词只做候选提示或安全过滤，不能单独决定最终 Skill。
- 不相关请求允许零 Skill；项目写作请求不得无条件回退到 `novel-story-telling`。

### US-R008——随时标记上下文

作为作者，我和 AI 都可以把重要角色、地点、规则、剧情线程、线索或章节标记为当前 Context。

验收：

- `ContextMarkV1` 只引用 Canon 文件、稳定实体 ID、章节或标题区间及来源 Hash，不复制一份隐藏事实。
- AI 可以提出标记，用户可以固定、取消固定或检查来源。
- 尚未进入 Canon 的聊天事实必须先走 Canon Proposal，不能只靠隐藏 Mark 长期保留。
- Mark 保存为项目内 Agent 元数据，随工程移动；删除 Mark 不修改小说 Canon。

### US-R009——冲突由作者决策

作为作者，当新内容与现有 Canon 冲突时，我会看到冲突证据并选择解决方式。

验收：

- 冲突卡同时显示新候选、旧 Canon、来源位置、受影响文件和 Agent 解释。
- 固定提供“保留现有”“采用新设定”“合并编辑”“保存为备选/素材”“取消本次变更”。
- 未决冲突阻断相关事务，不允许 `auto-apply-safe` 绕过，也不允许部分提交关联文件。
- 决议写入 Canon 后刷新索引，并记录可见的 Turn 决策摘要；不保存隐藏推理。

### US-R010——支持几十万字长篇工程

作为作者，我可以在几十万字、上百章的项目中持续写作，而不会依赖把整本小说放入单次 Prompt。

验收：

- Context Index 可从 Markdown 全量重建，并支持实体、别名、关系、世界规则、剧情线程、章节、时间线、线索和状态查询。
- 形成项目摘要、卷/剧情弧摘要、章节摘要、实体卡、Story Ledger、最近工作集和当前 Context Marks 的分层上下文。
- 每个 Turn 产生可检查的 `ContextPlanV1`，在预算内选择摘要或原文片段，并保留来源 Hash。
- 写章使用独立内容生成通道和可分段 Composition Job，不把整章嵌套进控制 Action JSON，也不受统一 4096-token 输出上限截断。
- 使用不少于 120 章、50 万中文字符的合成项目完成检索、连续性、冲突和写章回归；测试不得使用真实用户手稿。

## 4. 目标体验

### 4.1 启动流程

```text
冷启动
  → 项目选择弹窗
    → 新建最小工程 ─┐
    → 打开已有工程 ─┼→ 只读扫描与索引 → 项目对话
    → 最近项目 ─────┘
```

新建工程不触发角色/世界观向导。需要引导时，作者在对话中说“先帮我设定世界观”或点击可选示例 Prompt。

### 4.2 每轮对话流程

```text
用户消息
  → Skill Selection
  → Context Plan
  → 按需读取 Canon / Skill 资源
  → 对话回复，或创建 Draft Artifact
  → Canon Proposal + 冲突检查
  → 自动安全写入 / 等待冲突决议
  → Diff + Undo + Context Index 刷新
```

三栏职责固定为：左栏管理工程与文件导航，中栏只显示对话，右栏检查 Context、来源、工具活动、文件内容、Diff、冲突与确认。内部 Schema 修复和 Provider 适配不向用户暴露。

## 5. 目标架构

### 5.1 Conversation Orchestrator

每个项目一个 `ProjectAgentSessionV2`，负责可见对话、当前目标、最近决策、Context Marks、活动 Skill、唯一活动 Turn 和唯一 Writer。重启后从项目内聊天历史、结构化会话记忆、Canon 和可重建索引恢复；不迁移 Provider 隐藏状态。

会话摘要必须由专门的 Memory 更新步骤生成并在后续 Turn 真正注入，不能把上一条终端回复直接冒充摘要。

项目内新增非 Canon 的 `.novelgainker/agent/`：

- `sessions/<session-id>/session.json`：Session 快照与版本；
- `sessions/<session-id>/turns/<sequence>-<turn-id>.json`：每个终态 Turn 的完整可见用户/AI 消息、来源引用和结果；
- `sessions/<session-id>/memory.json`：当前目标、已确认决定、未决问题和结构化摘要；
- `context-marks.json`：当前持久 Context Marks。

这些文件随工程保存和移动，但不属于小说正文 Canon。禁止写入 API Key、隐藏推理、Provider 原始响应、未脱敏底层错误和未展示给用户的内部消息。活动 Turn 只在成功、失败或取消后以原子替换形成终态文件，避免半条聊天记录。

### 5.2 Skill Runtime Loader

Skill Pack 采用渐进式加载：

1. 常驻加载 Skill ID、触发说明、允许能力和风险边界；
2. 选中 Skill 后加载完整 instruction 与模板；
3. 题材方法、评审规范和大体量参考资料按任务需要加载；
4. 任何 Skill 资源都不能扩大 Rust Policy 授权。

当前打包的 37 个资源可以继续作为供应链基础；未被运行时使用的模板、参考资料和上游 Context 行为必须纳入新的端到端证据。

### 5.3 Project Context Engine

新增可重建的 `ProjectContextIndex`，索引内容至少包括：

- 文件路径、类型、稳定 ID、标题、Hash、章节状态；
- 人物别名、关系、目标、秘密和知识边界；
- 地点、世界规则、限制和例外；
- 剧情线程、未决问题、伏笔、线索、时间线和 Story Ledger 状态；
- 章节/场景摘要及其原文来源区间；
- Context Marks、最近读取和最近修改。

搜索索引和可重建的派生缓存仍位于本机 App State；聊天历史、结构化会话记忆和 Context Marks 位于项目内 `.novelgainker/agent/`。两者都不得成为新的小说 Canon。任何准备写入的事实都必须能追溯到 Markdown 或项目内可见用户消息。

### 5.4 控制面与内容面分离

废止“把完整 Markdown 或整章塞入 `AgentActionPayloadV1.toolCalls[].arguments.content`”作为主生成协议。

新协议分为：

- `AgentControlActionV2`：只包含 Skill、Context 请求、任务类型、目标提示、风险和下一步等短结构化控制信息；
- `DraftArtifactV1`：由 App 分配 ID 的流式内容缓冲，可表示角色卡、设定、剧情、章纲或章节；
- `CanonProposalV1`：引用 Draft Artifact ID，声明目标、事实、来源和预期操作；
- `ConflictSetV1`：保存冲突两侧的结构化证据和待用户选择状态；
- `WritePlanV2`：由 Rust 将已决议 Draft 转换为路径、Diff、Hash、风险和事务计划。

模型不能直接决定磁盘路径、风险等级或提交结果。App 不要求模型在 JSON 中转义整章，也不使用精确字符串替换作为唯一修订手段。

### 5.5 长内容 Composition Job

章节和长篇修订使用 Planner → Composer → Reconciler 三段式：

1. Planner 产生小型章纲、必读来源和连续性约束；
2. Composer 以原始 Markdown 流输出 Draft，可按场景分段并携带上一段末尾和章节计划；
3. Reconciler 检查结构、连续性、冲突和目标生命周期，再交给 Rust 生成 Diff。

聊天回复、实体卡和章节可使用不同输出预算。Provider 能力不足时降级为分段生成，而不是要求返回“更短的合法写工具请求”。

### 5.6 冲突检测

冲突检测采用两层：

- 确定性层：稳定 ID、Schema、唯一字段、章节状态、时间排序、来源 Hash 和规则约束；
- 语义层：Agent 基于已读取 Canon 提出潜在矛盾，必须附带具体来源和解释。

语义层只能提出冲突，不能单独授权覆盖。所有未决冲突均进入同一个用户决议状态机。

## 6. 现有实现的处置

| 现有能力                                               | 处置                                    | 原因                                              |
| ------------------------------------------------------ | --------------------------------------- | ------------------------------------------------- |
| ProjectHandle、相对路径、符号链接与 UTF-8 门禁         | 保留                                    | 是可靠的 Host 安全边界                            |
| Markdown Schema/Validator、Hash、唯一 Writer、原子事务 | 保留并扩展                              | 可作为 Canon Commit Kernel                        |
| DPAPI Undo、取消、Provider/Vault                       | 保留并回归                              | 与新对话流程正交                                  |
| 4 个只读工具                                           | 保留实现，重做发现与 Context 编排       | 当前作用域和 8 KiB 截断不满足长篇需求             |
| 2 个模型可见 create/patch 工具                         | 改为 Host 内部 Commit 能力              | 长正文不应嵌套在控制 JSON 中                      |
| `AgentActionPayloadV1`                                 | V2 切换期间仅作迁移输入，切换完成后移除 | 现协议混合控制与内容，不保留并行历史产品路径      |
| 固定关键词 Router 与默认 storytelling 回退             | 替换                                    | 无法可靠表达多意图、零 Skill 和项目状态           |
| 只注入简短 Skill instruction                           | 替换为渐进式完整资源加载                | 当前未真正使用模板、参考资料和 Context 行为       |
| 当前 Session summary / recent turns                    | 重做                                    | 摘要未参与后续模型上下文，Timeline 也不能真实恢复 |
| 通过源码标记证明 E2E                                   | 降级为边界检查                          | 不能代替生产 Agent 状态机和真实用户旅程           |
| G3～G5 与旧 AM1/AM2 可复用实现                         | 安全内核合并进入新设计，其余移除        | 最终仓库只保留一个当前产品架构和一条运行路径      |

现有 AM1/AM2 不再作为独立历史阶段保留。可复用的 ProjectHandle、Provider/Vault、取消、Validator、Hash、事务、确认和 Undo 直接合并进新设计；旧 Router、旧 Session/Action 产品协议、旧入口、旧阶段状态和重复 Story 文档在重基线同步时移除。最终权威文档只描述当前统一设计，不携带 GA1/GA2 历史状态。

## 7. 新实施阶段

### RB0——产品与架构重基线

目标：冻结本文的产品模型、上下文模型、协议分层和迁移策略。

交付：

- [x] RB-Q006 Agent Protocol V2 评审通过。
- [x] RB-Q007 Context Mark 评审通过。
- [x] 修订架构 ADR、Sprint Plan、`CLAUDE.md` 当前授权状态。
- [x] 形成新的 AM-R1 Story Packet；AM-R2/AM-R3 在各自前置 Gate 后单独形成。

Exit Gate GRB0：所有 RB-Q001～RB-Q008 明确接受或修改；未通过前不改运行时代码。

当前状态：**通过并关闭**（2026-08-30）。GRB0 只关闭产品与协议重基线，不授权 NG-R101。

### AM-R1——项目入口、对话与上下文基础

目标：用户可以新建/打开工程，在干净对话中得到基于真实项目内容的回复。

候选 Stories：

| ID      | Story                       | 核心验收                                                     |
| ------- | --------------------------- | ------------------------------------------------------------ |
| NG-R101 | 启动项目选择与最小 Scaffold | 冷启动弹窗、新建无强制设定、已有工程只读识别                 |
| NG-R102 | Chat-first Workspace        | 对话为主界面，来源/工具/文件/Diff 为辅助面板                 |
| NG-R103 | Skill Runtime Loader V2     | 完整 instruction/模板/参考按需加载，Pack Hash 固定           |
| NG-R104 | Project Context Index       | 可重建索引、实体别名、章节/剧情/规则查询和来源 Hash          |
| NG-R105 | Context-aware Agent Loop    | 语义 Skill 选择、Context Plan、累计工具结果、真实摘要恢复    |
| NG-R106 | 生产路径只读 E2E            | 从 UI 输入到双 Provider Loopback、读取、回复和重启恢复全链路 |

Exit Gate GAR1：四类已有内容问答能读取正确 Canon、展示来源、保持零 Agent 写入；空工程也能自然对话；关键词误路由和丢失先前工具结果的回归关闭。

### AM-R2——对话 Canon 化、Context Mark 与冲突决议

目标：聊天中形成的稳定角色、世界、设定、素材和剧情能够安全进入 Markdown。

候选 Stories：

| ID      | Story                     | 核心验收                                                 |
| ------- | ------------------------- | -------------------------------------------------------- |
| NG-R201 | Control/Content V2 协议   | 控制 Action 不携带长 Markdown；Draft Artifact 可流式生成 |
| NG-R202 | Canon Proposal 与实体归一 | 候选事实映射到稳定实体、目标文档和来源消息               |
| NG-R203 | Context Mark              | AI/用户可固定 Canon 引用，Mark 可见、可取消、可重建      |
| NG-R204 | Conflict Engine 与决议卡  | 确定性+语义冲突、五种决议、未决冲突零写入                |
| NG-R205 | Host Commit Kernel V2     | Rust 生成路径/Diff/风险/事务，提交后刷新索引并可撤销     |
| NG-R206 | 人物/世界/剧情生产 E2E    | 自然对话→多 Skill→Draft→冲突/写入→重开后继续             |

Exit Gate GAR2：用户无需复制粘贴即可把对话事实沉淀到正确 Markdown；无冲突安全写入可自动完成，有冲突必定等待作者决议；任何模型输出都不能越过 Host Policy。

### AM-R3——长篇 Storytelling 与连续性

目标：完整落地 Novelist 的长篇 Context、写章、续写、修订、连续性和 Ledger 能力。

候选 Stories：

| ID      | Story                   | 核心验收                                                   |
| ------- | ----------------------- | ---------------------------------------------------------- |
| NG-R301 | 安全移植 Story Context  | 将上游 Context 逻辑移植为 Rust/声明式工具，建立分层摘要    |
| NG-R302 | 连续性与 Story Ledger   | 角色知识、关系、时间线、世界规则、线索和剧情线程检查       |
| NG-R303 | Chapter Composition Job | 章纲→分段创作→合成→验证，不受统一 4096-token 上限截断      |
| NG-R304 | 长篇修订与章节生命周期  | Draft 局部/整章修订、accepted/final 保护和风险确认         |
| NG-R305 | 50 万字合成工程 E2E     | 120+ 章检索、冲突注入、续写、重启、撤销和预算回归          |
| NG-R306 | 双 Provider 产品 Smoke  | 同一 V2 契约；真实 Provider Smoke 需另行明确授权和测试凭据 |

Exit Gate GAR3：在 50 万中文字符合成项目中，Agent 能以有界 Context 正确找到相关角色、规则、剧情线程和最近章节，完成连续性受控的下一章创作；删除 App State 后可从 Markdown 重建，不丢 Canon。

Sprint 6 的评审、视觉和 EPUB 继续暂停，直到 GAR3 获产品批准。

## 8. 验收与证据策略

新的 Gate 不再用 `source.includes(...)` 作为产品 E2E 证明。证据分四层：

1. **契约测试**：V2 Schema、预算、状态机、冲突决议、风险和数据泄漏负向矩阵。
2. **集成测试**：真实 Rust Context Engine、Skill Loader、Provider Loopback、Writer、Undo 和重启恢复。
3. **产品 E2E**：从启动弹窗、UI 对话到读取来源、生成 Draft、冲突选择、写入、Diff、撤销和重开。
4. **规模测试**：50 万字合成工程的索引重建、相关性、上下文预算、连续性和长章生成。

每个 Gate 仍需通过 `pnpm quality`；Cargo 继续 `--locked --offline`；不得读取真实用户手稿或凭据。真实 Provider Live Smoke 只能在新 Story 明确授权后执行。

## 9. 需求追踪矩阵

| 用户需求                  | 主要 Story                | Gate      |
| ------------------------- | ------------------------- | --------- |
| 启动新建/打开弹窗         | NG-R101                   | GAR1      |
| 干净对话主界面            | NG-R102                   | GAR1      |
| 新项目不强制设定          | NG-R101                   | GAR1      |
| 可先设角色/世界观         | NG-R105、NG-R206          | GAR1/GAR2 |
| 对话内容落到对应 Markdown | NG-R202、NG-R205、NG-R206 | GAR2      |
| 每轮读取检查相关设定      | NG-R104、NG-R105          | GAR1      |
| 使用 Novelist 完整能力    | NG-R103、NG-R301～R304    | GAR1/GAR3 |
| AI 随时 Mark Context      | NG-R203                   | GAR2      |
| 冲突提示用户决策          | NG-R204                   | GAR2      |
| 几十万字长篇              | NG-R301～R305             | GAR3      |

## 10. 重基线决策记录

### RB-Q001——产品入口

决定：冷启动使用项目选择弹窗；打开项目后保留三栏常驻布局。左栏负责项目、文件树与 Provider，中栏只保留对话和输入，右栏负责文件、Context、来源、工具活动、Diff、冲突和确认。

状态：**已接受并修订**（2026-08-30）。

### RB-Q002——空工程

决定：最小 Scaffold 不要求角色、世界或剧情；初始化向导改为可选对话提示。

状态：**已接受**（2026-08-30）。

### RB-Q003——Canon 自动写入

决定：沿用 `auto-apply-safe` 作为默认；仅无冲突的新建/安全局部补充可自动提交，冲突、高风险和低置信度全部等待作者。

状态：**已接受**（2026-08-30）。

### RB-Q004——会话记忆与聊天历史

决定：完整可见聊天历史、结构化会话记忆、最近决策和 Context Marks 默认保存在项目内 `.novelgainker/agent/`，并随工程复制、移动和恢复。它们是项目元数据而非小说 Canon；禁止保存隐藏推理、Provider 原始响应、API Key 和未脱敏内部数据。

状态：**已接受并修订**（2026-08-30）。GRB0 关闭后，本决定已替换“完整聊天默认不持久化”的旧目标规则；当前 as-built 存储行为只作为 AM-R1 迁移输入。

### RB-Q005——Novelist Context 能力

决定：保留自包含 Skill Pack 路线；把当前排除的上游 Context、continuity 和 Ledger 行为经审查移植为 Rust/声明式工具，不在 release 执行外部 Python。

状态：**已接受**（2026-08-30）。

### RB-Q006——Agent 协议

Revision 2：停止扩展 V1；Conversation、Control、Content、Canon、Persistence 完全分离，Memory 由 Host 增量更新并按阈值压缩，预算覆盖单次与每 Turn 累计限制。完整字段、状态机、安全边界、持久化和迁移方案见 `agent-protocol-v2-context-mark-review.zh-CN.md`。

状态：**Revision 2 已接受**（2026-08-30）；RB-Q006 已关闭。

### RB-Q007——Context Mark

Revision 2：Context Mark 是“让 Context Planner 在后续每轮优先带上的持久书签”，只引用当前 Turn 实际读取过的 Canon；使用稳定字段/标题路径、锚点 Hash 和文档 Hash，不复制事实。完整语义、生命周期、失效处理和示例见 `agent-protocol-v2-context-mark-review.zh-CN.md`。

状态：**Revision 2 已接受**（2026-08-30）；RB-Q007 已关闭。

### RB-Q008——旧 AM1/AM2 状态

决定：把旧 AM1/AM2 的可复用安全能力直接合并进当前设计，不保留独立历史阶段、旧产品路径或历史状态。重基线同步时移除被替代的 Story Packet、旧协议和重复说明，最终只保留一套当前架构与 Plan。

状态：**已接受并修订**（2026-08-30）。AM-R1 Packet 已形成分阶段保留/删除清单；实际删除仍受调用图、等价安全回归和对应 Story Gate 约束。

## 11. Review Gate

本 Plan Review Gate GRB0：**通过**（2026-08-30）。

- RB-Q001～RB-Q008 与 AP-Q001～AP-Q010 全部获产品批准。
- Agent Protocol V2 已成为 GAR1 当前生产 Agent 单路径；旧产品入口已退役，复用代码仅作为未挂载安全内核保留。
- `agent-conversation-authoring-am-r1-story-packet.zh-CN.md` 的 NG-R101～NG-R106 已全部完成。
- GAR1 技术提交已生成，但 `gar1Approved=false`，不代表产品批准。
- Sprint 6、真实 Provider Live Smoke、AM-R2 与 AM-R3 继续未授权。

下一步且唯一允许动作是 GAR1 独立产品评审。不得把技术提交解释为 GAR1 产品批准或后续阶段实施授权。
