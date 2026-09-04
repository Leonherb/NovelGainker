# NovelGainker 当前架构（NR-2）

## 产品边界

NovelGainker 是 Windows-first、local-first 的 Tauri 2 中文小说写作应用。界面保持三栏：左栏管理工程、文件与 Provider；中栏只承载自然对话；右栏显示真实 Markdown 编辑器。启动时只要求新建或打开工程，不要求预先建立人物、世界或章节。

NR-2 在 NR-1 的完整 History、派生 Memory 和长对话 Context 上增加 Host Agent Loop / Tool Request-Result 基础设施。生产 Tool Registry 仍为空，因此当前版本不会执行 Tool，不加载 Novelist Skill，不读取小说 Canon 作为模型上下文，也不由 AI 写入 Markdown。

## 运行结构

```text
React 三栏 UI
  └─ Conversation View（自然语言 + 当前 Turn 内联 Task 状态）
       │ 3 typed commands + conversation-turn-event
       ▼
Rust Tauri Host
  ├─ Conversation Store（完整可见 History）
  ├─ Memory Store / Context Assembler / 0..1 Compactor
  ├─ Agent Attempt 1 → 0..1 Tool → 0..1 Agent Attempt 2
  ├─ 空生产 Tool Registry / 严格 Tool Contract
  ├─ 独立 Task Store / Revision CAS / recovery
  ├─ DeepSeek 与 OpenAI Provider-native adapters
  └─ CredentialVault / atomic persistence / validation kernels
```

生产 Command 固定为 24 条；Conversation Command 仍只有 `conversation_session_load`、`conversation_turn_start`、`conversation_turn_cancel`。Load 返回独立 `ConversationRuntimeView`，其中嵌套原样 Conversation 与有界 Task Activity。Event Channel 不新增，只扩展严格 `taskActivity` Variant。

## Conversation、Memory 与 Task

`.novelgainker/conversation/session.json` 是完整可见对话事实源，每 Turn 仍只有一条用户消息和一条助手自然语言消息。Tool Request、Result、Arguments、Continuation 和 Task JSON 不进入 Conversation。

`.novelgainker/memory/state.json` 是可丢弃派生摘要。它只读取可见 History，不读取 Task Store；固定五栏目、Segment Cursor、源 Hash、阈值与回填规则保持 NR-G2 不变。

`.novelgainker/tasks/session.json` 是独立、工程绑定、原子替换的运行状态。记录只包含关联 Hash、字节数、受控标签、稳定错误码和 `queued/running/succeeded/failed/cancelled`；Revision 从 1 单调增加，终态不可回退。重开时遗留 `queued/running` 只转为 `cancelled + runtime.interrupted`，不重放 Tool 或 Provider。

## Provider-native Agent Loop

普通无 Tool Registry 请求继续走 NR-1 单次主回复路径。存在测试注入 Registry 时，Adapter 可从 Provider 原生结构归一化一个 Tool Call：DeepSeek 使用 Chat `tools/tool_calls/tool_call_id`，OpenAI Responses 使用 Function Tool、类型化 Output Item 白名单和 `function_call_output`。Host 不解析自然语言、Markdown、XML 或尾部 JSON 来猜测动作。

合法调用先通过本地定义、严格 Schema、参数、Continuation、完整 96 KiB 请求体和取消检查，再由 Host 唯一执行。每 Turn 最多一个 Tool、一个 Round、两次 Agent Attempt；第二次 Attempt 不提供 Tool，若仍返回 Tool Call 则零执行并以 `agent.tool_round_limit` 终止。Provider Continuation 仅存在于当前 Turn 内存，不写 Store、IPC、日志或 Evidence。

## 预算、取消与 UI

Compactor 最多 1 次、Agent HTTP Attempt 最多 2 次，单 Turn Provider HTTP Attempt 最多 3 次，全部零自动重试。请求输出预算固定为 `1024 + 4096 + 4096 = 9216` tokens。Tool Deadline 为 30 秒，用户停止后的 Executor 取消确认不超过 2 秒。

Task 与 Conversation 终态独立：Task 失败可由 Attempt 2 解释为自然语言；Task 已成功而 Attempt 2 失败时 Task 不回滚。任何非空助手文本都保留。中栏只显示受控本地映射：`正在处理……`、`已完成相关操作。`、`相关操作未完成。`；不显示 Tool 名称、参数、结果、JSON 或技术卡片。活动状态使用唯一 `aria-live="polite"`，不抢输入焦点。

Event 是低延迟路径，Store 是事实源。Snapshot/Event 仅按更大的 `taskRevision` 合并，相等冲突或更旧 Revision 被拒绝，终态不会被迟到的活动态覆盖。

## Project 与关闭边界

Project Metadata 仍为严格 `schemaVersion = 2`。原始关闭请求只会被未保存的手动 Markdown 阻断，并显示 `保存 / 放弃 / 取消`；确认关闭后取消活动 Turn，不等待 Provider 或 Tool 自身超时。重开只对账，不自动续跑或重复副作用。

## Gate 状态

NR-201～NR-206 与 NR-G3-R01～R03 Remediation 已完成，NR-G3 已获产品批准，当前实现仍停止于通过后的 Gate。Task Store 已闭合 Result 严格配对和非法恢复拒绝；测试 Registry 只允许已验证的具体 Executor；同一核心 Turn 编排已覆盖 Compactor + Tool 三次调用和新 Memory 消费。两秒取消保证仅适用于封闭 Registry 中经验证的实现，不代表 Host 能强制终止任意进程内代码。自动化仅使用本地 Loopback、离线 Cargo、Debug/Release Build 和本地窗口 Smoke；真实 Provider Live Smoke 未执行。NR3-D2-R01～R06 与 FR01～FR02 已在 NR-3 Story Packet Draft 2 Final 中获产品批准，当前停止于 Packet 批准 Gate；NR-301～NR-306 未获实施授权，所以当前运行时仍不加载 Skill、不读写 Canon；NR-4～NR-7 Story Packet 未形成。
