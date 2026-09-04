# NovelGainker NR-2 Host Agent Loop / Tool Request-Result Story Packet Draft 1

> 文档状态：Story Packet Draft 1 / 待评审  
> 形成日期：2026-09-02  
> 当前实现基线：NR-1，NR-G2 已批准并通过  
> 上位计划：NR-2～NR-7 Capability Delivery Plan Draft 2 已批准  
> 本 Packet 范围：`NR-201 → NR-206`，退出 Gate 为 `NR-G3`  
> 当前授权：仅形成并评审本 Packet  
> 明确未授权：NR-2 实施、真实 Provider Live Smoke、NR-3～NR-7 Story Packet

## 1. BMAD Routing Brief

### Scope

- Packet type：`execution-packet`；
- Project type：Windows-first、Local-first Tauri 2 小说创作桌面应用；
- Project level：4；
- Runtime context：Codex；
- Main constraint：在不恢复 Control JSON 的前提下建立可执行、可取消、可审计的 Agent Tool 通道；
- Confidence：高。

### Current phase

- 已通过 NR-G2 后的 NR-2 Story Packet 评审；
- 代码仍是 NR-1 实现基线，尚未进入 NR-201。

### Recommended next move

- 评审本 Draft 1 的 `NR2-D1-R01～R06`；
- 关闭阻断后形成 Draft 2；
- 只有 Packet 最终获批且明确授权实施，才可开始 NR-201。

### What not to do yet

- 不实现任何生产代码；
- 不加载 Novelist Capability Pack；
- 不读取或修改 Canon；
- 不执行真实 Provider Live Smoke；
- 不形成 NR-3～NR-7 Story Packet。

## 2. 产品问题与阶段结果

NR-1 只能完成一次自然语言主回复。Provider Adapter 虽能把 DeepSeek 的 `finish_reason = tool_calls` 归一化为 `FinishReason::ToolCall`，但当前归一化响应只有文本、Usage 和 Finish Reason，没有可执行的 Tool Call 载荷；OpenAI Responses 的流式解析也只处理文本、Usage 和终态。Conversation Store 固定为每 Turn 一条用户消息加一条助手消息，尚无独立 Task 状态。

NR-2 的目标不是实现小说领域能力，而是建立以下最小闭环：

```text
用户自然发送
  ↓
Host 先保存用户消息 + running
  ↓
Provider 返回自然文本或 Provider-native Tool Call
  ↓（有 Tool Call 时）
Host 严格校验 → 独立 Task Store → 执行允许的 Tool
  ↓
Host 把 Tool Result 通过 Provider-native 续接格式交回同一 Provider
  ↓
Provider 返回最终自然语言
  ↓
Conversation 与 Task 分别落终态
```

用户仍只在中栏自然聊天。Tool Request/Result 是 Host 内部机器通道，不是可见回复格式，也不要求用户输入 Anchor、关键词、任务卡或 JSON。

## 3. NR-2 不变量

1. **自然对话优先**：普通闲聊、小说讨论和任务发布进入同一发送路径；生产 Tool Registry 为空时，行为必须与 NR-1 普通回复一致。
2. **Provider-native Tool Calling**：只接受 Adapter 从 Provider 原生结构解析出的 Tool Call；不得从自然语言、Markdown Fence、XML、正则或回复尾部 JSON 猜测动作。
3. **Host 唯一执行**：模型只能请求；Host 根据当前 Registry、版本、Schema、预算、取消和权限决定执行或拒绝。
4. **双终态独立**：Task 失败不自动等于 Conversation 失败；已经产生的自然语言文本按 NR-0/NR-1 规则保留。
5. **零领域副作用**：NR-2 没有生产 Canon、文件、网络、Shell、Process、Skill 或发布 Tool。
6. **测试 Tool 不进入发布面**：`test.echo`、`test.fail`、`test.block_until_cancelled` 只存在于 Rust/Loopback/UI Fixture；发布扫描必须证明生产 Registry 为空且发布文件不含这些 ID。
7. **一个 Tool Round**：每 Turn 最多执行一项 Tool；Tool Result 后的第二次 Agent Attempt 强制不再调用 Tool。多调用、嵌套调用和并行调用留给后续阶段。
8. **零自动重试**：Compactor、初始 Agent Attempt、Tool 和续接 Agent Attempt 均不自动重试，不调用修复模型。
9. **瞬态 Provider Continuation**：Provider 为续接所需的 Call Item、Call ID 或隐藏状态只可在本 Turn 内有界保留；不得写入 Conversation、Memory、Task Store、日志或 App State。
10. **现有 UI 结构不扩张**：Task 只在中栏当前 Turn 内显示简短自然语言活动/结果，不增加第四栏、Task Center、右侧 History 或技术卡片。

## 4. 当前基线与改动表面

| 基线                                | 当前状态                                       | NR-2 允许的最小变化                                             |
| ----------------------------------- | ---------------------------------------------- | --------------------------------------------------------------- |
| `NormalizedRequest`                 | System + 文本消息 + `max_output_tokens`        | 增加有界 Tool Definition 和一次续接所需的归一化输入项           |
| `NormalizedResponse` / Stream Event | 文本、Usage、Finish Reason                     | 增加有界 Tool Call 聚合结果；原始 Provider JSON 不外泄          |
| Conversation Runtime                | Compactor 0/1 + 主回复 1                       | 主回复扩展为 Agent Attempt 1 + 可选 Tool + Agent Attempt 2      |
| Conversation Store                  | 每 Turn 用户 1 条 + 助手 1 条                  | 结构继续保持；Tool 数据不进入可见消息                           |
| Memory                              | 五栏目派生摘要                                 | 继续不包含 Tool Request/Result 或 Task 技术状态                 |
| IPC                                 | 24 Commands，Conversation 3 Commands + 1 Event | Command 数量保持；Load DTO 与 Event 增加严格 Task Activity 投影 |
| 三栏 UI                             | 中栏回复/停止/终态                             | 当前助手 Turn 内增加简短活动/任务结果文本                       |

NR-2 不重写 Provider Preset、CredentialVault、Project Metadata v2、History/Memory、文件编辑、安全 Kernel 或关窗规则。

## 5. Tool 合同

### 5.1 归一化定义

```text
ToolDefinition
  name                 固定允许字符，最长 64
  version              正整数，由当前请求快照绑定
  description          非空、有界、仅用于模型语义选择
  inputSchema          受支持的 JSON Schema 子集
  displayLabel         简体中文、非技术化、只用于 UI 投影

ProviderToolCall
  callId               Provider 返回的本轮唯一关联 ID
  name                 必须命中当前请求的 ToolDefinition
  argumentsJson        UTF-8 JSON Object，解析后再做严格 Schema 校验

ToolResult
  callId               与请求严格相等
  name + version       与请求快照严格相等
  status               succeeded | failed | cancelled
  outputJson           成功时的有界 JSON Object
  errorCode            失败/取消时的稳定非敏感代码
```

本地 Schema 校验始终是权威边界。即使 Provider 支持 `strict`，Host 也不能信任模型输出已经有效。Schema 禁止远程引用、任意代码、路径解析或运行时扩展；参数必须是单个 JSON Object。

### 5.2 Draft 1 候选大小边界

以下数值在 Draft 1 中作为 NR-205/NR-206 的候选 Profile，而不是未经规模测试的最终冻结值：

| 项目                              | 候选上限 |
| --------------------------------- | -------: |
| 每次请求 Tool Definitions         |       16 |
| 单个 Tool Description             |    2 KiB |
| 单个 Input Schema                 |    8 KiB |
| 全部 Tool Definitions 序列化总量  |   24 KiB |
| Tool Arguments                    |   16 KiB |
| Tool Result                       |   24 KiB |
| 单个 Provider 输出中的 Tool Calls |        1 |
| 每 Turn Tool Executions           |        1 |
| 每 Turn Tool Rounds               |        1 |

Adapter 生成最终请求体后必须再次按 UTF-8 字节计算完整序列化 Body，继续服从现有 96 KiB Request Hard Limit；不能只计算文本 Message 而忽略 Tool Schema、Tool Call 或 Tool Result 包装。

### 5.3 Registry 与 Executor

- Registry 是 Host 内的不可变请求快照；模型不能注册、重命名或修改 Tool。
- 执行前按 `name + version` 查找；未知、错版本、重复 Call ID、Schema 失败、越界和已取消请求均为零执行。
- Executor 接收当前 Project/Session/Turn 身份与共享 Cancellation Token，但不接收 API Key。
- NR-2 正式发布的生产 Registry 必须为空。
- 确定性合成 Tool 只通过测试依赖注入；不得存在隐藏 Feature Flag、环境变量或生产调试入口来启用它们。

## 6. 双 Provider Adapter 合同

### 6.1 DeepSeek Chat Completions

Tool-enabled 请求使用官方 Chat Completions 的 `tools`/`tool_choice`，解析 Assistant `tool_calls`，并用关联 `tool_call_id` 的 Tool Message 续接。Host 必须完整验证 `name` 和 JSON Arguments。官方文档明确说明模型只产生调用请求，具体函数由调用方执行。

为避免把未公开的推理内容持久化或暴露，NR-2 候选方案规定：**仅 Tool-enabled Attempt 显式使用 DeepSeek non-thinking mode**。无 Tool Definitions 的普通对话保持 NR-1 现状。若评审不接受该边界，Draft 2 必须另行设计有界、仅内存、不可持久化的 `reasoning_content` 续接合同。

### 6.2 OpenAI Responses

Tool-enabled 请求使用 Responses API Function Tool；流式 Adapter 聚合 `function_call`/arguments 事件，以 `call_id` 建立 Tool Result 关联，并在第二次 Attempt 中加入 `function_call_output`。`store` 继续固定为 `false`，不使用 Provider 端 Conversation State。

Responses API 在手动管理 Context 时可能要求把本次续接所需的输出 Item 一并带入下一请求。NR-2 只允许在当前 Turn 内保留 Adapter 归一化后的最小 Provider Continuation：Function Call Item、Call ID，以及协议要求的有界 opaque continuation item。它们不得序列化到任何本地 Store、IPC、Event、日志或 Evidence；窗口关闭或进程退出后不恢复、不重放。

### 6.3 共同解析规则

- Tool Arguments Delta 在 Rust Adapter 内有界聚合，不逐 Delta 发往前端。
- Tool Call 完整前不得执行；响应终止、缺少 Call ID/Name、重复字段、非法 UTF-8、越界、乱序或多 Call 时执行次数为 0。
- Provider 返回文本与 Tool Call 时，真实非空文本继续进入当前助手回复缓冲；Tool JSON 不进入文本。
- Tool Result 后的第二次 Attempt 不提供 Tool Definitions，并强制 `tool_choice = none` 的等价语义。
- 第二次 Attempt 再返回 Tool Call 属于 `agent.tool_round_limit`，不执行并按异常终态保留已生成文本。

## 7. Task Store 与 Conversation 投影

### 7.1 Task Store

新根固定为：

```text
.novelgainker/tasks/session.json
```

它是项目绑定、原子替换、严格 `schemaVersion = 1` 的运行状态，不是 Canon，也不是 Conversation History。每条记录最少保存：

- `taskId`、Project ID、Conversation Session ID、Turn ID；
- `callIdHash`、Tool Name、Tool Version；
- `queued / running / succeeded / failed / cancelled`；
- `startedAtMs / updatedAtMs`；
- 稳定 `errorCode` 与简短 `displayLabel`；
- Arguments/Result 的 SHA-256 与字节数。

Task Store 不保存原始 Provider Body、隐藏 Prompt、opaque continuation、完整 Arguments、完整 Result、API Key、绝对路径或小说正文。NR-4 若需要事务引用或 Undo，必须通过新的 Packet 扩展，不能在 NR-2 偷渡。

启动发现遗留 `queued/running` 时原子转为 `cancelled`，错误码为 `runtime.interrupted`；不自动恢复 Tool、不发起 Provider、不重复用户消息。

### 7.2 Conversation Store 保持稳定

- 用户消息与 `running` 仍在任何 Provider Attempt 前保存。
- 每 Turn 仍只有一条用户消息和一条助手自然语言消息。
- Tool Request、Tool Result 和 Task JSON 不进入 Conversation Message。
- Memory Segment 只读取可见用户/助手文本与 Conversation 终态，不读取 Task Store。
- 有文本时按既有规则保留；无文本异常时仍显示 `本次回复未生成。`。

### 7.3 IPC 与 UI 投影

不新增 Tauri Command 或 Event Channel：

- `conversation_session_load` 由 Host 组合 Conversation Store 与 Task Store，返回版本化 Runtime View；
- `conversation-turn-event` 增加严格 `taskActivity` Variant，只包含 Turn/Task ID、状态和简短 Display Label；
- `conversation_turn_start` 与 `conversation_turn_cancel` 保持现有用户调用方式；
- 生产 Command 仍为 24 条，其中 Conversation Command 仍为 3 条。

中栏只允许显示：

```text
正在处理……
已完成相关操作。
相关操作未完成，回复仍可继续。
```

实际文案来自受控本地映射，不直接显示 Tool Name、Arguments、Result、错误堆栈或 JSON。活动内容使用 `aria-live="polite"`，不能抢占输入焦点。终态后活动信息收敛在对应助手 Turn 内，不产生独立气泡或卡片。

## 8. 单次 Turn 状态机

### 8.1 无 Tool

1. 保存用户消息与 `running`；
2. 按 NR-1 执行 0/1 次 Compactor；
3. Agent Attempt 1 返回自然文本并正常结束；
4. Tool Execution = 0，行为与 NR-1 相同。

### 8.2 一个合法 Tool Call

1. 保存用户消息与 `running`；
2. 按 NR-1 执行 0/1 次 Compactor；
3. Agent Attempt 1 返回 0/1 个 Tool Call，可同时产生自然文本；
4. Host 先原子创建 `queued` Task，再更新为 `running`；
5. Executor 最多执行一次并原子保存 Task 终态；
6. 无论 Tool 成功、失败或被策略拒绝，只要 Call ID 足以安全关联，Host 都以规范化 Tool Result 发起最多一次 Agent Attempt 2，让 AI 用自然语言继续回复；
7. Attempt 2 强制无 Tool，随后 Conversation 独立落终态。

### 8.3 非法或越界 Tool Call

- 多 Call、无 Call ID、Arguments 无法有界完成或 Provider 协议损坏：执行 0 次，不发修复请求；
- 若已有非空文本，保存文本并显示既有未完成提示；否则显示未生成；
- 未形成可安全关联的 Call 时不创建虚假 Task；已形成关联但 Schema/Registry 拒绝时记录 `failed` Task，并可按第 8.2 节进行一次自然语言续接；
- 不把非法 Tool JSON回显给用户。

### 8.4 文本与双终态

| 情况                                 | Conversation                           | Task      |
| ------------------------------------ | -------------------------------------- | --------- |
| Attempt 1 无 Tool、正常文本          | completed                              | 无 Task   |
| Tool succeeded、Attempt 2 正常文本   | completed                              | succeeded |
| Tool failed、Attempt 2 正常解释      | completed                              | failed    |
| Tool succeeded、Attempt 2 零文本失败 | not_generated 或按已有部分文本规则终态 | succeeded |
| Tool running 时用户停止              | cancelled / 已有文本保留               | cancelled |
| Tool 已成功后用户停止 Attempt 2      | cancelled / 已有文本保留               | succeeded |

Task 状态绝不能反向覆盖 Conversation 文本或终态。

## 9. 调用、输出与取消预算

### 9.1 结构上限

- Compactor HTTP Attempt：0 或 1；
- Agent HTTP Attempts：无 Tool 时 1，有 Tool 时最多 2；
- 单 Turn Provider HTTP Attempts：最多 3；
- Tool Executions：0 或 1；
- Tool Rounds：0 或 1；
- 自动重试、修复模型、后台 Tool、并行 Tool：0。

两次 Agent Attempt 都通过 `stream-once-cancellable` 等价路径；Tool Executor 使用同一个 Turn Cancellation Token。任何停止都不得等待 Provider 或 Tool 自身超时才返回。

### 9.2 Draft 1 输出预算建议

为不降低普通对话现有 4,096-token 主回复能力，Draft 1 建议以以下候选 Profile 开始规模验证：

| 请求                 | 候选 Requested Output |
| -------------------- | --------------------: |
| Compactor            |          1,024 tokens |
| Agent Attempt 1      |          4,096 tokens |
| Agent Attempt 2      |          4,096 tokens |
| 单 Turn 累计声明上限 |          9,216 tokens |

累计边界约束各请求声明值之和，不按实际 Usage 动态回收，也不能因为前一次提前结束而提高后一次上限。NR-205 实现候选 Profile，NR-206 使用请求体捕获、长输入和 Tool Result 极值测试决定最终值，并在 NR-G3 Manifest 冻结。

### 9.3 停止、关闭与重开

- 停止在 Attempt 1：取消 HTTP；不执行尚未完整形成的 Tool。
- 停止在 Tool：触发共享 Token；Task 转 cancelled；不发 Attempt 2。
- 停止在 Attempt 2：取消 HTTP；已完成 Task 不回滚，真实非空文本保留。
- 原始窗口关闭请求仍先处理未保存 Markdown；关闭未确认不取消 Turn。
- 确认关闭后取消在途 Provider/Tool 并立即退出，不等待 Task/Conversation flush；重开时遗留状态按各自 Store 对账。
- 不持久化 Provider Continuation，因此中断后不自动续传、恢复或重放 Tool Round。

## 10. Story 顺序

### NR-201——Tool Request/Result 与 Task Store 合同

交付：

- 定义 ToolDefinition、ProviderToolCall、ToolResult、TaskRecord 和 Runtime View；
- 建立 `.novelgainker/tasks/session.json` 的严格 Schema、项目绑定、原子写入、Hash 与遗留 running 对账；
- 扩展请求大小核算，使 Tool Schema/Call/Result 进入完整序列化 Body 上限。

验收：

- Conversation/Memory/Canon 字节不因 Task Store 写入而改变；
- 原始 Arguments、Result、Provider Body、opaque continuation、Key 和绝对路径不落盘；
- 未知字段、非法 ID/状态、错 Project/Session/Turn、重复 Call ID 和越界均拒绝；
- 生产 Registry 仍为空，Tool Executions = 0。

### NR-202——双 Provider 原生 Tool-Call 适配

交付：

- DeepSeek Chat 原生 `tools/tool_calls/tool_call_id` 请求、流式聚合与续接映射；
- OpenAI Responses Function Tool、Function Call Arguments、`function_call_output` 与有界瞬态 Continuation 映射；
- Tool-enabled DeepSeek non-thinking 候选路径；
- 四条请求体捕获：双方各自 Initial + Continuation。

验收：

- 不解析可见文本中的 JSON/Markdown/XML；
- Args Delta 乱序、重复、缺失、越界和多 Call 全部零执行；
- Tool Result 与 Call ID 严格配对，Attempt 2 不再允许 Tool；
- 普通无 Tool 请求体与 NR-G2 冻结行为保持一致；自动化外部请求为 0。

### NR-203——有界 Host Agent Loop 与合成 Executor

交付：

- 实现 Attempt 1 → 可选单 Tool → Attempt 2 状态机；
- Registry/Executor 依赖注入和共享 Cancellation Token；
- 只在测试构建注入 success/failure/block-until-cancelled 合成 Tool；
- Tool 失败可作为 Result 返回模型，由 Attempt 2 形成自然语言回复。

验收：

- Tool 最多执行一次；无 Tool 时 Agent Attempt 仍为一次；
- Registry/Schema/预算失败为零副作用；
- Task 失败不自动导致 Conversation 失败；
- Release 递归扫描证明合成 Tool ID 和启用入口不存在于生产发布面。

### NR-204——Conversation/Task 双终态与中栏内联投影

交付：

- `conversation_session_load` 组合 Runtime View，Event 增加 Task Activity；不新增 Command/Channel；
- 保持 Conversation Store 的一用户一助手可见结构；
- 在当前 Turn 内显示简短活动/结果，重开后从 Store 恢复终态投影；
- 失败、部分文本和零文本行为沿用既有文案。

验收：

- IPC 仍为 24/3，所有 Manifest/Handler/Permission/Capability/生产 Client 严格闭包；
- 无第四栏、Task Center、右侧 History、Tool 卡或 JSON；
- `aria-live`、键盘输入和停止按钮无回归；
- Task/Conversation 终态矩阵全部覆盖。

### NR-205——取消、幂等、恢复与预算

交付：

- 冻结一次 Tool Round、一次 Tool Execution、最多两次 Agent Attempts 和零重试；
- 实现 Call ID 去重、Task 状态 CAS、共享取消、窗口关闭和重开对账；
- 实现候选输出/字节 Profile 和完整序列化请求校验。

验收：

- 每个可取消阶段在确定性测试中实际中断，不依赖请求超时；
- 任何重复 Event/Call/Result 不重复执行 Tool 或用户消息；
- Tool 成功后 Attempt 2 失败不会回滚或伪造 Task；
- 没有后台继续、断点续传、Provider Cursor 或自动重放。

### NR-206——NR-2 规模回归与 NR-G3 技术提交

交付：

- 覆盖双 Provider Initial/Continuation、极限 Schema/Args/Result、长 History/Memory、Compactor + Tool、失败/取消/重开和 UI；
- 比较输出/请求 Candidate Profile，冻结 NR-2 调用和累计预算；
- 增加 Release、零领域副作用、零合成 Tool 泄漏和机器 Claim 门；
- 运行完整 `pnpm quality`，形成 NR-G3 Manifest 与技术报告后停止。

验收：

- Provider Attempts、Tool Executions、输出预算和序列化字节均有机器证据；
- `Skill Calls = 0`、`Canon Reads = 0`、`Canon Writes = 0`、`External Tool Calls = 0`；
- `automatedExternalRequests = 0`、`realProviderLiveSmokeExecuted = false`；
- `nrG3Approved = false`，不形成或进入 NR-3。

## 11. NR-G3 Gate

NR-G3 技术提交必须证明：

1. 双 Provider 原生 Tool Call 与 Result 续接被 Adapter 正确归一化；
2. 可见自然语言与机器 Tool 数据物理分离，不存在 Control JSON 修复循环；
3. Host 只执行 Registry 允许且本地 Schema 通过的单一调用；
4. Production Registry 为空，合成 Tool 不进入发布面；
5. Task Store 独立、原子、有界、无原始敏感载荷；
6. Conversation/Task 终态独立，任何非空文本保留；
7. Stop/Close/Restart 在三个阶段均不挂起、不重放、不重复副作用；
8. 完整请求、Provider Attempt、Tool Execution 与累计输出预算全部受本地边界约束；
9. IPC 保持 24/3，三栏 UI 无新增任务面板；
10. `pnpm quality`、离线 Loopback、Debug/Release Build、Windows 本地 Smoke 和发布边界全部通过；
11. 没有加载 Skill、读取/修改 Canon、执行真实 Provider Live Smoke或进入 NR-3。

技术提交只形成证据并停止，不自动批准 NR-G3。

## 12. 明确不包含

- Novelist Capability Pack、Skill Registry、Skill Instruction 或语义能力调用；
- Canon Catalog、角色/世界/设定/章节读取；
- Markdown Create/Update/Upsert、Diff、确认、Undo 或冲突处理；
- 任何生产 Tool，包括无意义的 echo、clock、capabilities 或 debug Tool；
- 多 Tool、并行 Tool、多 Round Agent Loop 或后台 Task；
- Anchor、Context Mark、Proposal、WritePlan、关键词 Router 或可见 Control JSON；
- Provider Cursor、断点续传、自动重试、后台恢复或 Tool 重放；
- 新窗口、第四栏、Task Center、右侧 History 或技术协议 UI；
- 真实 Provider Live Smoke；
- NR-3～NR-7 的任何 Packet 或实现。

## 13. Draft 1 待评审决策

| 决策                       | Draft 1 建议                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `NR2-D1-R01` Tool 通道     | 只使用 Provider-native Function Tool；禁止从可见自然语言解析动作                                                         |
| `NR2-D1-R02` 生产 Registry | NR-2 生产 Registry 为空；合成 Tool 仅测试注入且发布扫描必须为零                                                          |
| `NR2-D1-R03` Task/IPC      | Task Store 独立；Conversation Store 不变；通过现有 Load/Event 投影，IPC 保持 24/3                                        |
| `NR2-D1-R04` Loop/预算     | 每 Turn 最多一项 Tool、一个 Round、两次 Agent Attempts；候选累计 Requested Output 为 9,216 tokens，NR-206 规模测试后冻结 |
| `NR2-D1-R05` Provider 续接 | DeepSeek Tool Attempt 使用 non-thinking；OpenAI 必需 Continuation 只在当前 Turn 有界保留且永不持久化                     |
| `NR2-D1-R06` 双终态/UI     | Task 与 Conversation 终态独立；中栏当前 Turn 只显示受控自然语言活动/结果，无新面板或技术卡片                             |

## 14. 评审与批准 Gate

当前状态：`NR-2 Story Packet Draft 1 review`。

本轮评审应逐项确认或修订 `NR2-D1-R01～R06`。Draft 1 获得评审结论不等于实施授权；如有修改，先形成 Draft 2。只有产品负责人明确批准 Final Packet，并同时授权按 Story 顺序实施，才能开始 NR-201。

当前固定状态：

```yaml
capabilityPlanStatus: approved
nr2PacketStatus: draft_1_review
nr2ImplementationAuthorized: false
realProviderLiveSmokeAuthorized: false
nr3PacketAuthorized: false
nextAction: review-NR2-D1-R01-through-R06
```

## 15. 协议依据

- OpenAI Responses API 的 Function Tool 使用 `function_call`、`call_id` 与 `function_call_output`，手动管理 Context 时需要把协议要求的相关 Item 传入后续请求：[OpenAI Responses streaming reference](https://platform.openai.com/docs/api-reference/responses-streaming/response/refusal?lang=python)。
- DeepSeek Chat Completions 原生支持 `tools`、Assistant `tool_calls` 与带 `tool_call_id` 的 Tool Message；具体函数由调用方执行：[DeepSeek Tool Calls guide](https://api-docs.deepseek.com/guides/tool_calls/)、[DeepSeek Chat Completions API](https://api-docs.deepseek.com/api/create-chat-completion/)。
