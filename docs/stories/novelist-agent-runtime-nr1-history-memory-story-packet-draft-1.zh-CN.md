# NovelGainker NR-1 Story Packet Draft 1

## History / Memory / Compaction / Long-Conversation Context

> Packet：`NR-1-D1`  
> 状态：`draft`，等待产品评审  
> 上游 Gate：`NR-G1` 已于 2026-09-01 获产品批准  
> 实施授权：`false`  
> 真实 Provider Live Smoke：`false`

## 1. 本次目标

NR-1 在已经通过 NR-G1 的自然对话基线上，补齐类似 Codex 的长对话连续性：

1. 中栏继续保留工程绑定的完整可见对话，不增加独立 History 窗口。
2. Host 增量维护可重建的 Memory，只在上下文达到阈值时调用一次压缩。
3. 每次用户发送后，Host 用“较早历史摘要 + 最近精确对话 + 当前用户消息”组装 Provider Context。
4. 压缩失败不能让普通聊天失败；Host 使用最后一个有效摘要和可容纳的最近对话继续主回复。
5. 先以规模测试选出阈值和输出预算，再在 NR-G2 技术提交中冻结数字。

NR-1 不让 AI 读取小说 Canon，不加载 Novelist Skill，不执行工具，也不写角色、世界观、设定或章节 Markdown。以上能力继续留在 NR-2～NR-4。

## 2. 已冻结的产品结论

以下内容继承自 NR-0 / NR-G1，不在本轮重新讨论：

- 用户只需自然聊天；不使用 Anchor、Context Mark、任务卡、关键词 Router 或线性工作流。
- 用户可见回复只有自然语言，内部 Memory 不混入 Conversation 气泡。
- Host 负责 Provider 调用、History/Memory 持久化、阈值、Context 组装、取消、恢复和安全边界。
- AI 负责生成压缩摘要中的语义内容；Host 不用关键词判断哪些句子“重要”。
- Conversation History 随工程保存，并完整显示在中栏；右栏仍只是真实 Markdown 编辑器。
- 用户消息和 `running` 在主 Provider 调用前落盘；非空回复、未完成标记、停止与关闭行为保持 NR-G1 规则。
- 窗口关闭只受未保存的手动 Markdown 阻断；确认关闭后取消活动工作并直接退出。
- 真实 Provider Live Smoke 需要单独授权，不能由本 Packet 或后续实施授权自动推导。

## 3. 三个概念必须分开

### 3.1 History

History 是 `.novelgainker/conversation/session.json` 中用户可见的完整对话，是 Conversation 的事实源。压缩不得删除、改写、折叠或替换这里的旧消息。

### 3.2 Memory

Memory 是 Host 管理的较早对话摘要，用于后续 Provider Context。它是可丢弃、可重建的派生状态，不是小说 Canon，也不能覆盖真实 History。

建议单独保存到 `.novelgainker/memory/state.json`，最小内容为：

- `schemaVersion`
- `projectId`
- `conversationSessionId`
- `summaryText`
- 已覆盖的连续 Turn 前缀位置
- 已覆盖前缀的校验摘要
- `updatedAtMs`

不得保存 API Key、隐藏 Prompt、原始 Provider 响应、绝对路径、Skill ID、Tool Request、Canon 路径或未来 Workflow 状态。

### 3.3 Context Window

Context Window 是每次请求前临时组装的 Provider 输入，不是新的持久化历史。它由以下内容按固定优先级组成：

1. 固定系统指令；
2. 最后一个有效 Memory 摘要；
3. Memory 尚未覆盖的最近精确对话；
4. 当前用户消息。

当前用户消息必须保留且只出现一次。组装不得超过 Provider 的消息数、单消息大小、请求字节数和本轮冻结后的输出预算。

## 4. Host 与 AI 的边界

```text
完整可见 Conversation Store
             │
             ├── Host 识别连续已终态前缀、阈值与未覆盖范围
             │
             ├── 达阈值：最多一次内部压缩调用
             │             └── AI 只返回自然语言摘要
             │
             ├── Host 校验并原子提交 Memory
             │
             └── Host 组装 Memory + 最近精确 History + 当前消息
                                      │
                                      └── 一次主回复 Provider 调用
```

AI 不负责决定文件路径、写入 Memory 文件、选择阈值或修改 History。Host 不负责用关键词解释小说语义，也不把 Memory 当作角色卡或世界观真值。

## 5. 压缩与增量维护规则

### 5.1 可压缩前缀

- 只处理按顺序排列、已经终态的连续旧 Turn。
- 保留一个受预算保护的最近精确尾部，不把所有消息都压进摘要。
- 已被当前有效 Memory 覆盖的 Turn 不重复逐条送入压缩；下一次压缩输入为“旧摘要 + 新增连续前缀”。
- `running` Turn 永不进入 Memory。
- 对 `failed` / `cancelled` 且已有非空 AI 文本的 Turn，保留实际部分文本并标明其未完成语义，不把 UI 附加的“本次内容未生成完成。”当成小说内容。
- `not_generated` 的占位文本不进入语义摘要，但对应用户消息仍是 History，可供下一次“继续”请求理解。

### 5.2 触发方式

Host 依据可计算的 Context 规模触发压缩，不依据“角色”“设定”“继续写”等关键词。低于阈值时压缩调用数必须为 0；达到阈值时，同一次用户发送最多增加 1 次压缩调用。

Draft 1 建议压缩发生在主回复调用之前：这样本轮主回复可以立即使用新摘要。压缩失败时不重试、不修复输出、不产生一条可见失败回复，而是回退到最后有效 Memory 和当前预算可容纳的最近精确 History，然后继续主回复。

### 5.3 成功提交

压缩输出必须是非空、有大小上限的纯文本。Host 只在输出校验通过后，以原子替换提交新 Memory；失败、取消或进程退出均保留旧 Memory。下次发送时由 Host 根据 History 与覆盖位置重新判断是否需要压缩。

Memory 缺失、损坏、Project/Session 不匹配或覆盖校验不一致时，Host 将其视为无效派生状态，从 History 重新开始建立；不得损坏或阻断 History，也不得触碰小说 Markdown。

## 6. Provider 调用与预算

每次用户发送的调用上限固定为：

- 主回复调用：最多 1 次；
- 压缩调用：达到阈值时最多 1 次，否则 0 次；
- Provider 调用累计上限：最多 2 次；
- 自动重试、修复调用、并行后台压缩：0 次；
- Skill / Tool / Canon 操作：0 次。

输出预算必须同时约束单次和本次用户发送的累计值：

- 主回复输出上限继续受当前 Provider 硬上限保护；
- 压缩输出上限、压缩触发阈值、Memory 摘要上限、精确尾部保留量与累计输出上限先作为内部候选参数进入 NR-106 规模测试；
- 只有规模测试证明不会超出 64 条消息、96 KiB 请求和现有单消息边界后，才能在 NR-G2 Manifest / Report 中冻结最终数字；
- 不增加用户可调“高级预算”、Feature Flag、按 Provider 分叉的产品 UI 或成本仪表盘。

Provider 返回的 Usage 可用于证据与诊断，但不得成为唯一安全边界；没有 Usage 时 Host 的本地上限仍必须成立。

## 7. Conversation 连续性

- 普通闲聊、小说讨论、构思和直接创作继续走同一条 Conversation 路径。
- 缺少角色卡、世界观、章节、Memory 或有效摘要都不能阻断回复。
- 用户在失败或取消后说“继续”，Context 必须包含上一条用户请求和已有的非空部分回复；零文本未生成时至少包含上一条用户请求。
- Memory 只总结更早的聊天，不得声称其中内容已经写入项目 Markdown。
- 最近精确 History 与 Memory 冲突时，最近精确 History 优先；本阶段不提供 Canon 冲突解决。
- 切换 Provider 后，已有 History 与有效 Memory 仍属于工程；新 Turn 使用当前选择的 Provider，并在本次调用中遵守同一 Context 规则。

## 8. Story 顺序

### NR-101 — History / Memory 合同与派生 Store

交付：

- 建立独立、严格、工程绑定的 Memory Store；
- 实现路径边界、大小上限、原子替换、恢复和校验；
- 保持 Conversation History 完整可见且为事实源；
- Memory 无效时可从 History 重建，不迁移旧 AM/Anchor/Workflow 数据。

验收：

- Memory 写入不改变 Conversation 或任一小说 Markdown/资产字节；
- Store 不含 Key、隐藏 Prompt、原始 Provider 响应、绝对路径、Skill/Tool/Canon 字段；
- 缺失、损坏、错 Project/Session 和覆盖校验失败均安全退化。

### NR-102 — 增量覆盖与阈值判断

交付：

- 从 History 计算连续已终态前缀、已覆盖位置、新增可压缩范围和受保护精确尾部；
- 低于阈值零压缩调用，达到阈值只产生一次待执行压缩；
- 明确 completed / failed / cancelled / not_generated 的输入规则。

验收：

- 多轮压缩只处理旧摘要与新增前缀，不重复发送全部 History；
- running、占位提示和 UI 附加语句不会污染摘要；
- 不使用关键词、正则或小说类型规则触发压缩。

### NR-103 — 有界 Compactor Provider 模式

交付：

- 在现有 Rust Provider Adapter 内增加只返回纯文本摘要的内部 Compactor 模式；
- 使用当前选定 Provider 与现有 CredentialVault，不新增 Credential IPC；
- 无流式 Conversation 气泡、无 Control JSON、无解析修复循环、无自动重试；
- 校验成功才提交 Memory，失败保留旧 Memory。

验收：

- 每次用户发送压缩调用最多 1；
- 压缩内容不进入可见 History；
- 自动化只使用本地 Loopback，不读取真实 Key、不访问外网。

### NR-104 — Context Assembler 与主回复回退

交付：

- 按固定优先级组装系统指令、有效 Memory、最近精确 History 和当前用户消息；
- 保障当前消息只出现一次，所有输入满足 Provider 硬边界；
- 压缩失败时仍执行一次主回复调用；
- 保持现有流式、终态持久化、Store 对账和“未生成/未完成”语义。

验收：

- 阈值以下每 Turn 仍为 1 次 Provider 调用；阈值触发时最多 2 次；
- 压缩失败不产生“本次回复未生成”，除非主回复本身也未生成；
- 用户说“继续”时可看到上一请求和部分输出，不需要专用 Continue 状态机。

### NR-105 — 生命周期与无新增 History UI

交付：

- 重启、工程重开、Provider 切换、取消和关窗后的 History/Memory 恢复；
- 中栏继续呈现完整 Conversation，不新增 Memory 卡片、History 右栏、Anchor 或压缩按钮；
- 活动压缩与主回复共用一次用户可理解的 AI 活动生命周期。

验收：

- 未保存手动 Markdown 仍是唯一关闭阻断；
- 确认关窗不等待压缩或 Provider；
- 重开后遗留 running 仍按 NR-G1 规则变成未生成，Memory 不虚构该回复。

### NR-106 — 规模测试、预算冻结与 NR-G2 技术提交

交付：

- 使用确定性中文长对话 Fixtures 覆盖阈值前后、多次增量压缩、10,000 条消息上限、长单消息、失败/取消/重启和 Memory 重建；
- 记录 Context 字节数、消息数、精确尾部、摘要大小、每 Turn 调用数、单次/累计输出上限和自动化外部请求数；
- 根据规模结果选择并冻结 NR-1 参数；
- 运行完整 `pnpm quality`，形成 NR-G2 Manifest 与技术报告后停止。

验收：

- 可见 History 全量保留，Context 始终在硬边界内；
- 阈值与预算来自机器规模证据，不由 Draft 预先拍定；
- `Skill Calls = 0`、`Tool Calls = 0`、`Canon Operations = 0`；
- `realProviderLiveSmokeExecuted = false`；
- `nrG2Approved = false`，不得进入 NR-2。

## 9. 规模测试必须回答的问题

NR-106 在冻结参数前至少回答：

1. 多少精确 History 会稳定逼近当前 96 KiB Request 边界？
2. 为当前消息、系统指令、Memory 和最近精确尾部分别保留多少空间？
3. 一次摘要在连续多轮压缩后是否仍受大小上限约束？
4. 10,000 条可见消息下，增量判断是否只与新增前缀相关，而不是每次重做全量 Provider 压缩？
5. 阈值触发 Turn 的最大调用数与累计输出预算是否有机器证据？
6. 压缩失败、取消或崩溃后，下一次普通对话能否不经人工修复继续？

性能时间只作为观测值，不在没有稳定 CI 基线时冻结脆弱的毫秒 SLA；结构边界、调用数、字节数和输出上限必须冻结。

## 10. 明确不包含

- Novelist Skill Registry、Skill 加载或语义选择；
- Agent Tool Loop、Tool Request/Result、任务自主执行；
- 读取或索引角色、世界观、设定、章节等 Canon；
- AI 创建/更新 Markdown、Diff、冲突解决、确认或 Undo；
- Anchor、Context Mark、Proposal、WritePlan、Control JSON 或关键词 Router；
- History 分支、清空、搜索、导出、独立窗口或 Memory 编辑器；
- 多 Conversation Session、跨工程 Memory、云同步；
- Provider Cursor、断点续传、自动重试或真实 Provider Live Smoke；
- NR-2～NR-6 的任何实现。

## 11. Draft 1 评审项

### NR1-D1-R01 — 压缩时机与失败回退

建议：达到阈值时在主回复前最多压缩一次；压缩失败便使用旧 Memory + 最近精确 History 继续主回复，不重试、不显示独立失败气泡。

### NR1-D1-R02 — Memory 的语义范围

建议：NR-1 只生成“帮助继续对话”的一般摘要，可包含用户决定、已有创作内容、待办和未解决问题，但明确不是 Canon。角色/世界观文件的语义维护要等 NR-3/NR-4 的 Novelist Skill 与 Host Tool 通道。

### NR1-D1-R03 — 规模参数冻结

建议：Draft 只冻结“最多 2 次 Provider 调用、无重试、零 Skill/Tool/Canon”这些结构边界；具体压缩阈值、摘要上限、精确尾部和累计输出 Token 上限由 NR-106 的候选规模测试选定，并在 NR-G2 提交中冻结。

### NR1-D1-R04 — Memory 无效时的处理

建议：Memory 是派生缓存。缺失、损坏或与 History 不一致时不弹迁移/修复流程，不阻断聊天；Host 丢弃无效 Memory，并从完整 History 渐进重建。

## 12. 授权边界

本文件只是 `NR-1 Story Packet Draft 1`。当前仅授权形成和评审 Packet：

```yaml
nrG1Approved: true
nr1PacketStatus: draft
nr1ImplementationAuthorized: false
realProviderLiveSmokeAuthorized: false
nextAction: review-NR-1-Draft-1
```

只有产品负责人明确批准 NR-1 Final Packet 并给出实施顺序后，才能开始 NR-101。实施完成后必须停在 NR-G2 技术提交，不得自动批准 Gate 或进入 NR-2。
