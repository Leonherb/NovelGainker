# NovelGainker NR-2～NR-6 能力交付总计划 Draft 2

2026-09-05 范围纠正：用户未提出云同步需求，现已从整个路线、阶段表、覆盖矩阵及待决定项删除；本计划止于NR-6 / NR-G7，不保留云同步待办。下方原阶段时点的授权说明为历史记录，现行进度见[sprint-plan](sprint-plan.zh-CN.md)。

> 文档状态：Approved / `NR2-D2-R01～R05` 全部通过  
> 形成日期：2026-09-01  
> 当前实现基线：NR-1  
> 当前已通过 Gate：NR-G2  
> 当前授权：仅形成并最终评审 `NR-2 Host Agent Loop / Tool Request-Result Story Packet Draft 2`  
> 明确未授权：`NR-201～NR-206` 实施、真实 Provider Live Smoke、形成 `NR-3～NR-6` Story Packet

2026-09-05 NR-6执行方式修订：R04多会话/历史操作/保守重试的产品行为已接受；NR-604/605由多位用户直接试用应用并提交自愿脱敏反馈，替代助手自动Live批次。见[多用户测试方案](testing/novelist-agent-runtime-nr6-multi-user-test-plan.zh-CN.md)。下方原真实验证的授权要求不构成助手调用Key或自动批跑许可；执行安排以现行联合Story和该测试方案为准，尚未发放/测试。原双Provider/长篇真实证据与NR-G7要求保留。

## 1. 计划结论

报告中“当前明确未实现的产品能力”不能作为一个大 Story 一次实现。它们包含 Agent 协议、Skill、Canon 读取、安全写入、章节生产、会话管理、Provider 恢复、真实模型验证等不同风险层级，必须按依赖关系逐层开放。

现行序列保持不变：

- 下一阶段为 `NR-2`，Story 从 `NR-201` 开始；
- `NR-2` 的退出 Gate 为 `NR-G3`；
- 后续依次为 `NR-3 / NR-301… / NR-G4`，不创建任何新前缀；
- `NR-2～NR-6` 构成 Local-first 核心产品路线；

核心依赖链为：

```text
NR-1 长对话底座（已完成，NR-G2 已通过）
  ↓
NR-2 Host Agent Loop / Tool Request-Result
  ↓ NR-G3
NR-3 常驻 Novelist Capability Pack / Canon Read Context
  ↓ NR-G4
NR-4 Canon Create-Update / Conflict / Undo
  ↓ NR-G5
NR-5 Chapter / Continuity / Publication Loop
  ↓ NR-G6
NR-6 Conversation Operations / Provider Resilience / Full Live Semantic Validation
  ↓ NR-G7
```

## 2. 跨阶段不可破坏边界

所有后续 Story Packet 都必须继承以下产品合同：

1. 中栏始终是自然语言 Conversation，不恢复 Anchor、Context Mark、关键词路由、任务卡流水线或用户可见 Control JSON。
2. 普通闲聊、小说讨论和任务发布共用同一对话入口；缺少角色、设定、章节或 Skill 结果不能阻止 AI 正常回复。
3. AI 与常驻 Novelist Capability Pack 负责语义判断；Host 不使用关键词判断意图、选择能力或决定什么内容值得写入 Canon。
4. Host 是唯一实际执行者，负责 Provider、Tool、文件、事务、凭据、持久化、恢复、取消、预算和安全校验。
5. 用户可见回复与可选任务执行拥有独立终态。Tool、Skill 或文件任务失败不能把已经正常完成的自然语言回复改成“本次回复未完成”。
6. Task 状态只在中栏当前 Turn 内以简短自然语言活动状态呈现，并在终态后收敛为结果；不增加第四栏、独立 Task Center、右侧 History 窗口、技术卡片或机器 JSON。
7. 小说 Markdown 与资产是唯一 Canon；History 是 Conversation 事实源；Memory 仍是可丢弃派生状态。
8. Novelist 必须作为一个随应用内置、版本固定、发布可验证的 Capability Pack；正式产品不得从外部插件缓存、用户 `SKILL.md`、Python Sidecar 或外部 Runtime Host 动态加载。
9. 所有 Canon 写入必须经过工程相对路径、Writer Lease、Source Hash、原子替换、恢复日志和稳定错误边界。
10. AI 的构思、假设和备选方案不能自动升格为 Canon；只有具备允许来源的确定信息才能自动请求写入，冲突、破坏性操作和不确定目标必须向用户说明并等待决策。
11. 每一阶段先形成并批准独立 Story Packet，连续实施授权只在该阶段内有效，并在对应技术 Gate 停止。

## 3. 分阶段 Story Plan

## 3.1 NR-2——Host Agent Loop 与独立 Tool 通道

### 阶段目标

在不加载 Novelist Capability Pack、不读取 Canon、不修改 Markdown 的前提下，建立 Provider、Agent Loop 与 Host Tool Executor 之间可验证的机器通道。证明 AI 可以在自然回复之外请求任务，Host 可以执行或拒绝任务，并且二者终态互不污染。

### Story

| Story    | 标题                                  | 最小交付                                                                                                                                             |
| -------- | ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `NR-201` | Tool Request/Result 合同              | 建立版本化、类型化、大小有界的 Tool Request、Tool Result、Task Status 和稳定错误合同；机器通道不进入可见 Conversation 文本                           |
| `NR-202` | 双 Provider Tool-Call 适配            | DeepSeek/OpenAI Adapter 保真转换工具声明、调用和结果；未知、畸形、越界请求在出站或执行前被拒绝                                                       |
| `NR-203` | Host Agent Loop 状态机                | 实现有界的“主回复/工具请求 → Host 执行 → 工具结果 → 后续回复”循环；首批只挂载合成的无副作用工具                                                      |
| `NR-204` | Conversation 与 Task 双终态及内联状态 | Conversation Turn 保存真实回复；Task 独立记录 queued/running/succeeded/failed/cancelled；中栏当前 Turn 只显示简短活动/结果，任务失败不生成假回复失败 |
| `NR-205` | 取消、幂等、恢复与预算                | 冻结单 Turn 工具轮数、累计 Provider 调用/输出、超时、取消、重复 Tool ID、重开恢复和窗口关闭语义                                                      |
| `NR-206` | NR-2 回归与 NR-G3 技术提交            | Loopback 覆盖双方请求体、非法协议零执行、回复/任务独立终态、三栏 UI 不扩张、零 Canon 访问、完整质量门和冻结证据                                      |

### NR-G3 必须证明

- 自然语言回复与机器 Tool 通道物理分离；
- 双 Provider 均能完成相同的有界 Tool Loop；
- Tool 失败、拒绝、取消或恢复不会抹除非空回复；
- 中栏只呈现简短自然语言活动/结果，左右栏职责不变，无新 Task/History 面板；
- 未加载 Skill、未读取或修改 Canon、未执行真实 Provider Live Smoke；
- NR-1 History/Memory/Context、停止与关窗行为无回归。

## 3.2 NR-3——常驻 Novelist Capability Pack 与 Canon Read Context

### 阶段目标

让用户始终与一个常驻 Novelist Agent 自然对话。批准的 `novelist 0.2.0` 九项 Skill 作为一个版本固定的内置 Capability Pack 随应用发布；模型在内部按语义使用相应能力，并通过 Host 的只读工具发现、读取和检查相关 Canon。此阶段只读，不允许 AI 写入 Markdown。

### 常驻 Agent 与能力包合同

- 每一轮都具有同一个 Novelist Agent 身份和公共写作合同，不先运行独立 Router。
- Provider 接收一个紧凑的内置能力目录；模型根据自然语言语义决定是否请求某项能力的完整指令或资源。
- Host 只验证请求的能力 ID、内置版本、Hash、资源边界和允许的 Tool，不按关键词替模型选择能力。
- 能力调用和加载过程不生成用户可见 Skill 卡、选择步骤、Control JSON 或新的对话流程。
- Capability Pack 是应用发布物的一部分；更新版本必须重新评审、重新冻结资源清单和回归证据。

### Story

| Story    | 标题                                      | 最小交付                                                                                                                                      |
| -------- | ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `NR-301` | 内置 Novelist Capability Pack             | 将批准的 `novelist 0.2.0` 九项 Skill 作为一个能力包打入应用，固定版本、资源清单、Hash、能力描述、许可证和发布校验，不依赖用户机器上的外部插件 |
| `NR-302` | 常驻 Novelist Agent 与内部语义调用        | 公共 Novelist 合同常驻；模型根据语义从内置能力目录请求所需指令/资源；禁止关键词 Router、前置 Skill Selection、强制任务卡和可见选择步骤        |
| `NR-303` | Canon Catalog 与只读 Tool                 | 为角色、世界、设定、素材、剧情、大纲和章节提供有界 list/search/read；Host 执行路径与大小安全校验                                              |
| `NR-304` | Canon Context 预算与来源                  | 把相关 Canon 以明确来源、优先级和预算组装进 Context；最近 Canon 真值优先于 Memory，不把整部小说无界塞入请求                                   |
| `NR-305` | 自然对话只读闭环                          | 小说讨论、无关闲聊、事实检查、续写准备和信息不足场景共用一条 Conversation；AI 可主动查询但不得因无 Canon 拒绝聊天                             |
| `NR-306` | NR-3 规模回归、Live 决策与 NR-G4 技术提交 | 验证能力包自包含、内部语义调用反例、Canon 检索相关性、长工程读取预算、零写入和完整离线质量门；记录进入 NR-4 前的最小 Live 兼容性决策          |

### NR-G4 必须证明

- 正式应用只使用内置、版本固定且完整性可验证的 Novelist Capability Pack；
- 用户始终面对同一个 Novelist Agent，不存在前置 Router 或用户可见 Skill 选择；
- 模型按语义决定能力与 Canon 读取，Host 不做关键词判断；
- Canon、History、Memory 的权威关系没有混淆；
- 空工程、缺失设定和无关闲聊仍可正常完成；
- 此阶段 Canon 写入次数严格为 0。

### 进入 NR-4 前的最小 Live 兼容性决策

NR-G4 技术提交仍可完全使用离线 Fixture 和 `127.0.0.1` Loopback。由于 NR-4 将首次开放 Canon 写入，在授权 NR-4 实施前，产品负责人必须另行作出以下二选一决策，沉默不视为授权：

1. 单独授权最小只读 Live Smoke：使用合成测试工程和专用凭据，分别验证 DeepSeek/OpenAI 的一次自然回复、一次 Tool Call/Result 和一次 Canon Read；不得写 Canon；或
2. 明确放弃该检查并接受真实 Provider Tool/Skill 兼容性尚未验证的风险。

当前没有任何 Live Smoke 授权。本决策点不增加新 Story 编号，也不阻止 NR-3 的离线实现与技术提交。

## 3.3 NR-4——Canon 安全创建、更新、冲突与撤销

### 阶段目标

把角色、世界、设定、素材和剧情等对话成果安全落实到 Markdown。AI/Capability Pack 判断需要创建或更新什么，Host 执行类型化 Canon 操作；不恢复 Anchor，也不要求每次更新都经过机械确认。

### Canon 自动维护来源规则

AI 可以进行语义判断，但自动写入必须声明可核对的来源引用：Conversation Message ID、用户明确任务 ID 或已提交 Chapter Hash。来源分为四类：

| 来源                                           | 默认处理                                                            |
| ---------------------------------------------- | ------------------------------------------------------------------- |
| 用户明确决定、纠正或发布的任务                 | 允许 AI 请求 Host 自动创建或更新相关 Canon                          |
| 已正式提交章节中明确出现且无歧义的新事实       | 允许同步维护相关 Canon；若涉及推断、范围不明或冲突则询问用户        |
| AI 自己提出的构思、假设、建议或备选方案        | 只保留在 Conversation，不自动升格为 Canon；用户接受后才获得允许来源 |
| 冲突、歧义目标、删除、合并、重命名或大范围覆盖 | 必须向用户说明影响并等待选择                                        |

Host 校验来源引用真实存在、目标类型、路径、Hash、事务和权限，但不替 AI 判断文本语义。自动执行结果在当前 Turn 内以简短自然语言呈现，并提供查看 Diff/Undo 的入口，不使用 Anchor 或逐项确认弹窗。

### Story

| Story    | 标题                           | 最小交付                                                                                                                            |
| -------- | ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| `NR-401` | Canon Mutation 与来源合同      | 定义 character/world/material/plot/outline 等类型化 create/update/upsert 请求、目标解析、来源类别/引用、预期 Source Hash 和执行结果 |
| `NR-402` | Canon Apply/Recovery 挂载      | 将现有安全 Kernel 挂载到生产 Tool Executor，支持安全新建与更新 Markdown；不开放任意路径、任意文件或 Shell 写入                      |
| `NR-403` | 语义 Upsert 与持续维护         | AI/Capability Pack 根据允许来源判断新实体、已有实体变化和关联更新；Host 先读取当前 Canon，再执行最小差异写入                        |
| `NR-404` | Diff、自动执行与确认策略       | 明确决定和无歧义章节事实可按策略自动执行；AI 建议不得自动写入；冲突、歧义和破坏性操作请求用户选择；提供 Diff/Undo 入口              |
| `NR-405` | 冲突检测、解决与 Undo          | 处理 Source Hash 冲突、设定矛盾、重复实体、部分失败、重放和撤销；冲突不由 Host 猜测语义结论                                         |
| `NR-406` | NR-4 事务回归与 NR-G5 技术提交 | 覆盖四类来源、创建、更新、并发修改、取消、失败恢复、Undo、Canon Tree 与零越界写入，形成完整 Gate 证据                               |

### NR-G5 必须证明

- 用户无需 Anchor 或关键词即可由 AI 语义触发 Canon 维护；
- 用户明确决定与正式章节事实可以自然落盘，AI 自己的提议不能静默成为 Canon；
- Host 是唯一写入者，AI 不能绕过来源引用、类型、路径、Hash、事务或恢复边界；
- 冲突时保留磁盘原文件、草稿和对话回复，并向用户提供可理解的选择；
- 自动更新可追踪、可解释、可撤销；
- 尚不把讨论稿误写为正式章节。

## 3.4 NR-5——章节提交、连续性与出版闭环

### 阶段目标

形成真正面向小说写作的非线性工作流：作者只需自然聊天，AI/Capability Pack 可以讨论、规划、写作、维护 Canon、将确认内容提交到正式章节，并在章节前后检查连续性和出版条件。

### Story

| Story    | 标题                            | 最小交付                                                                                                      |
| -------- | ------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `NR-501` | 正文状态与章节目标              | 区分聊天讨论、候选正文和正式章节目标；由语义和用户决策确定提交意图，不引入 Anchor 按钮                        |
| `NR-502` | Chapter Create/Update/Commit    | 通过 Novelist Capability Pack 与 Host Tool 安全创建或更新章节，保持模板、顺序、元数据、Source Hash 和原子恢复 |
| `NR-503` | 连续性读取与回写                | 在章节前后检查角色、时间线、世界规则、线索、MacGuffin 和剧情线程，并把确认变化回写相关 Canon                  |
| `NR-504` | 非线性多任务编排                | 同一自然 Conversation 可完成讨论、检索、角色更新、章节写作和后续修正；每项任务独立终态且整体调用预算有界      |
| `NR-505` | Review 与 Publication Loop      | 挂载批准的连续性审查、只读分析和 EPUB 发布能力；发布只消费合格 Canon，不把 Memory 或内部任务状态纳入成书      |
| `NR-506` | 端到端小说回归与 NR-G6 技术提交 | 以合成小说完成“讨论 → 角色/设定更新 → 章节提交 → 连续性检查 → 发布验证”闭环和失败反例                         |

### NR-G6 必须证明

- AI 能持续维护角色、世界观、设定和章节，而不是一次性生成孤立文件；
- 小说任务可以非线性执行，普通闲聊仍不会触发无关写入；
- 章节提交与普通讨论有清楚但不机械的语义边界；
- 连续性冲突不被静默覆盖；
- Publication 只使用 Canon 与批准资产，失败不会破坏源文件。

## 3.5 NR-6——会话操作、Provider 恢复与完整真实语义验证

### 阶段目标

在核心 Novelist 闭环成立后补齐长期使用能力，并通过单独授权的完整真实 Provider 验证评估语义质量。NR-3 的最小只读兼容性检查不能替代本阶段的完整 Tool、Write、Chapter 与长篇验证；本阶段也不能用自动重试掩盖协议、工具或模型问题。

### Story

| Story    | 标题                                | 最小交付                                                                                                                 |
| -------- | ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `NR-601` | 多 Conversation Session             | 同一工程支持多个命名 Session、稳定切换和独立 History/Memory；不增加右侧 History 窗口                                     |
| `NR-602` | History 分支、清空与导出            | 提供可恢复分支、明确清空和本地导出；操作不改变 Canon，且不会把内部 Tool 协议泄漏为用户对话                               |
| `NR-603` | Provider Cursor、续传与重试         | 在 Provider 能力允许时保存游标或重连续传；冻结幂等、重试资格、调用上限和“不得重复用户消息/Tool 副作用”规则               |
| `NR-604` | 完整真实 Provider Live Smoke        | 在产品负责人单独授权、使用测试工程和专用凭据时验证 DeepSeek/OpenAI 自然回复、Tool Call、Canon Read/Write、取消与错误映射 |
| `NR-605` | 长篇语义与规模评估                  | 对 Memory 摘要、Canon 检索、内部能力调用、冲突判断、章节连续性和几十万字工程进行机械与人工评分                           |
| `NR-606` | Release Hardening 与 NR-G7 技术提交 | 冻结支持矩阵、预算、恢复策略、质量证据和已知限制，完成 Local-first 核心产品 Gate                                         |

### NR-G7 必须证明

- 多 Session/History 操作不会破坏 Canon、Memory 绑定或现有单 Session 数据；
- Cursor、续传或重试不会重复回复、用户消息或已提交 Tool 副作用；
- 完整真实 Provider 验证必须可审计、可停止、无 Key 泄漏，并与离线 Gate 和 NR-3 最小只读检查分开记录；
- 几十万字规模不仅通过字节压力测试，还具备明确的语义质量结论；
- 若真实模型语义不达标，应回到对应 Story 修正，不以 NR-G7 通过掩盖。

## 4. 未实现能力覆盖矩阵

| 当前未实现能力                                      | 主要落点                                   | 完成标志                                                 |
| --------------------------------------------------- | ------------------------------------------ | -------------------------------------------------------- |
| 常驻 Novelist Capability Pack、版本和内部语义调用   | `NR-301～NR-302`                           | 内置、自包含、版本固定、同一 Agent、无关键词 Router      |
| Host Agent Loop、Tool Request/Result 和自主任务执行 | `NR-201～NR-205`，`NR-504`                 | 机器通道、Host 执行、任务独立终态、非线性多任务          |
| Canon 读取并组装进 Context                          | `NR-303～NR-305`                           | 有界检索、来源明确、Canon 优先于 Memory                  |
| 角色/世界/设定/素材/剧情/章节语义 Upsert            | `NR-401～NR-403`，`NR-502～NR-503`         | 允许来源、新建、持续更新、关联回写且无 Anchor            |
| AI Markdown、Diff、确认、Undo、冲突解决             | `NR-401～NR-405`                           | 来源约束、安全事务、风险分级、可查看、可撤销、冲突不覆盖 |
| 确认正文提交到正式章节                              | `NR-501～NR-502`                           | 讨论稿与正式章节边界清楚、提交可恢复                     |
| Task 与 Conversation 终态展示                       | `NR-204`，`NR-504`                         | 中栏内联活动/结果，回复不被任务失败污染，无新增面板      |
| 多 Session、History 分支/清空/导出                  | `NR-601～NR-602`                           | 工程内多会话与本地操作不改变 Canon                       |
| Provider Cursor、断点续传和自动重试                 | `NR-603`                                   | 有资格、有上限、幂等且无重复副作用                       |
| 真实模型兼容性与语义验证                            | `NR-306` 决策点，`NR-604～NR-605` 完整验证 | 最小只读检查与完整双 Provider/长篇评估分开授权、分开留证 |
| Canon Apply、连续性审查和出版 Kernel 挂载           | `NR-402`，`NR-503～NR-505`                 | 从安全写入到连续性与 EPUB 的完整闭环                     |

## 5. 实施与 Gate 规则

1. 本总计划通过只代表路线和编号获得认可，不代表任何阶段实现授权。
2. 每个阶段必须先形成独立 Story Packet Draft，经评审和产品负责人明确批准后才能实施。
3. 每次实施授权只覆盖该 Packet 内按编号连续执行的 Stories，并在对应技术提交停止。
4. 技术提交不自动等于 Gate 通过；产品负责人批准 Gate 后才能形成下一阶段 Packet。
5. `pnpm quality` 仍是每个实施 Story 和 Gate 的本地权威入口；新增 Tool/Skill/Canon IPC 后必须同步 Manifest、Handler、Permission、Capability 与全部生产 Client。
6. NR-2 和 NR-3 的实现及技术提交默认保持离线。进入 NR-4 前必须对“最小只读 Live Smoke”作出单独授权或明确放弃决定；NR-6 的完整 Live/语义验证仍需再次单独授权。任何计划批准都不自动产生网络或凭据授权。

## 6. Draft 2 评审结论落实

| 决策                            | Draft 2 结论                                                                                                              |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `NR2-D2-R01` 编号               | 接受 Draft 1，不修改：`NR-201` 起步，阶段 Gate 从 `NR-G3` 递增                                                            |
| `NR2-D2-R02` 阶段顺序与 UI      | 接受阶段顺序；Task 状态仅在中栏当前 Turn 内以简短自然语言内联呈现，不增加第四栏、Task Center、右侧 History 或技术卡片     |
| `NR2-D2-R03` Novelist 发布方式  | 修订为一个常驻、内置、版本固定的 Novelist Capability Pack；模型内部语义调用九项能力，Host 不建立关键词或前置 Skill Router |
| `NR2-D2-R04` Canon 自动维护     | 增加四类来源规则；用户明确决定与无歧义正式章节事实可自动请求写入，AI 建议不自动成为 Canon，冲突/破坏性操作询问用户        |
| `NR2-D2-R05` 真实 Provider 验证 | 改为分层验证：NR-3 后设置单独授权的最小只读兼容性决策；NR-6 保留完整 Tool/Write/Chapter/长篇语义验证                      |

## 7. 当前停止点与下一步

产品负责人已批准本 Draft 2，并确认 `NR2-D2-R01～R05` 全部通过。代码仍停止在已通过的 `NR-G2`；当前下一步仅为评审已经形成的：

> `NR-2 Host Agent Loop / Tool Request-Result Story Packet Draft 2`

在该 Packet 获得明确实施批准前，不修改生产代码、不进入 `NR-201～NR-206`、不执行真实 Provider Live Smoke，也不形成 `NR-3～NR-6` Story Packet。

## 8. BMAD Routing Brief

### Scope

- Packet type：`review-gate-packet`；
- Project type：Windows desktop novel-writing application；
- Project level：4；
- Runtime context：Codex；
- Confidence：高。

### Current phase

- Capability Delivery Plan Draft 2 已获批准，NR-2 Draft 1 评审结论已接受，当前为 NR-2 Story Packet Draft 2 最终评审；
- 代码仍停止在 NR-1 实现基线，没有进入 NR-2。

### Recommended next move

- 最终评审 `NR2-D2-R01～R05`；
- 未获另行批准前只停留在 Packet 评审，不进入实现。

### What not to do yet

- 不并行形成 NR-3～NR-6 Story Packet；
- 不把路线认可解释为实施授权；
- 不执行真实 Provider Live Smoke；
- 不恢复 Anchor、关键词 Router 或可见 Control JSON。
