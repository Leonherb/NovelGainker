# Pre-NR-4 Provider Tool Compatibility Repair Story

## 0. 文档状态

- Story：`PRE-NR4-PTC-001`
- 标题：`Provider Tool 传输身份、参数合同与真实兼容性修复`
- 版本：`Draft 2（独立评审修订版）`
- Packet 类型：`brownfield-resume-packet / single repair story`
- 总项目级别：`Level 4`
- 当前修复切片：`Level 1`
- 上游基线：`NR-3 已实现，NR-G4 已通过`
- 形成授权：**产品负责人已于 2026-09-04 明确要求形成并评审本 Story**
- 当前状态：**宽范围 Story 已评审待产品决定；用户直接授权的窄修复已离线完成，待应用内手工复测**
- 评审决定：`ready_for_product_decision`
- 评审记录：`docs/reviews/novelist-agent-runtime-pre-nr4-provider-tool-compatibility-repair-story-review.zh-CN.md`
- 产品批准：**未批准**
- 宽范围实施授权：**未授权**
- 窄修复实施：**已获用户直接授权、已离线完成并通过质量门**
- 正式 Provider Live Smoke：**未授权、未执行**
- `NR-4～NR-7`：**未进入、未形成 Story Packet**

`PRE-NR4-PTC-001` 是真实使用暴露兼容缺陷后新增的阶段外 Preflight Repair 定位符。它不计入已经关闭的 `NR-301～NR-306`，不占用 `NR-401`，不重开已通过的 `NR-G4`，也不复活已经清理的 NR-G4 Remediation、Follow-up、Second Remediation 或 Corrigendum lineage。

批准本 Story、授权其离线实施与授权真实 Provider Live 验证是三个独立决定；任何一个决定都不得从另一个决定推导。

### 0.1 范围收窄与已实施结果（2026-09-04）

用户明确表示“只需要修这个 bug”，因此本次只实施以下窄范围修复：

- 为 DeepSeek 投影 `novelist_read_context` 的兼容参数 Schema；省略的无关字段仅在 Host 内补为既有的安全默认值，仍由严格 Canon 合同校验必填语义字段；
- 空工程的 `characters` list 实际返回空列表后，第二轮只自然答复“当前工程未发现人物资料。”，不暴露 Tool 名称、接口组合或参数报错；
- 以本地 loopback 覆盖“DeepSeek 工具调用 → 空 Canon 结果 → 自然答复”的全链路，并通过完整离线质量门。

该结果不等于真实 Provider Live 已验证；未读取 Credential，未执行正式 Live Smoke。其余宽范围 Story 条款仍保持待产品决定状态。

---

## 1. BMAD Routing Brief

### Scope

- Packet type：`brownfield-resume-packet`；
- Evidence in hand：活动仓库、已通过 Gate、用户手动真实 Provider 观察、受控 Task 终态；
- Project type：Windows-first local-first desktop application；
- Project level：整体 `4`，本修复切片 `1`；
- Runtime context：Codex；
- Main constraint：`review-blocker / execution-readiness`；
- Confidence：高。

### Current phase

- 当前位于 implementation Gate boundary；
- NR-G4 继续保持 `passed / approved`；
- 真实 DeepSeek 已证明当前 Provider Tool 路径尚不能成功执行受支持的只读请求，因此 NR-4 入口存在已知阻断。

### Recommended next move

- 只形成并评审一个 `PRE-NR4-PTC-001`；
- 评审后停止于产品决定，不自动实施、不自动 Live、不形成 NR-4 Packet。

### What not to do yet

- 不追改或重开 NR-G4；
- 不通过放宽 Host 校验、增加重试或强制每轮 Tool Call 掩盖兼容问题；
- 不增加 Canon Write、NR-4 DTO/Executor、UI/IPC 或新 Provider 端点；
- 不把用户日常使用观察写成正式双 Provider Live Smoke 已完成。

---

## 2. 依据、问题与阻断判断

### 2.1 已观察到的事实

用户在正式应用中使用真实 DeepSeek 时，先后观察到：

1. 内部 Tool 身份 `novelist.read_context` 被原样放入 Provider function name 后，真实端点以协议错误拒绝请求；
2. 当前工作区已有一个暂定的传输层修正：Provider wire name 使用 `novelist_read_context`，进入 Host 后恢复为唯一内部身份 `novelist.read_context`；
3. 修正 wire name 后，DeepSeek 能发出可配对的 Tool 请求，Host 能完成名称恢复和 Registry lookup；
4. 该请求在参数 JSON/Schema/领域语义校验阶段以 `agent.tool_arguments_invalid` 失败，Task 没有进入 `running`，Executor 执行次数为 0，Pack Resource/Canon I/O 均为 0；
5. Host 生成了受控失败 Tool Result，Continuation 后 Conversation 仍为 `completed`，说明 fail-closed 与“工具失败不阻断聊天”路径正确；
6. 模型同时错误声称组合工具无法独立读取 Capability 基础说明或只检查 Canon。实际合同允许 `capabilityResources = 0`、`canonOperations = 0`；Capability 基础 instruction 仍可返回，空工程也应返回合法空 Canon 结果。

这组事实仅来自用户主动操作应用形成的局部真实观察。它不是由自动化或代理执行的正式 Live Smoke，也没有覆盖 OpenAI。

### 2.2 当前证据不能证明的内容

Task Store 按安全合同只保存原始 Arguments 的 SHA-256、UTF-8 字节数、受控状态和错误码，不保存原始参数、Provider Payload 或 Continuation。因此当前不能断言具体非法字段是什么。

“DeepSeek 省略了 required-null 字段”是强假设，不是已证实根因。实现不得围绕该假设做无限制容错，也不得为了取证新增原始参数、Query、路径、Capability ID、Resource ID、Provider 回复或隐藏 Prompt 的持久化与日志。

### 2.3 阻断级别

| 范围              | 结论                                                                                        |
| ----------------- | ------------------------------------------------------------------------------------------- |
| NR-G4 历史批准    | 不追溯阻断；真实 Provider 验证原本明确在 Gate 范围外                                        |
| 普通聊天          | 不阻断；失败续聊已经工作                                                                    |
| NR-4 规划         | 不阻断；但不得把规划写成已进入实施                                                          |
| NR-4 实施入口     | 默认阻断；双 Provider Live 通过，或产品负责人以书面风险豁免明确覆盖既有进入条件后，才可解除 |
| NR-G5 / NR-4 完成 | 默认阻断；不得从 NR-4 形成或实施授权中隐式推导解除，仍需 Live 通过或同一项明确风险豁免      |

本 Story 的默认建议是先修复并完成独立 Live 复验，再决定是否形成 NR-4 Packet。

---

## 3. User Story 与产品结果

> 作为使用 DeepSeek 或 OpenAI 的小说作者，当我要求 AI 核对当前工程事实或加载内置小说方法时，我希望 Provider 能生成 Host 可接受的单次只读 Tool 请求；即使工程为空、读取失败或没有可读内容，聊天也必须继续自然完成，并且绝不写入 Canon。

完成后的用户可观察结果：

1. 普通聊天仍不调用 Tool，并以一次主回复完成；
2. 明确依赖当前工程事实的请求可触发一次只读 Tool；
3. Capability-only 请求不需要伪造 Canon 操作；
4. 空工程 Canon list 返回合法空集合，而不是参数错误；
5. 成功活动显示既有受控“已完成相关操作”，失败活动显示既有受控失败文案；
6. Tool 成功或失败后都继续给出自然语言，机器参数和内部路径不进入气泡；
7. 整个修复期间 Canon 写入次数保持 0。

---

## 4. 继承且不可破坏的合同

1. Production Registry 仍精确包含一个 Tool；内部产品身份固定为 `novelist.read_context`，版本固定为 `1`。
2. 每 Turn 最多一个 Tool Call、一个 Tool Execution、一个 Tool Round、两个主 Agent Attempts；Conversation 自动重试保持 0。
3. Host 不扫描用户关键词、不预选 Capability、不猜 Query/Scope/Path/Limit，也不强制普通聊天调用 Tool。
4. 通用 Tool Schema 校验与 `NovelistReadContextExecutorV1` 二层语义校验都保留；未知字段、非法类型、越界、路径穿越、Scope 越权和 Cursor 错配继续 fail closed。
5. Capability Pack 仍为编译期内置、自包含、版本固定的发布物；不加载用户 `SKILL.md`、外部插件缓存、Python Sidecar 或外部 Runtime Host。
6. Tool Result 只进入当前 Turn 的 Provider Continuation；不进入 History、Memory 或 Canon。
7. Task Store 不保存原始 Arguments/Result/Continuation；Provider Key 只存在于 Rust CredentialVault 调用路径。
8. 不增加 Tauri Command、Event Channel、前端文件权限、Shell/Process/Opener 或任意 URL。
9. 不创建、修改、删除、移动或重命名任何 Canon Markdown。
10. NR-G4 的技术提交、机器 Manifest 和 Gate rereview 都保持冻结，不改写其历史声明或 Hash。

---

## 5. 设计决策

### 5.1 产品身份与 Provider wire name 分离

- `novelist.read_context` 继续是唯一 canonical/internal Tool identity；
- DeepSeek Chat Completions 与 OpenAI Responses 的 function wire name 固定为 `novelist_read_context`；
- wire alias 只是 Adapter 传输映射，不是第二个 Registry Tool、不进入 Capability Catalog，也不产生第二个版本；
- Definition 出站、流式/非流式 Call 入站、DeepSeek Assistant Tool Call Continuation、OpenAI Function Call/Output Continuation 必须使用同一可逆映射；
- 任何未知 wire name 或映射不对称都在执行前拒绝，Executor 次数为 0。

该修正覆盖 Provider function name 只接受字母、数字、下划线和连字符的真实约束，同时保留已批准的内部领域命名。

### 5.2 canonical Host DTO 与 Provider wire schema projection 分离

内部 `NovelistReadContextRequestV1`、结果 Bundle、权限交集与二层校验继续是唯一 canonical 语义合同。Adapter 可以为不同 Provider 生成语义等价但表示方式不同的 wire schema：

- OpenAI 继续使用 `strict = true`，每个 object 的 properties 全部 required，语法可选值通过 nullable 表达；
- DeepSeek 正式端点继续使用当前非 Beta Chat Completions，不静默切换 `/beta`，不把 Beta strict mode 当作修复前提；
- DeepSeek wire schema 必须明确标注 Capability ID、资源、空数组语义，以及 list/search/read 三类操作各自需要的字段、互斥字段和数值范围；
- 两个 Provider 的 schema 不再要求序列化 JSON 逐字相同，但必须投影到同一个 canonical DTO，并由相同 Host 安全校验裁决。

DeepSeek 非 strict wire schema 的 required/optional 集合固定如下；可以使用 `oneOf`/判别分支或语义等价表示，但不得退回 canonical required-all schema 原样出站：

| 对象/分支                | wire required                                                 | wire optional / 省略语义                                              |
| ------------------------ | ------------------------------------------------------------- | --------------------------------------------------------------------- |
| 根对象                   | `capabilityId`                                                | `capabilityResources`、`canonOperations`；省略分别表示显式空数组      |
| Capability Resource item | `resourceId`                                                  | `cursor`；省略表示 `null` / 首屏                                      |
| Canon `list`             | `kind`、非空 `scopes`、`limit`                                | `cursor`；省略表示首屏。其余字段只能省略或为 `null`                   |
| Canon `search`           | `kind`、非空 `scopes`、非空 `query`、`caseSensitive`、`limit` | `cursor`；省略表示首屏。其余字段只能省略或为 `null`                   |
| Canon `read` 初次读取    | `kind`、`relativePath`、`startLine`、`maxBytes`               | 空 `scopes` 与 `cursor = null` 可省略；其余字段只能省略或为 `null`    |
| Canon `read` 分页继续    | `kind`、`relativePath`、非空 `cursor`、`maxBytes`             | 空 `scopes` 与 `startLine = null` 可省略；其余字段只能省略或为 `null` |

每个分支只能暴露 canonical 字段并保持 `additionalProperties = false`；已提供的字段仍须满足 canonical 类型、枚举、长度、范围和互斥合同。规范化完成后的对象必须满足 canonical required-all Schema，之后再进入同一个 Executor 二层校验。

Provider-visible description 至少明确：

1. 选择一个 Capability 时，其基础 instruction 总会包含在结果中；
2. `capabilityResources` 与 `canonOperations` 可分别为空；
3. Capability-only 不需要虚构 Canon 操作；
4. Canon-only 意图可以用零项 optional resources 加合法 Canon 操作完成；
5. 空工程 list/search 的空结果是成功结果；
6. 不得用自然语言回答替代用户明确要求的实际工程核对。

### 5.3 只允许纯语法、白名单化规范化

DeepSeek 非 strict wire 参数进入 canonical 校验前，可以在当前 Turn 内进行一次显式、Provider-specific、Tool-specific 的纯语法规范化。允许范围固定为：

| 位置                | 允许补全                                                                                                                                                                           | 禁止推断                                                                         |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| 顶层                | 已批准为零到多项且缺失的 `capabilityResources` / `canonOperations` 补为空数组                                                                                                      | 不得补 `capabilityId`                                                            |
| Capability Resource | 缺失的分页 `cursor` 补 `null`                                                                                                                                                      | 不得替换、修正或选择 `resourceId`                                                |
| `list`              | 缺失且不适用的 `query/caseSensitive/relativePath/startLine/maxBytes` 补 `null`；缺失 `cursor` 表示首屏并补 `null`                                                                  | 不得补 scopes 或 limit                                                           |
| `search`            | 缺失且不适用的 `relativePath/startLine/maxBytes` 补 `null`；缺失 `cursor` 表示首屏并补 `null`                                                                                      | 不得补 query、caseSensitive、scopes 或 limit                                     |
| `read`              | 缺失且不适用的 `query/caseSensitive/limit` 补 `null`，缺失 scopes 补空数组；初次读取已有 startLine 而缺失 cursor 时补 `null`；分页继续已有非空 cursor 而缺失 startLine 时补 `null` | 不得补 relativePath、maxBytes、初次读取缺失的 startLine，或替模型选择分页 Cursor |

除此以外一律拒绝：

- 不删除 unknown 字段；
- 不做字符串/数字/布尔类型转换；
- 不裁剪或重写 ID、Query、Path；
- 不将别名改成合法 Capability/Resource ID；
- 不默认 Scope、Limit、Query、Path、Start Line、Max Bytes 或非空 Cursor；
- 不把多个操作合并、拆分、排序或丢弃；
- 不在失败后自动发起第二个 Tool Round或第二次修复模型请求。

原始 Provider Arguments 的 Hash/Bytes 必须在规范化前计算并继续写入 Task Store；规范化对象只存在于当前 Turn 内存，且仍须完整通过 canonical Schema 与 Executor 二层语义校验。

`capabilityId` 与 `resourceId` 都必须按原值精确匹配；已知 ID 外包空白也必须拒绝，canonical validator 不得通过 `trim()` 将其静默变成合法 ID。

### 5.4 内容无关的受控诊断

参数失败必须至少能在内部与离线 Evidence 中区分：

- JSON parse / non-object；
- wire shape / required field / unknown field / type；
- canonical schema；
- Capability/Resource 语义；
- Canon operation matrix / scope / path / budget / cursor 语义。

诊断只能是固定枚举或稳定错误码，不得包含字段原值、用户 Prompt、小说内容、Query、相对/绝对路径、Capability/Resource ID 原值、Provider 原始响应或 Key。前端继续只显示既有受控自然语言，不增加技术面板。

### 5.5 Agent 语义说明

System addendum 与 Tool description 必须一致说明组合 Tool 的独立空集语义，但不得变成关键词 Router 或“每轮先读 Canon”的强制规则。模型仍自主判断普通创意讨论是否需要 Tool；当用户明确要求以当前工程事实为准时，不得虚构已读取结果。

---

## 6. 允许改动的表面

预期改动限制在：

- `crates/app-core/src/provider_core.rs`：双 Provider wire identity、Provider schema projection 与 Continuation 对称性；
- `crates/app-core/src/canon_context.rs`：Provider-visible schema/description、纯语法规范化入口与 canonical 二层校验衔接；
- `crates/app-core/src/capability_pack.rs`：组合 Tool 的准确 Agent 语义说明；
- `crates/app-core/src/tool_runtime.rs`、`task_store.rs`、`conversation_runtime.rs`：仅在受控诊断分类与原始 Hash/Bytes 顺序确有需要时修改；
- 同模块 Rust 测试、独立 Pre-NR-4 离线 Evidence 脚本与技术提交文档；
- 必要的当前状态文档。

不得为了本 Story 修改 NR-G4 冻结 Manifest/技术提交、增加生产 UI/IPC、切换 Provider endpoint/model、读取真实 Credential 或引入 Canon Write。

---

## 7. 验收条件

### AC01——唯一 Tool 身份与双 Provider wire 映射

双 Provider Definition、流式/非流式 Call、Result Continuation 都使用合法 wire name `novelist_read_context`，入站后只恢复为唯一内部 `name = novelist.read_context, version = 1`。Provider 请求体中不得出现 dotted function name；Provider 入站若返回 dotted internal name、未知 alias 或不对称映射，必须在 Registry/Executor 前拒绝且执行次数为 0；Production Registry 工具数仍为 1。

### AC02——Provider schema 可理解且语义等价

OpenAI strict schema 继续满足所有 object `additionalProperties = false`、所有 properties required、nullable 表示语法可选值。DeepSeek schema/description 严格符合第 5.2 节冻结的 required/optional 集合，并清楚表达独立空集与 list/search/read 字段矩阵。两个投影生成的合法参数进入同一个 canonical DTO 和同一二层安全校验。

### AC03——安全规范化不改变语义

第 5.3 节白名单中的省略形式可被补成 canonical null/empty；原始完整 canonical 形式保持逐项等价。缺少语义必填字段、unknown 字段、非法类型或越权值不得被“修好”。原始 Hash/Bytes 在规范化前计算，规范化值不持久化。

### AC04——Capability-only 成功

选择有效 Capability、optional resources 为 0、Canon operations 为 0 时，Task 必须经过 `queued → running → succeeded`，Executor 恰好执行 1 次，返回对应基础 instruction，Canon I/O 和 Canon Write 均为 0，Attempt 2 输出正常自然语言。

### AC05——空工程 Canon list 成功

在确定性合成空工程中请求列出 `characters`：Task 必须经过 `queued → running → succeeded`，Executor 恰好执行 1 次，Canon list 恰好 1 次并返回合法空集合，Continuation 正常完成。不得出现 `本次回复未生成。` 或未完成尾注，工程文件集合、内容、Hash 和 mtime 不变。

### AC06——完整 list/search/read 保持安全

三类操作的完整 canonical 形式及允许省略形式均有正例；显式 `canonOperations = []` 必须作为 Capability-only 正例。缺失 canonical 顶层字段、未知/重复/9 项资源、未知 Capability、外包空白的已知 Capability/Resource ID、9 项操作、非法 kind、单项操作缺少语义必填字段、互斥字段冲突，以及非法 Scope/Path/Query/Limit/StartLine/MaxBytes/Cursor 均有反例。反例在 Pack Resource/Canon I/O 前失败。

### AC07——失败继续聊天但不伪造成功

参数失败时 Task 可为 `failed`，Executor 与 Canon I/O 为 0；受控失败 Result 进入 Attempt 2，Conversation 可以 `completed`。助手不得声称已经核对工程，也不得把 Task 失败显示成 Conversation 未生成。

### AC08——调用与预算不扩张

普通聊天 Provider 调用为 1；合法 Tool Turn 的主 Agent Attempts 为 2、Tool Execution 为 1、Tool Round 为 1；失败参数不触发 Executor、自动重试、修复模型、第二 Tool Round或后台调用。现有完整请求体字节上限、输出预算和时间预算保持不变；请求体内容只允许本 Story 明列的 wire name、Provider schema projection 与 description 变化。

### AC09——诊断与隐私

失败分类只能产生受控枚举/错误码。Task、Conversation、Memory、App State、日志、Evidence 和 UI 均不得出现原始 Arguments/Result/Continuation、Prompt、小说内容、Query、路径、Provider Payload 或 Credential。既有隐私 canary 必须扩展覆盖规范化前后路径。

### AC10——发布与安全边界不变

Canon Writes、自动 Apply、外部插件加载、新 IPC、新 Event、新 UI 面板、Host 关键词 Router、Provider 自动重试与自动化外部请求全部为 0；DeepSeek endpoint/model 和 OpenAI endpoint/model 不因本修复变化。

### AC11——离线质量门

新增双方 Adapter、参数规范化、成功/失败 Agent Loop 和隐私反例后，`pnpm quality`、锁定离线 Cargo、Debug/Release 构建、桌面 Smoke 与现有 NR-0～NR-3 回归全部通过。自动化只使用合成 Fixture 和 `127.0.0.1`。

### AC12——真实验证与关闭条件

离线实现通过后必须停止。只有获得单独、明确的真实 Provider 验证授权，才可使用专用测试 Credential 与合成测试工程执行第 9 节 Live Matrix。默认关闭路径要求 DeepSeek 与 OpenAI 均成功；在此之前只能写“离线实现完成，Live pending”，不得写“真实兼容已修复”，`PRE-NR4-PTC-001` 不得以 Live-verified 状态关闭，NR-4 入口继续阻断。

上游既有风险豁免路径仍保留：产品负责人可以另行书面确认“接受未完成双 Provider Live 的已知风险”，并明确覆盖 NR-4 进入条件。该决定只能解除指定阶段的入口阻断，Story 状态必须保持 `implemented / live-waived` 或 `live-pending`，不得记为 `completed / live-verified`，也不得把豁免描述为真实兼容成功；NR-4 的形成与实施仍需各自授权。

---

## 8. 离线测试与 Evidence 矩阵

| 层级              | 必须证明                                                                                                                                                                  |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Provider Core     | DeepSeek/OpenAI Definition、stream/non-stream Call、wire alias 入出站、成功/失败 Continuation 对称；请求体无 dotted wire name；dotted inbound 与未知 alias 均在执行前拒绝 |
| Wire Schema       | OpenAI strict 合规；DeepSeek 根对象、resource item 与各操作分支 required/optional 集合精确符合第 5.2 节；双方语义等价而非 JSON 强行相同                                   |
| 参数规范化        | 完整 canonical、允许缺省 null/empty、resource cursor 缺省、Capability-only、空 Canon list 正例                                                                            |
| 参数反例          | malformed/non-object/oversize/unknown/type/enum；未知/重复/9 resources；空白包裹已知 ID；9 operations；各 kind 缺少语义必填或互斥冲突                                     |
| Canon Executor    | 空工程、no-match、missing/binary、分页、取消、Deadline；所有测试 Canon Writes=0                                                                                           |
| Agent Loop        | 成功执行一次并 Continuation；失败零执行仍 Continuation；Attempt 2 再 Tool 仍以 round limit 结束                                                                           |
| Conversation/Task | 双终态独立、活动文案受控、空结果不变成未生成、重开对账不重放 Provider/Tool                                                                                                |
| Privacy           | raw/canonical 参数、Result、Continuation、Prompt、Query、Path、绝对路径、secret canary 均不进入持久化/日志/Evidence/UI                                                    |
| Release           | Production Tools=1，新 IPC/Router/Write=0，完整 `pnpm quality` 通过                                                                                                       |

独立 Evidence 不得改写 `target/qa/nr-306/nr-g4-manifest.json`。若实施获批，应新增 Pre-NR-4 专用 Manifest/Report，并显式保留：

- `nrG4Approved = true`；
- `preNr4ProviderCompatibilityRepairImplemented = true|false`；
- `formalProviderLiveSmokeExecuted = false`，直到另行授权并真实完成；
- `nr4EnteredOrPacketFormed = false`；
- `automatedExternalRequests = 0`；
- `canonWrites = 0`。

---

## 9. 单独授权后的 Live Matrix

正式 Live 复验不是 `pnpm quality` 的组成部分。只有产品负责人单独授权后才执行：

| Provider | 合成场景                                  | 成功标准                                                            |
| -------- | ----------------------------------------- | ------------------------------------------------------------------- |
| DeepSeek | 普通闲聊                                  | 一次主回复、Tool/Canon I/O=0、Conversation completed                |
| DeepSeek | Capability-only                           | Tool Request → running → succeeded → Result → 自然回复；Canon I/O=0 |
| DeepSeek | 预置一份无敏感内容的合成人物 Canon 后读取 | 一次合法 list/read、结果来自合成文件、Canon Writes=0、自然回复完成  |
| OpenAI   | 同上三项                                  | 与 DeepSeek 相同的逻辑结果和安全边界                                |

Live 复验必须使用专用测试 Credential 与合成工程；不得使用用户小说、生产项目或真实隐私内容。Evidence 只记录 Provider、固定模型、场景 ID、调用计数、Task/Conversation 终态、受控错误码、字节数与 SHA-256，不记录文本摘要、Prompt、正文、参数、结果、路径、Key 或原始响应。

任何失败都按真实结果记录，不通过自动重试、改 Prompt 碰概率、临时切模型或切 Beta endpoint 掩盖。

---

## 10. 实施顺序与停止条件

若本 Story 后续获得产品批准和明确实施授权，唯一顺序为：

1. 冻结现有用户观察的内容无关事实，不导入原始小说或 Provider Payload；
2. 为参数失败增加内容无关的分类测试，先证明现有失败边界；
3. 闭合 internal/wire identity 的双 Provider 全路径映射；
4. 实现 Provider schema projection、准确 description 与第 5.3 节纯语法规范化；
5. 完成 Capability-only、空工程 Canon list、完整 list/search/read 及全部反例；
6. 运行完整离线质量门并形成独立 Pre-NR-4 技术提交；
7. 停止，等待单独 Live 授权或书面风险豁免；
8. 获得 Live 授权后执行双 Provider Live Matrix，全部成功后提交 `PRE-NR4-PTC-001` 的 Live-verified 关闭评审；
9. 若产品改走风险豁免，只记录 `implemented / live-waived` 或 `live-pending`，按第 7 节 AC12 保留未验证事实，不把 Story 关闭为 Live-verified；
10. 停止，等待是否形成 NR-4 Story Packet 的独立新决定；风险豁免本身不构成该授权。

任一步失败都不得跳过、弱化 Host 校验、增加重试、扩大工具权限或提前宣称 Story/NR-4 入口通过。

---

## 11. 明确非目标

本 Story 不包含：

- 新增、拆分或重命名内部产品 Tool；
- Canon create/update/upsert/delete、Diff、确认、Undo、冲突解决或 Writer 自动写入；
- NR-4 DTO、来源引用、Apply/Recovery 或任何 `NR-401～NR-406` 实现；
- 强制 Tool Choice、每轮预读 Canon、关键词 Router 或 Host 语义判断；
- 自动重试、修复模型、多 Tool Round、并行 Tool 或后台 Tool；
- DeepSeek Beta endpoint、模型迁移、Provider 设置 UI 或任意 URL；
- 原始参数/结果/Prompt/小说内容日志、调试导出或 Credential 读取 IPC；
- 新窗口、第四栏、Task Center、技术错误卡或右侧 History；
- 修改 NR-G4 技术提交、Manifest、Gate 决定或历史批准状态；
- 形成或进入 NR-4～NR-7 Story Packet。

---

## 12. 官方 Provider 约束依据

- DeepSeek Chat Completions 规定 function name 只能使用字母、数字、下划线和连字符；其非 strict Tool Call 仍要求调用方自行验证模型生成的参数：[DeepSeek Chat Completions](https://api-docs.deepseek.com/api/create-chat-completion/)。
- DeepSeek strict function calling 当前属于 Beta，并要求 `/beta` endpoint；本 Story 不静默采用该路径：[DeepSeek Tool Calls](https://api-docs.deepseek.com/guides/tool_calls/)。
- OpenAI strict function calling 要求每个 object 设置 `additionalProperties = false`，并把所有 properties 标为 required；语法可选字段通过 nullable 表达：[OpenAI Function Calling](https://developers.openai.com/api/docs/guides/function-calling)。

Provider 文档只约束 Adapter 传输表示，不替代 Host 的本地权限、路径、预算、取消和领域语义校验。

---

## 13. 产品决定入口

独立技术评审完成后，产品负责人需要明确决定：

1. 是否批准 `PRE-NR4-PTC-001` 的产品范围；
2. 是否授权按第 10 节顺序实施离线修复；
3. 是否在离线实现通过后另行授权双 Provider Live Matrix。

若不授权 Live，但希望解除 NR-4 默认入口阻断，还必须单独书面确认第 7 节 AC12 定义的风险豁免；沉默、Story 产品批准、离线实施授权或 NR-4 规划均不构成豁免。

未得到明确决定前，本 Story 只处于“已形成并评审”状态；不得实施、不得读取 Credential、不得执行正式 Live Smoke、不得形成或进入 NR-4。
