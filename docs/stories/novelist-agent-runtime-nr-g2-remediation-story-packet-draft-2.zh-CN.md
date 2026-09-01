# NR-G2 Remediation Story Packet Draft 2

> 状态：`implemented`，已停止在 NR-G2 技术重新提交  
> 日期：2026-09-01  
> BMAD Packet：`review-gate-packet`  
> 上游：`NR-G2-technical-submission` 评审结论为 `changes_requested`  
> 前版：`NR-G2-REMEDIATION-D1` 评审方向已接受，边界补充见本版  
> 范围：只关闭 `NR-G2-R01`～`NR-G2-R03`  
> 实施授权：`true`  
> 真实 Provider Live Smoke：`false`  
> NR-2 授权：`false`

## 1. Draft 2 结论

NR-101～NR-106 的既有实现与 NR-G2 技术证据不回退，但 NR-G2 暂不批准。本 Packet 仍只修复评审确认的三个合同缺口：Provider 动态系统指令传递、有效 Memory 的增量失败边界、五栏目摘要的严格结构校验。

Draft 2 不增加新的 Story 或产品能力，只补齐 Draft 1 评审确认的实现边界：

1. `NormalizedRequest` 在 Adapter 序列化前强制验证唯一、非空、首位 System Message，并以双 Provider × completion/streaming 四条实际序列化路径证明保真；
2. 只有 Memory 加载/验证结果为“不存在有效 Memory”时才允许近期重建；有效 Memory 的任何增量不可执行或后续失败都不得被重新解释为无效 Memory；
3. 五栏目合同使用精确行语法和与之完全一致的 Compactor Prompt；JSON 拒绝只针对整体顶层结构，不扫描栏目正文中的关键词、括号或合法叙述片段。

三项修复完成并通过证据后，只重新提交 NR-G2 技术报告与 Manifest。不得借本 Packet 增加 Novelist Skill、Tool Loop、Canon 读取或写入、AI Markdown 写入、Anchor、Control JSON、独立 History/Memory UI、自动重试或真实 Provider Live Smoke。

## 2. 保持不变的边界

- 完整 Conversation History 仍是事实源；Memory 仍是可丢弃、不可见、非 Canon 的工程派生状态。
- 每次用户发送最多一次 Compactor attempt 和一次主回复 attempt；两者可取消且零重试。
- 当前冻结的 `balanced-64k` Profile、消息/请求/响应硬边界和输出预算不因本修复调整。
- 用户消息与 `running` 必须继续在任何本轮 Provider 调用前持久化。
- Compactor 普通失败继续保留最后一个有效 Memory，并允许主回复继续。
- `Skill Calls = 0`、`Tool Calls = 0`、`Canon Operations = 0`。
- 不新增 IPC、Provider、Credential 路径、产品 UI、后台任务或兼容层。
- 自动化保持 Cargo Offline 和本地 Loopback；不得读取真实 Key 或访问真实 Provider。

## 3. Story NR-G2-R01：Provider 动态系统指令端到端保真

### 3.1 问题

`NormalizedRequest::with_system` 可以构造 Compactor 专用系统指令和带 Memory 的主回复系统指令，但 OpenAI Adapter 当前把 `instructions` 固定回普通 Conversation 指令，并过滤请求中的 System Message。结果是 OpenAI Compactor 收不到五栏目合同，OpenAI 主回复也收不到 Memory。

### 3.2 请求级不变量

每个出站 `NormalizedRequest` 在进入 Adapter 序列化前必须通过统一校验：

1. `messages[0]` 必须是 System Message；
2. 整个 `messages` 中必须恰好存在一条 System Message；
3. System Message 去除首尾空白后必须非空；
4. 普通 Conversation、带 Memory 的主回复、Compactor 和连接测试请求都必须经过同一校验入口；
5. 校验失败必须在发出 HTTP 请求前终止，不允许 Adapter 猜测、补写、回退或选择另一条系统指令。

该不变量只收紧既有内部请求合同，不新增用户可编辑 Prompt、IPC 或 Provider 能力。

### 3.3 Adapter 映射边界

- 通过校验的 `NormalizedRequest` 中唯一且位于首位的 System Message，是本次请求系统指令的唯一事实源。
- DeepSeek Adapter 必须把该 System Message 原样保留为 `messages[0]`。
- OpenAI Adapter 必须把该 System Message 的内容原样写入 `instructions`，并只从 `input` 中移除这一条 System Message。
- Adapter 不再自行选择、生成、覆盖或重复系统指令。
- 不得把 Compactor 指令混入用户可见 History，不得记录或输出完整请求体。
- 本修复不改变固定 Provider URL、模型、CredentialVault、一次 attempt、取消或响应解析逻辑。

### 3.4 验收标准

1. DeepSeek Compactor completion 请求的 `messages[0]` 精确等于 `COMPACTOR_SYSTEM_INSTRUCTION`。
2. OpenAI Compactor completion 请求的 `instructions` 精确等于 `COMPACTOR_SYSTEM_INSTRUCTION`，且 `input` 不含 System Message。
3. DeepSeek 主回复 streaming 请求包含带 Memory 的动态 System Message，Memory 只出现一次。
4. OpenAI 主回复 streaming 请求的 `instructions` 包含同一份有效 Memory，`input` 不重复它。
5. 普通无 Memory Conversation 对两个 Provider 仍使用固定普通对话指令。
6. 双 Provider 请求体捕获测试覆盖四条实际序列化路径：DeepSeek completion、DeepSeek streaming、OpenAI completion、OpenAI streaming。
7. 四条路径均从本地 Loopback 捕获实际序列化 JSON，只断言必要字段；不得把 Key 或完整请求写入 Evidence。
8. 非首位 System、重复 System 或空白 System 均在出站前失败，并证明 outbound HTTP attempt 为零。
9. Compactor 与主回复仍各自最多一个 outbound HTTP attempt，自动重试为零。

### 3.5 非目标

- 新增第三个 Provider；
- 允许用户编辑系统 Prompt；
- Prompt 模板版本系统；
- 真实 Provider 语义质量测试。

## 4. Story NR-G2-R02：有效 Memory 增量失败时禁止近期重建

### 4.1 问题

存在有效 Memory 时，若增量计划因没有可压缩的前向 Segment、无法同时容纳前向连续性与 Backfill，或其他机械边界而不可执行，当前规划器会继续尝试 `recent_rebuild_plan`。这可能用一个新的近期区间替换有效 Memory，导致覆盖范围缩小、迁移或较早摘要丢失。

### 4.2 唯一状态来源与固定决策表

“没有有效 Memory”只能由本轮规划开始时的 Memory 加载与合同校验结果确定：

- `Absent`：Memory 文件不存在；
- `Invalid`：Memory 文件存在，但未通过既有 Schema、工程绑定、History 源一致性或覆盖合同校验；
- `Valid`：Memory 已通过上述既有校验。

只有 `Absent` 或 `Invalid` 可以进入近期优先重建。一个已经判定为 `Valid` 的 Memory，在本轮后续不得因为增量计划为空、规划错误、预算不足、请求构造失败、网络失败、输出无效或提交失败而被重新分类为“无有效 Memory”。

| Memory 状态   | 阈值/输入状态                               | 允许的计划                                           |
| ------------- | ------------------------------------------- | ---------------------------------------------------- |
| 无有效 Memory | 达到阈值且有可压缩输入                      | 可以执行一次近期优先重建                             |
| 有效 Memory   | 增量计划同时满足前向连续性、Backfill 和预算 | 可以执行一次增量压缩                                 |
| 有效 Memory   | 增量计划不可执行                            | 不产生 Compactor 计划；保留旧 Memory，直接组装主回复 |
| 任意状态      | 未达到阈值或没有可压缩输入                  | Compactor 调用为零                                   |

近期重建只允许用于本轮初始判定为“没有有效 Memory”的状态。它不能作为有效 Memory 增量失败后的第二选择，也不能作为 Compactor 后续失败后的补救路径。

### 4.3 实施边界

- 有效 Memory 的增量计划返回空时，`plan_compaction` 返回无计划，不调用 Compactor。
- 有效 Memory 的规划或请求构造失败不得继续调用近期重建；按既有稳定错误与主回复回退边界处理。
- 旧 Memory 文件、摘要、`coveredFromCursor`、`coveredThroughCursor`、`sourceHash` 和 `updatedAtMs` 必须逐字节保持不变。
- 主回复继续使用旧 Memory 与预算可容纳的最近精确 History；不得产生可见压缩失败气泡。
- 不得通过缩小、移动或重建覆盖区间来制造“成功”。
- 后续 Turn 只有再次达到正常阈值且增量计划可执行时，才允许继续前向衔接和 Backfill。
- 不新增“等待压缩”“待修复”或其他持久状态，不增加后台重建、修复模型、额外 Provider attempt、迁移或用户提示。

### 4.4 验收标准

1. 有效 Memory 且没有可压缩前向 Segment 时，规划结果为空，Compactor attempt 为零，Memory 字节不变，主回复仍可执行一次。
2. 有效 Memory 且组合输入无法同时满足连续性与最小 Backfill 时，不调用 Compactor，也不调用近期重建。
3. 有效 Memory 的增量规划为空、规划错误或请求构造失败，都不会进入近期重建，也不会改写 Memory。
4. 有效 Memory 的正常增量成功路径仍同时推进 `coveredThroughCursor`，并在存在旧积压时推进 `coveredFromCursor`。
5. 无有效 Memory 时，原有近期优先重建行为保持不变。
6. Compactor 普通网络失败、输出无效或提交失败继续保留旧 Memory，并只影响内部派生状态。
7. 回归测试必须比较压缩前后的 Memory 文件字节和覆盖区间，不能只断言函数返回值。

### 4.5 非目标

- 主动修复或迁移无效 Memory；
- 为追赶旧 History 增加独立调用；
- 改变 Backfill、Segment 或触发阈值；
- 增加 Memory UI。

## 5. Story NR-G2-R03：五栏目摘要严格结构校验

### 5.1 问题

当前校验只检查五个标题字符串是否出现，没有证明标题唯一、顺序正确、各栏目有正文，也没有完整拒绝标题前说明、代码围栏或整体 JSON/Control JSON。结构不符合合同的模型输出仍可能被提交为有效 Memory。

### 5.2 Prompt 与接受语法必须一致

Compactor System Prompt 必须直接给出以下精确输出模板，不得只在说明句中罗列标题，也不得要求 Markdown 标题标记或 Control JSON：

```text
已确认决定
<自然语言正文；无内容时写 - 无>
正在创作或讨论
<自然语言正文；无内容时写 - 无>
未决问题
<自然语言正文；无内容时写 - 无>
用户明确任务
<自然语言正文；无内容时写 - 无>
已产生但未完成的内容
<自然语言正文；无内容时写 - 无>
```

Compactor 输出必须是 UTF-8 自然语言纯文本，并满足：

1. 摘要字节上限针对 Provider 返回的原始 UTF-8 字节计算；越限后不得先截断再尝试通过校验。
2. 解析时只允许把 `CRLF` 或单独 `CR` 规范化为 `LF`，然后去除全文首尾空白；不得改写正文内容。
3. 规范化后的第一行必须精确为 `已确认决定`；标题前不允许说明文字。
4. 以下五个标题必须各自独占一行、精确出现一次，并严格按此顺序出现：
   - `已确认决定`
   - `正在创作或讨论`
   - `未决问题`
   - `用户明确任务`
   - `已产生但未完成的内容`
5. 标题只按“整行精确相等”识别，不按子串、关键词或正文内容识别。
6. 每个标题与下一个标题之间必须存在去除首尾空白后仍非空的正文；没有内容时允许使用 `- 无`。
7. 栏目正文可以使用普通段落或 Markdown 列表，但不得包含代码围栏行。
8. 整个输出为顶层 JSON 对象、顶层 JSON 数组或 Control JSON 时无效；该判断不得扫描栏目正文中的 `{}`、`[]`、`JSON`、`Control JSON` 等字样。
9. 栏目正文偶然提到标题词、JSON 或括号不自动失败；如果某个必需标题只嵌在正文句子中而没有独占一行，则仍因“缺少该标题”而失败。

### 5.3 失败语义

- 无效输出不得写入或替换 Memory。
- 不调用模型修复，不重试 Compactor，不创建可见失败气泡。
- 存在旧 Memory 时保留其完整字节；不存在时继续无 Memory 主回复。
- 主回复是否成功仍只由主回复 Provider 结果决定。

### 5.4 验收标准

1. 精确五栏目、顺序正确、栏目均有正文的自然语言摘要通过。
2. 同一份合法摘要分别使用 LF 与 CRLF 时均通过。
3. 缺失任一标题、任一标题重复、任意乱序、标题前有正文或任一空栏目均失败。
4. 某标题只出现在正文句子中而没有对应独占标题行时失败；在五个合法标题均存在时，正文提到标题词不会形成重复标题。
5. 整体顶层 `{...}`、整体顶层 `[...]`、整体 Control JSON、代码围栏行以及越过 `summaryBytes` 的输出均失败。
6. 合法五栏目正文中出现 JSON 字样、括号或 JSON 片段时，不得仅因此失败。
7. 无效摘要回归必须证明 Memory 文件未创建或旧文件逐字节不变，主回复仍按普通失败回退。
8. 校验器是确定性本地结构解析，不使用关键词 Router、正文子串扫描、第二次模型调用或 Provider 专用分支。

### 5.5 非目标

- 用 Host 判断小说语义是否正确；
- 证明真实模型一定不会把建议提升为决定；
- 将 Memory 转为 JSON Schema；
- 把 Memory 暴露给用户编辑。

## 6. 拟定实施顺序

本 Draft 2 已获产品负责人明确批准并授权实施，顺序固定为：

1. `NR-G2-R01`：修正请求级不变量和双 Provider 动态系统指令，并补四条请求体捕获路径；
2. `NR-G2-R02`：收紧有效 Memory 的规划状态机并补不替换回归；
3. `NR-G2-R03`：对齐 Compactor Prompt 与严格五栏目解析，并补正反例矩阵；
4. 更新 NR-106 Evidence，使三项结论来自请求体捕获和回归结果，而不是只写期望常量；
5. 运行定向测试与完整 `pnpm quality`；
6. 重新生成 NR-G2 Manifest，更新技术报告后停止，等待 NR-G2 重新评审。

不得执行真实 Provider Live Smoke，不得进入 NR-2。

## 7. 测试与证据矩阵

| Story | 最小证据                                                                | 必须证明                                                           |
| ----- | ----------------------------------------------------------------------- | ------------------------------------------------------------------ |
| R01   | DeepSeek/OpenAI × completion/streaming 四条实际请求体捕获；非法请求反例 | 动态 System 指令保真且唯一；非法 System 合同在出站前失败           |
| R02   | 有效/无效 Memory、无前向输入、预算不可满足、规划/构造失败、正常增量     | 只有初始无有效 Memory 才能近期重建；有效 Memory 不被重新分类或替换 |
| R03   | LF/CRLF 正例与缺失/重复/乱序/空栏目/前导文本/顶层 JSON/围栏/超限反例    | 严格五栏目可提交；正文不受关键词扫描；失败不阻断主回复             |

共同证据：

- 两个 Provider 的请求体测试全部使用本地 Loopback；
- `maximumOutboundAttemptsPerTurn = 2`；
- `automaticRetriesPerTurn = 0`；
- `Skill Calls = 0`、`Tool Calls = 0`、`Canon Operations = 0`；
- `automatedExternalRequests = 0`；
- `realProviderLiveSmokeExecuted = false`；
- 完整 `pnpm quality`、Debug/Release 构建和桌面窗口 Smoke 通过。

## 8. NR-G2 重新提交条件

只有以下条件全部满足，才能重新形成 NR-G2 技术提交：

- R01～R03 全部完成并有定向自动化证据；
- NR-106 Manifest 从实际请求体与测试结果派生新的 Remediation 结论；
- 原有 Profile、调用数、预算、History/Memory、取消和安全证据继续通过；
- NR-G2 Report 明确列出本次修复，并继续声明真实 Provider 摘要语义质量未验证；
- BMAD 状态仍保持 `nrG2Approved = false`、`realProviderLiveSmokeExecuted = false`、`nr2Authorized = false`。

## 9. Draft 1 评审关闭记录

| 决策项      | Draft 2 关闭结论                                                                                 |
| ----------- | ------------------------------------------------------------------------------------------------ |
| NRG2-D1-R01 | 方向接受；补充统一请求校验、Adapter 禁止猜测，以及双 Provider × completion/streaming 四路径证据  |
| NRG2-D1-R02 | 方向接受；补充“无有效 Memory”唯一来源，禁止把增量不可执行或后续失败重新分类后进入近期重建        |
| NRG2-D1-R03 | 方向接受；Prompt 与行语法完全对齐，JSON 只按整体顶层结构拒绝，正文不做关键词、括号或标题子串扫描 |

Draft 1 的三项设计决策已在 Packet 层关闭。产品负责人已于 2026-09-01 批准 Draft 2，并授权按 `NR-G2-R01` → `NR-G2-R02` → `NR-G2-R03` 连续实施。

## 10. 批准边界

当前批准与授权状态：

```yaml
packetId: NR-G2-REMEDIATION-D2
packetStatus: implemented
implementationAuthorized: true
nrG2Approved: true
realProviderLiveSmokeAuthorized: false
nr2Authorized: false
nextAction: await-product-direction-at-NR-G2
```

R01～R03 已按批准顺序完成，Manifest、技术报告与完整 `pnpm quality` 已更新。产品负责人已于 2026-09-01 批准 NR-G2 技术提交并确认 Gate 通过；当前停止于 NR-G2，不执行真实 Provider Live Smoke，不形成或进入 NR-2。
