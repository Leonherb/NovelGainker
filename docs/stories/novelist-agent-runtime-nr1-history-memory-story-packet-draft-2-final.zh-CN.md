# NovelGainker NR-1 Story Packet Draft 2 Final

## History / Memory / Compaction / Long-Conversation Context

> Packet：`NR-1-D2-FINAL`  
> 状态：`final`，等待产品批准  
> 上游 Gate：`NR-G1` 已于 2026-09-01 获产品批准  
> 取代：`NR-1-D2`  
> 实施授权：`false`  
> 真实 Provider Live Smoke：`false`

## 1. Draft 2 Final 结论

NR-1 在 NR-G1 的自然对话基线上，只解决完整对话历史、派生 Memory、按阈值压缩和长对话 Context。用户仍只在中栏自然聊天，不需要操作 Anchor、任务卡、压缩按钮或独立 History 窗口。

Draft 2 已关闭 Draft 1 的四项设计决策：

1. 压缩发生在主回复之前，但一次用户发送最多调用一次 Compactor；压缩的普通失败不能让主回复失败。
2. Memory 使用有固定栏目、保留认知状态的自然语言摘要，但不是 Canon，也不进入可见 Conversation。
3. 现在冻结调用数和零重试等结构边界；具体阈值与输出预算由 NR-106 的离线规模证据选定，并在 NR-G2 冻结。
4. Memory 无效时优先恢复最近上下文；此后只借用正常触发的压缩调用，向新消息衔接并逐步把更早历史纳入同一个连续覆盖区间。

Draft 2 Final 只补充最终评审发现的三个实施边界：

- `FR01`：一次 Provider 调用等于一次真实 outbound HTTP attempt；Compactor 与主回复分别使用可中断、零重试路径。
- `FR02`：Memory 使用可表达消息局部覆盖的确定性 Segment Cursor，并保证旧历史不会因前向增量长期饥饿。
- `FR03`：累计输出上限约束两次请求声明的输出预算总和；Usage 只用于观测，响应字节上限继续独立生效。

除此以外，Draft 2 的产品范围、Story 顺序和授权边界保持不变。本 Packet 不授权实现 NR-101～NR-106，不授权真实 Provider Live Smoke，也不进入 NR-2。

## 2. 已冻结的产品边界

- 用户只需自然聊天；不使用 Anchor、Context Mark、任务卡、关键词 Router 或线性工作流。
- 用户可见回复只有自然语言；内部 Memory 和 Compactor 结果不生成 Conversation 气泡。
- Host 负责 Provider 调用、History/Memory 持久化、阈值与预算、Context 组装、取消、恢复和安全边界。
- AI 负责 Memory 摘要的语义判断；Host 不用关键词判断哪些内容重要，也不把推测提升为用户决定。
- Conversation History 随工程完整保存并显示在中栏；不增加独立 History 窗口。
- 用户消息和一个 `running` 助手状态在任何本轮 Provider 调用前落盘。
- 仅未保存的手动 Markdown 可以阻断窗口关闭；用户确认关闭后取消本轮活动并直接退出。
- NR-1 不加载 Novelist Skill、不读取 Canon、不调用 Tool、不创建或更新小说 Markdown。
- 真实 Provider Live Smoke 始终需要单独授权，不能从 Packet 批准或实施授权推导。

## 3. History、Memory 与 Context

### 3.1 History 是完整事实源

History 保存在 `.novelgainker/conversation/session.json`，是用户可见 Conversation 的事实源。压缩不能删除、替换、折叠或改写其中任何旧消息。

每个 Turn 的用户消息、助手实际产生的非空文本和终态均按 NR-G1 规则保留。`running` 只表示活动状态，不是可压缩终态。

### 3.2 Memory 是可丢弃的派生状态

Memory 建议独立保存到 `.novelgainker/memory/state.json`。它用于延续对话，不是角色卡、世界观、设定、章节或任何 Canon 真值。

最小合同包含：

- `schemaVersion`
- `projectId`
- `conversationSessionId`
- `summaryText`
- `coveredFromCursor`
- `coveredThroughCursor`
- 覆盖区间的 `sourceHash`
- `updatedAtMs`

Cursor 是按 History 顺序稳定定位内容的确定性位置，最少包含 `turnId + messageId + UTF-8 byte offset`，也可以使用能证明同等顺序与边界的不可歧义 Segment ID。Host 在 UTF-8 字符边界上把持久化消息映射为有界 Context Segment；分段只影响 Provider 输入，不拆分、改写或重存用户可见 History。

`coveredFromCursor..coveredThroughCursor` 是一个包含首尾的连续内容区间，可以表达一个超长消息只被处理到某个安全片段的位置。覆盖校验必须绑定该区间的规范化 History 源字节；不能只保存 Turn 级“已覆盖到哪里”，否则既无法表达近期优先重建，也无法在单消息大于 Provider 边界时无损推进。

Memory 不得保存 API Key、隐藏 Prompt、原始 Provider 响应、绝对路径、Skill ID、Tool Request、Canon 路径或未来 Workflow 状态。Memory 缺失、损坏或过期时可以直接丢弃并重建，History 不受影响。

### 3.3 Context 是临时请求输入

每次主回复前，Host 按以下优先级组装 Context：

1. 固定系统指令；
2. 当前有效 Memory；
3. Memory 之后、受保护的最近精确 History；
4. 当前用户消息。

当前用户消息必须保留且只出现一次。最近精确 History 与 Memory 冲突时，最近精确 History 优先。Memory 覆盖区间之前、尚未被重建纳入的旧 History 仍完整保存在工程中并可见，但可暂时不进入本次模型 Context。

## 4. Memory 的语义合同

Compactor 只输出纯自然语言，不输出 JSON、Control JSON、工具请求或文件写入声明。摘要采用固定栏目，以便保留对话连续性和认知状态：

```text
已确认决定
正在创作或讨论
未决问题
用户明确任务
已产生但未完成的内容
```

语义规则：

- 只有用户已经确认的内容才能进入“已确认决定”。
- AI 的建议、猜测和候选方案必须保持“暂定”或“未决”，不能提升为用户决定。
- 用户明确发布但尚未完成的工作进入“用户明确任务”。
- 已输出部分文本但 Turn 失败或取消时，只记录真实产生的内容及其未完成状态。
- `not_generated` 的助手占位文案不作为语义内容；对应用户请求仍可保留为未完成任务。
- 摘要必须保留 `已确认 / 暂定 / 未决 / 任务 / 未完成` 的认知状态，不得把不同状态揉成无条件事实。
- 摘要不得声称任何内容已经写入角色、世界观、设定或章节 Markdown。
- 最近精确 History 是冲突时的更高优先级证据。

AI 决定栏目内的语义取舍；Host 只提供固定 Prompt、输入范围、大小边界、输出校验和持久化。角色、世界观与 Canon 的判断和维护留给 NR-3/NR-4 的 Novelist Skill 与 Host Tool 通道。

## 5. 单次用户发送的状态机

### 5.1 调用前持久化

Host 接受用户发送后，必须先原子保存：

- 当前用户消息；
- 与本轮绑定的助手 `running` 状态。

完成该持久化后，Host 才能估算 Context、调用 Compactor 或调用主回复 Provider。这样即使窗口突然关闭，History 仍至少能恢复用户请求，并按 NR-G1 规则把遗留 `running` 对账为未生成。

### 5.2 阈值以下

若预测 Context 低于压缩阈值：

- Compactor 调用为 0；
- Host 直接组装 Context；
- 主回复 Provider 调用最多 1 次。

### 5.3 达到阈值

若预测 Context 达到阈值：

1. Host 最多发起 1 次 Compactor 调用；
2. 输出非空、未越界且校验通过时，Host 原子替换 Memory；
3. 普通 Compactor 失败时，Host 保留最后一个有效 Memory，使用可容纳的最近精确 History，继续发起最多 1 次主回复；
4. 不重试 Compactor，不调用修复模型，不启动后台或并行压缩。

本 Packet 中一次 Provider 调用严格等于一次真实 outbound HTTP attempt，而不是一次可能包含内部重试的 Adapter 逻辑调用。Compactor 必须走 `complete-once-cancellable` 等价路径，主回复必须走 `stream-once-cancellable` 等价路径；两条路径都只允许一次网络 attempt，并能由同一个 Turn Cancellation Token 中断正在等待的请求。现有带重试接口只能用于不属于 Conversation Turn 的连接测试等边界，不能被 NR-1 Turn 调用。

Compactor 的普通失败不创建 Conversation 失败气泡，也不把助手状态标记为失败。除用户主动停止或已确认关窗触发既有取消/未生成终态外，只有主回复的实际结果决定用户可见的助手终态。

### 5.4 取消与关闭

- 用户在 Compactor 阶段点击停止：立即触发共享 Cancellation Token，中断正在进行的 HTTP attempt，取消整个 Turn，不再发起主回复；不能等到请求超时后才结束“停止中”。
- 用户确认关闭窗口：取消正在进行的 Compactor 或主回复，不等待其完成，然后按 NR-G1 关闭语义持久化终态并退出。
- 取消或退出期间产生的 Compactor 半成品不得提交；最后一个有效 Memory 保持不变。
- 尚未保存的手动 Markdown 仍显示“保存 / 放弃 / 取消”确认；除此之外不增加关闭阻断。

## 6. 近期优先、向旧历史渐进重建

### 6.1 无效 Memory 的判定

以下任一情况使 Memory 无效：文件缺失或损坏、Project/Session 不匹配、覆盖区间不存在于 History、`sourceHash` 不一致或合同校验失败。

无效 Memory 不触发弹窗、迁移向导或同步修复，不阻断当前聊天，也不触碰小说 Markdown。Host 丢弃它；当前回复先使用预算允许的最近精确 History。

### 6.2 首次重建

Host 不为了“修 Memory”单独增加 Provider 调用。只有正常 Context 压力达到阈值时，才使用本轮唯一一次 Compactor 调用。

第一次可执行重建时，Compactor 摘要的是“受保护最近精确尾部”之前紧邻的一组有界 Context Segment。成功后 Memory 覆盖一个连续内容区间：

```text
更早、暂未纳入的 History | Memory [coveredFromCursor..coveredThroughCursor] | 受保护最近精确尾部 | 当前消息
```

这保证用户刚刚讨论的内容立即以精确文本参与回复；更早 History 仍完整持久化和显示，只是暂时未进入模型 Context。

### 6.3 后续正常压缩

后续不为了追赶旧 History 额外调用 Compactor。只有再次达到正常压缩阈值时，才在本轮唯一一次 Compactor 调用内按顺序处理。

当 `coveredFromCursor` 之前仍有旧历史积压时，候选 Context Profile 必须预留一个大于零的 Backfill 输入预算；触发阈值必须保证“旧摘要 + 正常前向增量 + 至少一个旧历史 Segment”能够同时进入一次 Compactor 请求。Context Segment 必须足够小，使单个 Segment 不会单独突破该预留或 Provider 单消息边界。

1. **前向衔接优先**：先把 `coveredThroughCursor` 之后、已离开受保护精确尾部的新可压缩 Segment 纳入摘要，保证 Memory 与最近精确尾部之间不产生缺口。
2. **向旧历史保证推进**：只要 `coveredFromCursor` 之前仍有积压，每次成功的正常压缩必须从它之前紧邻地纳入至少一个旧历史 Segment，使 `coveredFromCursor` 严格向更早位置移动。
3. **按时间顺序输入**：Compactor 输入按“更早 Backfill Segment → 旧 Memory → 新的前向 Segment”排列，不能按处理优先级打乱叙事时间。
4. **单一区间提交**：输出仍是一个摘要，并把覆盖区间原子更新为 `[newCoveredFromCursor..newCoveredThroughCursor]`。

任何一次扩展都不得制造覆盖缺口。当前向边界到达最新可压缩内容、向后边界到达最早保留内容时，Memory 恢复为普通的连续旧前缀摘要。如果运行时无法同时满足前向连续性与 Backfill 最小进度，本次 Memory 不提交并按 Compactor 普通失败回退；NR-106 必须用候选阈值和分段上限证明正常 Profile 不会长期进入该状态。

Compactor 失败或被取消时，旧的有效摘要、覆盖区间和 `sourceHash` 全部保持不变；主回复是否继续按第 5 节执行。

## 7. 调用与预算边界

现在冻结的结构边界如下。这里的“调用”均指真实 outbound HTTP attempt：

- 主回复调用：每次用户发送最多 1 次可中断、零重试的 HTTP attempt；
- Compactor 调用：达到阈值且存在可压缩输入时最多 1 次可中断、零重试的 HTTP attempt，否则 0 次；
- Provider 累计调用：每次用户发送最多 2 次真实 HTTP attempts；
- Conversation Turn 内自动重试、模型修复调用、后台或并行压缩：0 次；
- Skill Calls：0；
- Tool Calls：0；
- Canon Operations：0。

以下数字不在 Draft 2 预先拍定：

- 压缩触发阈值；
- Memory 摘要大小上限；
- 受保护精确尾部大小；
- 单次 Compactor 输出上限；
- 单次用户发送累计请求输出 Token 上限；
- 旧历史积压期间的 Backfill Segment 与输入预留量。

每个候选 Profile 必须显式提供 `compactorRequestedMax`、`mainReplyRequestedMax` 与 `cumulativeRequestedOutputMax`，并满足：

```text
compactorRequestedMax + mainReplyRequestedMax
<= cumulativeRequestedOutputMax
```

该累计边界约束两次请求声明的最大输出预算，不依赖 Provider 是否返回 Usage，也不根据 Compactor 实际提前结束动态放大主回复预算。Provider Usage 只用于观测和证据；实际响应的字节数仍由独立响应字节上限保护。

NR-106 可在测试中注入若干内部候选 Profile，但它们不是用户设置、产品 Feature Flag 或按 Provider 分叉的 UI。候选选择必须满足：

- 所有构造出的 Provider 请求都在现有 64 条消息、96 KiB 请求和单消息硬边界内；
- 当前用户消息、系统指令和受保护精确尾部有确定的保留空间；
- 多次压缩后摘要仍被大小上限约束；
- 每次用户发送的调用数和累计输出都被本地边界约束；
- 10,000 条可见消息下不需要把全量 History 重复送给 Provider；
- 缺少 Provider Usage 时，Host 仍能用两次请求声明预算之和与独立响应字节上限保证机械边界。

最终参数由 NR-106 的机器规模证据选出，并在 NR-G2 Manifest / Report 冻结。离线 Loopback 只能证明状态机、请求边界、预算和持久化机制，不能证明真实 Provider 的摘要语义质量；在未授权 Live Smoke 的前提下，NR-G2 不得声称已经验证真实 Provider 摘要质量。

## 8. Conversation 连续性

- 普通闲聊、小说讨论、构思和直接创作继续使用同一 Conversation 路径。
- 缺少角色卡、世界观、章节、Memory 或有效摘要都不能阻断回复。
- 用户在失败或取消后说“继续”，Context 必须包含上一条用户请求和已有的非空部分回复；零文本未生成时至少包含上一条用户请求。
- Compactor 普通失败不显示“本次回复未完成”；只有主回复未形成正常终态时才沿用 NR-G1 的可见终态语义。
- Provider 切换不清除工程 History 或有效 Memory；新 Turn 使用当前 Provider 和同一组 Context/预算规则。
- 不新增专用 Continue 状态机、History 窗口、Memory 编辑器、重建按钮或修复流程。

## 9. Story 顺序

### NR-101 — History / Memory 合同与派生 Store

交付：

- 建立严格、工程绑定的 Memory Store；
- 实现 Segment Cursor 覆盖区间、规范化源字节 `sourceHash`、原子替换、恢复和校验；
- 保持完整 History 为事实源；
- Memory 无效时安全退化，不迁移旧 AM/Anchor/Workflow 数据。

验收：

- Memory 写入不改变 Conversation 或小说 Markdown/资产；
- Store 不含 Key、隐藏 Prompt、原始 Provider 响应、绝对路径、Skill/Tool/Canon 字段；
- 缺失、损坏、错 Project/Session、非法区间和 Hash 失败均不阻断聊天；
- 已提交 Memory 始终表示一个无缺口的连续内容区间，并能表达消息内 UTF-8 安全片段边界。

### NR-102 — 增量覆盖、阈值与近期优先重建

交付：

- 计算已终态 Turn、受保护精确尾部、Memory 前后未覆盖范围和正常触发阈值；
- 实现首次近期重建，以及“前向衔接优先、Backfill 保留、旧历史保证推进”的单区间算法；
- 明确 completed / failed / cancelled / not_generated 的摘要输入规则。

验收：

- 阈值以下 Compactor 调用为 0；达到阈值且有输入时最多 1；
- 重建不产生额外后台调用，覆盖区间不产生缺口；旧历史积压时，每次成功的正常压缩至少向后推进一个 Segment；
- `running`、占位提示和 UI 附加文案不污染摘要；
- 不使用关键词、正则或小说类型规则触发压缩。

### NR-103 — 有界 Compactor Provider 模式

交付：

- 在现有 Rust Provider Adapter 内增加仅返回纯文本摘要的内部 Compactor 模式；
- 使用当前 Provider 和 CredentialVault，不新增 Credential IPC；提供可中断、单 HTTP attempt、零重试的 Compactor 路径；
- 固定五个自然语言栏目并保留认知状态；
- 校验成功才原子提交 Memory。

验收：

- 不产生可见 Conversation 气泡、Control JSON、Tool 请求、自动重试或模型修复；
- 普通失败保留旧 Memory 并允许主回复继续；停止可中断正在进行的 Compactor HTTP attempt；
- 自动化只使用本地 Loopback，不读取真实 Key、不访问外网；
- 摘要不把 AI 建议升级为用户决定，也不声称已写入 Markdown。

### NR-104 — Context Assembler 与主回复回退

交付：

- 先持久化用户消息与 `running`，再执行本轮调用；
- 按固定优先级组装系统指令、有效 Memory、最近精确 History 和当前消息；主回复使用可中断、单 HTTP attempt、零重试路径；
- Compactor 普通失败时使用旧 Memory 与可容纳的最近 History 继续主回复；
- 保持现有流式、终态持久化、Store 对账和未生成/未完成语义。

验收：

- 当前用户消息恰好出现一次，所有输入保持在硬边界内；
- 阈值以下每 Turn 仍最多 1 次真实 HTTP attempt，触发压缩时累计最多 2 次；两次请求声明的输出预算之和不超过累计上限；
- Compactor 失败本身不产生可见失败气泡；
- “继续”能基于上一请求与真实部分输出自然衔接。

### NR-105 — 生命周期与无新增 History UI

交付：

- 覆盖重启、工程重开、Provider 切换、停止和关窗后的 History/Memory 恢复；
- 中栏继续显示完整 Conversation，不新增 Memory 卡片、History 右栏、Anchor 或压缩按钮；
- Compactor 与主回复共用一次用户可理解的活动生命周期。

验收：

- 未保存手动 Markdown 是唯一关闭阻断；
- 用户在 Compactor 阶段停止时可中断在途 HTTP attempt，并且不启动主回复；
- 确认关窗不等待 Provider，半成品 Memory 不提交；
- 重开后遗留 `running` 按 NR-G1 对账，Memory 不虚构未生成回复。

### NR-106 — 规模测试、参数冻结与 NR-G2 技术提交

交付：

- 用确定性中文长对话 Fixtures 覆盖阈值前后、重复压缩、近期优先重建、Backfill 进度、10,000 条消息、超过 32 KiB 的助手消息分段、失败/取消/重启和无效 Memory；
- 测试内部候选 Profile，并记录 Context 字节数、消息数、精确尾部、摘要大小、覆盖区间、每 Turn 调用数和单次/累计输出上限；
- 根据机器证据选择并冻结 NR-1 参数；
- 运行完整 `pnpm quality`，形成 NR-G2 Manifest 与技术报告后停止。

验收：

- 可见 History 全量保留，构造 Context 始终在硬边界内；
- 覆盖区间没有缺口，重建可优先恢复近期并随正常压缩渐进纳入更早 History；
- 调用与预算符合第 7 节；Loopback 精确证明每 Turn outbound attempts 不超过 2、自动化外部请求为 0；
- `Skill Calls = 0`、`Tool Calls = 0`、`Canon Operations = 0`；
- `realProviderLiveSmokeExecuted = false`，报告不声明真实 Provider 语义质量已验证；
- `nrG2Approved = false`，不得进入 NR-2。

## 10. NR-106 必须回答的规模问题

1. 候选阈值下，哪些最坏输入会逼近 64 条消息、96 KiB 请求或单消息边界？
2. 系统指令、Memory、最近精确尾部与当前消息各自需要怎样的确定性预留？
3. 连续多轮压缩和双向扩展后，摘要是否始终受大小上限约束？
4. 10,000 条可见消息下，增量判断和 Provider 输入是否避免重复提交全量 History？
5. 无效 Memory 首次近期重建后，Segment Cursor 能否在正常触发节奏下无缺口地向前衔接，并保证旧历史持续向后推进？
6. Compactor 普通失败是否仍允许一次主回复，而取消是否可靠阻止主回复？
7. 每 Turn 真实 HTTP attempts、Compactor/主回复声明输出上限及其累计上限是否均有机器证据？
8. 没有 Usage 字段时，请求声明预算之和与响应字节上限是否仍然成立？
9. 超过 32 KiB 的助手消息能否在不修改可见 History 的情况下按 Segment 渐进覆盖？

性能时间只记录为观测值；在没有稳定 CI 基线前不冻结脆弱的毫秒 SLA。结构边界、调用数、字节数和最终输出上限必须在 NR-G2 冻结。

## 11. 明确不包含

- Novelist Skill Registry、Skill 加载或语义选择；
- Agent Tool Loop、Tool Request/Result、任务自主执行；
- 读取或索引角色、世界观、设定、章节等 Canon；
- AI 创建/更新 Markdown、Diff、冲突解决、确认或 Undo；
- Anchor、Context Mark、Proposal、WritePlan、Control JSON 或关键词 Router；
- History 分支、清空、搜索、导出、独立窗口或 Memory 编辑器；
- 多 Conversation Session、跨工程 Memory；
- Provider Cursor、断点续传、自动重试或真实 Provider Live Smoke；
- NR-2～NR-6 的任何实现。

## 12. Draft 1 评审关闭记录

| 评审项     | Draft 2 结论                                                                                               | 状态   |
| ---------- | ---------------------------------------------------------------------------------------------------------- | ------ |
| NR1-D1-R01 | 接受主回复前最多一次压缩；补齐调用前落盘、普通失败回退、取消阻止主回复、无独立失败气泡和无重试边界         | 已关闭 |
| NR1-D1-R02 | 接受一般对话 Memory；补齐五栏目自然语言结构、认知状态、近期 History 优先和不得声称写入 Markdown            | 已关闭 |
| NR1-D1-R03 | 接受先冻结构、后冻参数；补齐内部候选 Profile、机器选择条件、Usage 非唯一边界和离线证据的能力声明           | 已关闭 |
| NR1-D1-R04 | 修订为最近窗口优先；用连续覆盖区间表达中段摘要，并在后续正常压缩中前向衔接、使用剩余预算向更早历史渐进扩展 | 已关闭 |

## 13. 最终评审修订关闭记录

| 评审项      | Draft 2 Final 补充                                                                                                 | 状态   |
| ----------- | ------------------------------------------------------------------------------------------------------------------ | ------ |
| NR1-D2-FR01 | 调用定义为真实 HTTP attempt；Compactor 与主回复均为可中断、零重试路径，停止不得等待请求超时                        | 已关闭 |
| NR1-D2-FR02 | 使用 Segment Cursor 表达消息局部覆盖；旧历史积压时保留 Backfill 预算并保证每次成功正常压缩至少向后推进一个 Segment | 已关闭 |
| NR1-D2-FR03 | 累计边界约束两次请求声明输出预算之和；不依赖 Usage、不动态回收 Compactor 余量，响应字节边界独立生效                | 已关闭 |

## 14. Draft 2 Final 产品批准 Gate

本轮只形成 `NR-1 Story Packet Draft 2 Final` 并等待产品负责人明确批准：

```yaml
nrG1Approved: true
nr1PacketStatus: pending_approval
nr1PacketId: NR-1-D2-FINAL
nr1ImplementationAuthorized: false
realProviderLiveSmokeAuthorized: false
nextAction: approve-NR-1-Draft-2-Final
```

只有产品负责人明确批准 NR-1 Final Packet，并明确给出实施授权与顺序后，才能开始 NR-101。任何后续实施完成后也必须停在 NR-G2 技术提交，不得自动批准 Gate、执行真实 Provider Live Smoke 或进入 NR-2。
