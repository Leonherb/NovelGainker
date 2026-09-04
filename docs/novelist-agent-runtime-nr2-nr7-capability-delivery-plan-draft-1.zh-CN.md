# NovelGainker NR-2～NR-7 能力交付总计划 Draft 1

> 文档状态：Planning Draft / 待评审  
> 形成日期：2026-09-01  
> 当前实现基线：NR-1  
> 当前已通过 Gate：NR-G2  
> 本文授权范围：仅形成并评审后续能力计划  
> 明确未授权：任何 NR-2 及后续实现、真实 Provider Live Smoke

## 1. 计划结论

报告中“当前明确未实现的产品能力”不能作为一个大 Story 一次实现。它们包含 Agent 协议、Skill、Canon 读取、安全写入、章节生产、会话管理、Provider 恢复、真实模型验证和云同步等不同风险层级，必须按依赖关系逐层开放。

现行序列保持不变：

- 下一阶段为 `NR-2`，Story 从 `NR-201` 开始；
- `NR-2` 的退出 Gate 为 `NR-G3`；
- 后续依次为 `NR-3 / NR-301… / NR-G4`，不创建任何新前缀；
- `NR-2～NR-6` 构成 Local-first 核心产品路线；
- `NR-7` 仅作为 Cloud Sync 条件路线保留编号，必须另行完成产品决策后才能形成 Story Packet。

核心依赖链为：

```text
NR-1 长对话底座（已完成，NR-G2 已通过）
  ↓
NR-2 Host Agent Loop / Tool Request-Result
  ↓ NR-G3
NR-3 内置 Novelist Skill / Canon Read Context
  ↓ NR-G4
NR-4 Canon Create-Update / Conflict / Undo
  ↓ NR-G5
NR-5 Chapter / Continuity / Publication Loop
  ↓ NR-G6
NR-6 Conversation Operations / Provider Resilience / Live Semantic Validation
  ↓ NR-G7
NR-7 Cloud Sync（条件路线，不属于连续实施授权）
  ↓ NR-G8
```

## 2. 跨阶段不可破坏边界

所有后续 Story Packet 都必须继承以下产品合同：

1. 中栏始终是自然语言 Conversation，不恢复 Anchor、Context Mark、关键词路由、任务卡流水线或用户可见 Control JSON。
2. 普通闲聊、小说讨论和任务发布共用同一对话入口；缺少角色、设定、章节或 Skill 结果不能阻止 AI 正常回复。
3. AI 与内置 Novelist Skill 负责语义判断；Host 不使用关键词判断什么内容值得写入 Canon。
4. Host 是唯一实际执行者，负责 Provider、Tool、文件、事务、凭据、持久化、恢复、取消、预算和安全校验。
5. 用户可见回复与可选任务执行拥有独立终态。Tool、Skill 或文件任务失败不能把已经正常完成的自然语言回复改成“本次回复未完成”。
6. 小说 Markdown 与资产是唯一 Canon；History 是 Conversation 事实源；Memory 仍是可丢弃派生状态。
7. Skill 必须随应用内置、版本固定并受发布门校验；正式产品不得从外部插件缓存、用户 `SKILL.md`、Python Sidecar 或外部 Runtime Host 动态加载。
8. 所有 Canon 写入必须经过工程相对路径、Writer Lease、Source Hash、原子替换、恢复日志和稳定错误边界。
9. 冲突、破坏性操作和不确定目标必须向用户说明并等待决策；普通安全更新不要求用户机械 Anchor。
10. 每一阶段先形成并批准独立 Story Packet，连续实施授权只在该阶段内有效，并在对应技术 Gate 停止。

## 3. 分阶段 Story Plan

## 3.1 NR-2——Host Agent Loop 与独立 Tool 通道

### 阶段目标

在不加载 Novelist Skill、不读取 Canon、不修改 Markdown 的前提下，建立 Provider、Agent Loop 与 Host Tool Executor 之间可验证的机器通道。证明 AI 可以在自然回复之外请求任务，Host 可以执行或拒绝任务，并且二者终态互不污染。

### Story

| Story    | 标题                        | 最小交付                                                                                                                   |
| -------- | --------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `NR-201` | Tool Request/Result 合同    | 建立版本化、类型化、大小有界的 Tool Request、Tool Result、Task Status 和稳定错误合同；机器通道不进入可见 Conversation 文本 |
| `NR-202` | 双 Provider Tool-Call 适配  | DeepSeek/OpenAI Adapter 保真转换工具声明、调用和结果；未知、畸形、越界请求在出站或执行前被拒绝                             |
| `NR-203` | Host Agent Loop 状态机      | 实现有界的“主回复/工具请求 → Host 执行 → 工具结果 → 后续回复”循环；首批只挂载合成的无副作用工具                            |
| `NR-204` | Conversation 与 Task 双终态 | Conversation Turn 保存真实回复；Task 独立记录 queued/running/succeeded/failed/cancelled，任务失败不生成假回复失败          |
| `NR-205` | 取消、幂等、恢复与预算      | 冻结单 Turn 工具轮数、累计 Provider 调用/输出、超时、取消、重复 Tool ID、重开恢复和窗口关闭语义                            |
| `NR-206` | NR-2 回归与 NR-G3 技术提交  | Loopback 覆盖双方请求体、非法协议零执行、回复/任务独立终态、零 Canon 访问、完整质量门和冻结证据                            |

### NR-G3 必须证明

- 自然语言回复与机器 Tool 通道物理分离；
- 双 Provider 均能完成相同的有界 Tool Loop；
- Tool 失败、拒绝、取消或恢复不会抹除非空回复；
- 未加载 Skill、未读取或修改 Canon、未执行真实 Provider Live Smoke；
- NR-1 History/Memory/Context、停止与关窗行为无回归。

## 3.2 NR-3——内置 Novelist Skill 与 Canon Read Context

### 阶段目标

让 AI 在自然对话中按语义使用随应用发布的 Novelist 能力，并通过 Host 的只读工具发现、读取和检查相关 Canon。此阶段只读，不允许 AI 写入 Markdown。

### Story

| Story    | 标题                           | 最小交付                                                                                                          |
| -------- | ------------------------------ | ----------------------------------------------------------------------------------------------------------------- |
| `NR-301` | 内置 Novelist Skill Registry   | 将批准的 Novelist Skill 内容打包进应用，固定版本、资源清单、Hash、能力声明和发布校验，不依赖用户机器上的外部插件  |
| `NR-302` | 语义能力选择与指令组装         | AI 根据对话语义和 Skill 描述选择能力；禁止关键词路由、强制任务卡和可见 Skill 选择步骤                             |
| `NR-303` | Canon Catalog 与只读 Tool      | 为角色、世界、设定、素材、剧情、大纲和章节提供有界 list/search/read；Host 执行路径与大小安全校验                  |
| `NR-304` | Canon Context 预算与来源       | 把相关 Canon 以明确来源、优先级和预算组装进 Context；最近 Canon 真值优先于 Memory，不把整部小说无界塞入请求       |
| `NR-305` | 自然对话只读闭环               | 小说讨论、无关闲聊、事实检查、续写准备和信息不足场景共用一条 Conversation；AI 可主动查询但不得因无 Canon 拒绝聊天 |
| `NR-306` | NR-3 规模回归与 NR-G4 技术提交 | 验证 Skill 发布自包含、语义选择反例、Canon 检索相关性、长工程读取预算、零写入和完整质量门                         |

### NR-G4 必须证明

- 正式应用只使用内置且版本固定的 Novelist Skill；
- AI/Skill 按语义决定是否读取 Canon，Host 不做关键词判断；
- Canon、History、Memory 的权威关系没有混淆；
- 空工程、缺失设定和无关闲聊仍可正常完成；
- 此阶段 Canon 写入次数严格为 0。

## 3.3 NR-4——Canon 安全创建、更新、冲突与撤销

### 阶段目标

把角色、世界、设定、素材和剧情等对话成果安全落实到 Markdown。AI/Skill 判断需要创建或更新什么，Host 执行类型化 Canon 操作；不恢复 Anchor，也不要求每次更新都经过机械确认。

### Story

| Story    | 标题                           | 最小交付                                                                                                             |
| -------- | ------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| `NR-401` | Canon Mutation 合同            | 定义 character/world/material/plot/outline 等类型化 create/update/upsert 请求、目标解析、预期 Source Hash 和执行结果 |
| `NR-402` | Canon Apply/Recovery 挂载      | 将现有安全 Kernel 挂载到生产 Tool Executor，支持安全新建与更新 Markdown；不开放任意路径、任意文件或 Shell 写入       |
| `NR-403` | 语义 Upsert 与持续维护         | AI/Skill 从自然对话判断新实体、已有实体变化和关联更新，Host 先读取当前 Canon 再执行最小差异写入                      |
| `NR-404` | Diff、自动执行与确认策略       | 普通可逆更新允许按批准策略执行并展示结果；歧义目标、破坏性变更和高风险覆盖必须请求用户选择；保留可查看 Diff          |
| `NR-405` | 冲突检测、解决与 Undo          | 处理 Source Hash 冲突、设定矛盾、重复实体、部分失败、重放和撤销；冲突不由 Host 猜测语义结论                          |
| `NR-406` | NR-4 事务回归与 NR-G5 技术提交 | 覆盖创建、更新、并发修改、取消、失败恢复、Undo、Canon Tree 与零越界写入，形成完整 Gate 证据                          |

### NR-G5 必须证明

- 用户无需 Anchor 或关键词即可由 AI 语义触发 Canon 维护；
- Host 是唯一写入者，AI 不能绕过类型、路径、Hash、事务或恢复边界；
- 冲突时保留磁盘原文件、草稿和对话回复，并向用户提供可理解的选择；
- 自动更新可追踪、可解释、可撤销；
- 尚不把讨论稿误写为正式章节。

## 3.4 NR-5——章节提交、连续性与出版闭环

### 阶段目标

形成真正面向小说写作的非线性工作流：作者只需自然聊天，AI/Skill 可以讨论、规划、写作、维护 Canon、将确认内容提交到正式章节，并在章节前后检查连续性和出版条件。

### Story

| Story    | 标题                            | 最小交付                                                                                                 |
| -------- | ------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `NR-501` | 正文状态与章节目标              | 区分聊天讨论、候选正文和正式章节目标；由语义和用户决策确定提交意图，不引入 Anchor 按钮                   |
| `NR-502` | Chapter Create/Update/Commit    | 通过 Novelist Skill 与 Host Tool 安全创建或更新章节，保持模板、顺序、元数据、Source Hash 和原子恢复      |
| `NR-503` | 连续性读取与回写                | 在章节前后检查角色、时间线、世界规则、线索、MacGuffin 和剧情线程，并把确认变化回写相关 Canon             |
| `NR-504` | 非线性多任务编排                | 同一自然 Conversation 可完成讨论、检索、角色更新、章节写作和后续修正；每项任务独立终态且整体调用预算有界 |
| `NR-505` | Review 与 Publication Loop      | 挂载批准的连续性审查、只读分析和 EPUB 发布能力；发布只消费合格 Canon，不把 Memory 或内部任务状态纳入成书 |
| `NR-506` | 端到端小说回归与 NR-G6 技术提交 | 以合成小说完成“讨论 → 角色/设定更新 → 章节提交 → 连续性检查 → 发布验证”闭环和失败反例                    |

### NR-G6 必须证明

- AI 能持续维护角色、世界观、设定和章节，而不是一次性生成孤立文件；
- 小说任务可以非线性执行，普通闲聊仍不会触发无关写入；
- 章节提交与普通讨论有清楚但不机械的语义边界；
- 连续性冲突不被静默覆盖；
- Publication 只使用 Canon 与批准资产，失败不会破坏源文件。

## 3.5 NR-6——会话操作、Provider 恢复与真实语义验证

### 阶段目标

在核心 Novelist 闭环成立后补齐长期使用能力，并首次通过单独授权的真实 Provider 验证评估语义质量。此阶段不能用自动重试掩盖协议、工具或模型问题。

### Story

| Story    | 标题                                | 最小交付                                                                                                   |
| -------- | ----------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `NR-601` | 多 Conversation Session             | 同一工程支持多个命名 Session、稳定切换和独立 History/Memory；不增加右侧 History 窗口                       |
| `NR-602` | History 分支、清空与导出            | 提供可恢复分支、明确清空和本地导出；操作不改变 Canon，且不会把内部 Tool 协议泄漏为用户对话                 |
| `NR-603` | Provider Cursor、续传与重试         | 在 Provider 能力允许时保存游标或重连续传；冻结幂等、重试资格、调用上限和“不得重复用户消息/Tool 副作用”规则 |
| `NR-604` | 真实 Provider Live Smoke            | 在产品负责人单独授权、使用测试工程和专用凭据时验证 DeepSeek/OpenAI 自然回复、Tool Call、取消与错误映射     |
| `NR-605` | 长篇语义与规模评估                  | 对 Memory 摘要、Canon 检索、Skill 选择、冲突判断、章节连续性和几十万字工程进行机械与人工评分               |
| `NR-606` | Release Hardening 与 NR-G7 技术提交 | 冻结支持矩阵、预算、恢复策略、质量证据和已知限制，完成 Local-first 核心产品 Gate                           |

### NR-G7 必须证明

- 多 Session/History 操作不会破坏 Canon、Memory 绑定或现有单 Session 数据；
- Cursor、续传或重试不会重复回复、用户消息或已提交 Tool 副作用；
- 真实 Provider 验证必须可审计、可停止、无 Key 泄漏，并与离线 Gate 分开记录；
- 几十万字规模不仅通过字节压力测试，还具备明确的语义质量结论；
- 若真实模型语义不达标，应回到对应 Story 修正，不以 NR-G7 通过掩盖。

## 3.6 NR-7——Cloud Sync 条件路线

### 阶段定位

Cloud Sync 不是当前 Local-first 小说 Agent 闭环的依赖项，而且会引入账号、远端服务、加密、设备身份、数据驻留和多写者冲突。它不能混入 NR-2～NR-6 的连续实施授权。

只有在 NR-G7 通过并且产品负责人单独批准 Cloud Sync 产品边界后，才允许形成 NR-7 Story Packet。以下编号只用于预留，不代表需求已经冻结：

| Story    | 候选标题                   | 必须先回答的问题                                                  |
| -------- | -------------------------- | ----------------------------------------------------------------- |
| `NR-701` | Sync 产品合同与威胁模型    | 同步哪些数据、谁拥有远端数据、是否同步凭据、如何删除与导出        |
| `NR-702` | 账号、设备与工程身份       | 无账号模式、设备授权、Project ID 和多设备身份如何保持 Local-first |
| `NR-703` | 端到端加密与传输           | 密钥归属、服务端可见性、传输/静态加密和数据驻留                   |
| `NR-704` | Canon/History 冲突合并     | 多 Writer、离线修改、章节冲突、History/Memory 派生状态如何处理    |
| `NR-705` | 离线队列、恢复与可观测性   | 断网、部分上传、重放、撤销、配额和稳定错误语义                    |
| `NR-706` | Sync E2E 与 NR-G8 技术提交 | 多设备合成回归、安全审计、删除证明和故障恢复                      |

NR-7 未获批准时，产品继续保持完全 Local-first；本地 History 导出不等于 Cloud Sync。

## 4. 未实现能力覆盖矩阵

| 当前未实现能力                                      | 主要落点                           | 完成标志                                           |
| --------------------------------------------------- | ---------------------------------- | -------------------------------------------------- |
| Novelist Skill Registry、加载、版本和语义选择       | `NR-301～NR-302`                   | 内置、自包含、版本固定、语义选择且无关键词路由     |
| Host Agent Loop、Tool Request/Result 和自主任务执行 | `NR-201～NR-205`，`NR-504`         | 机器通道、Host 执行、任务独立终态、非线性多任务    |
| Canon 读取并组装进 Context                          | `NR-303～NR-305`                   | 有界检索、来源明确、Canon 优先于 Memory            |
| 角色/世界/设定/素材/剧情/章节语义 Upsert            | `NR-401～NR-403`，`NR-502～NR-503` | 新建、持续更新、关联回写且无 Anchor                |
| AI Markdown、Diff、确认、Undo、冲突解决             | `NR-401～NR-405`                   | 安全事务、风险分级、可查看、可撤销、冲突不覆盖     |
| 确认正文提交到正式章节                              | `NR-501～NR-502`                   | 讨论稿与正式章节边界清楚、提交可恢复               |
| Task 与 Conversation 终态展示                       | `NR-204`，`NR-504`                 | 回复不被任务失败污染，每个任务可追踪               |
| 多 Session、History 分支/清空/导出                  | `NR-601～NR-602`                   | 工程内多会话与本地操作不改变 Canon                 |
| Cloud Sync                                          | 条件 `NR-701～NR-706`              | 仅在独立产品、安全和服务端决策后实施               |
| Provider Cursor、断点续传和自动重试                 | `NR-603`                           | 有资格、有上限、幂等且无重复副作用                 |
| 真实模型语义验证                                    | `NR-604～NR-605`                   | 双 Provider Live Smoke 与长篇人工/机械评估分开留证 |
| Canon Apply、连续性审查和出版 Kernel 挂载           | `NR-402`，`NR-503～NR-505`         | 从安全写入到连续性与 EPUB 的完整闭环               |

## 5. 实施与 Gate 规则

1. 本总计划通过只代表路线和编号获得认可，不代表任何阶段实现授权。
2. 每个阶段必须先形成独立 Story Packet Draft，经评审和产品负责人明确批准后才能实施。
3. 每次实施授权只覆盖该 Packet 内按编号连续执行的 Stories，并在对应技术提交停止。
4. 技术提交不自动等于 Gate 通过；产品负责人批准 Gate 后才能形成下一阶段 Packet。
5. `pnpm quality` 仍是每个实施 Story 和 Gate 的本地权威入口；新增 Tool/Skill/Canon IPC 后必须同步 Manifest、Handler、Permission、Capability 与全部生产 Client。
6. 真实 Provider Live Smoke 只属于 `NR-604`，且仍需单独授权；此前所有阶段使用离线 Fixture 和 `127.0.0.1` Loopback。
7. NR-7 不属于 NR-2～NR-6 的连续路线授权，不能因为核心路线通过而自动进入。

## 6. Draft 1 待评审决策

| 决策                            | Draft 1 建议                                                                                                 |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `NR2-D1-R01` 编号               | 沿用 `NR`：`NR-201` 起步，阶段 Gate 从 `NR-G3` 递增                                                          |
| `NR2-D1-R02` 阶段顺序           | Tool 通道 → Skill/Canon Read → Canon Write → Chapter/Continuity/Publish → Session/Resilience/Live Validation |
| `NR2-D1-R03` Skill 发布方式     | 把批准的 Novelist 能力内置并锁版本，不读取外部插件缓存或用户 Skill 文件                                      |
| `NR2-D1-R04` Canon 自动维护     | AI/Skill 语义判断；Host 执行。普通可逆更新可按策略执行，高风险/冲突项询问用户；不恢复 Anchor                 |
| `NR2-D1-R05` 真实 Provider 验证 | 推迟到核心闭环后的 NR-6，并保持独立授权和专用测试数据                                                        |
| `NR2-D1-R06` Cloud Sync         | 隔离为条件 NR-7，不耦合 Local-first 核心路线                                                                 |

## 7. 当前停止点与下一步

当前仍停止在已通过的 `NR-G2`。本 Draft 1 的下一步只有一项：**评审 `NR2-D1-R01～R06` 以及 Story 覆盖是否完整**。

在总计划获批后，最小下一 Artifact 才是：

> `NR-2 Host Agent Loop / Tool Request-Result Story Packet Draft 1`

在该 Packet 获得明确实施批准前，不修改生产代码、不进入 `NR-201`、不执行真实 Provider Live Smoke。

## 8. BMAD Routing Brief

### Scope

- Packet type：`brownfield-resume-packet`；
- Project type：Windows desktop novel-writing application；
- Project level：4；
- Runtime context：Codex；
- Confidence：高。

### Current phase

- 当前为已通过 `NR-G2` 后的后续规划评审；
- 代码仍停止在 NR-1 实现基线，没有进入 NR-2。

### Recommended next move

- 评审本 Capability Delivery Plan Draft 1；
- 获批后只形成 NR-2 Story Packet Draft 1。

### What not to do yet

- 不并行形成 NR-3～NR-7 Story Packet；
- 不把路线认可解释为实施授权；
- 不执行真实 Provider Live Smoke；
- 不恢复 Anchor、关键词路由或可见 Control JSON。
