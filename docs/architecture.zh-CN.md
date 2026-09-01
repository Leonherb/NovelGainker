# NovelGainker 当前架构（NR-1）

## 产品边界

NovelGainker 是 Windows-first、local-first 的 Tauri 2 中文小说写作应用。界面保持三栏：左栏管理工程、文件与 Provider；中栏只承载自然对话；右栏显示真实 Markdown 文件编辑器。启动时只要求新建或打开工程，不要求预先建立人物、世界或章节。

NR-1 在可靠自然对话上增加完整 History、派生 Memory、按阈值压缩和长对话 Context。它仍不加载 Novelist Skill，不读取小说 Canon 作为模型上下文，不调用 Tool，也不由 AI 写入 Markdown。后续能力必须经过新的 Story Packet 和 Gate。

## 运行结构

```text
React UI
  ├─ Project / Markdown editor
  ├─ Provider settings
  └─ Conversation view
          │ 3 typed commands + 1 event
          ▼
Rust Tauri Host
  ├─ Project/session registry
  ├─ Conversation Store（完整可见 History）
  ├─ 派生 Memory Store / Context Assembler
  ├─ 可选一次 Compactor → 一次流式主回复
  ├─ Provider adapters / CredentialVault
  └─ atomic persistence / recovery / validation kernels
```

生产 Command 固定为 24 条，其中 Conversation Command 固定为 `conversation_session_load`、`conversation_turn_start`、`conversation_turn_cancel`。流式事件为 `conversation-turn-event`。Command Manifest、Handler、Permission、Capability 与所有生产 TypeScript Client 必须严格相等。

## History、Memory 与 Context

工程内完整可见对话保存在 `.novelgainker/conversation/session.json`，始终是 Conversation 事实源。Host 在任何 Provider attempt 前先保存用户消息与 `running`；流式字符只保留在进程内，终态时一次保存完整结果。遗留 `running` 在重新打开时变成 `本次回复未生成。`，不会自动重试。

派生 Memory 独立保存在 `.novelgainker/memory/state.json`，绑定 Project/Session、连续的 UTF-8 安全 Segment Cursor 区间和覆盖 History 的源 Hash。文件缺失、损坏、绑定错误、区间无效或 Hash 不一致时直接丢弃，不改变 History 或小说 Markdown。Memory 不是 Canon，也不生成 Conversation 气泡。

主回复前，Host 依次组装固定系统指令、有效 Memory、Memory 之后的最近精确 History 和只出现一次的当前用户消息。最近精确 History 与摘要冲突时以前者为准。失败/取消 Turn 只提供真实非空输出及其未完成状态；`本次回复未生成。` 和 UI 附加提示不进入语义摘要。

## Provider 与冻结 Profile

预测 Context 低于 64 KiB 时，Compactor attempt 为 0，主回复最多 1 次。达到阈值且存在可压缩输入时，先最多进行 1 次非流式 Compactor，再最多进行 1 次流式主回复。两条路径共享 Cancellation Token，均可中断在途 HTTP、每条严格 1 attempt、零重试。Compactor 普通失败不会产生失败气泡，Host 保留旧 Memory 并用受限最近 History 继续；Compactor 阶段停止则取消整个 Turn，不启动主回复。

NR-G2 技术提交冻结：24 KiB 最近精确尾部、6 KiB Segment、72 KiB Compactor 输入、8 KiB Backfill 预留、12 KiB Memory 摘要、1,024 Compactor 输出 tokens、4,096 主回复输出 tokens、5,120 累计请求输出上限。Provider 硬边界保持 64 条消息、96 KiB 请求、32 KiB 输入单消息和 8 MiB 响应。

## Event 对账与关闭

Event 只负责低延迟 Delta 和终态通知，Store 是最终事实源。UI 完成 listener 注册后才允许发送；活动 Turn 使用单一非重叠的 1000ms Session reconciliation。Event 丢失、延迟、重复或乱序不会产生第二个 Provider Turn 或永久活动状态。

原始窗口关闭请求只检查手动 Markdown 是否未保存。只有这种情况显示 `保存 / 放弃 / 取消`。确认关窗后直接销毁窗口，不等待项目关闭、Compactor、主回复或 Conversation flush；窗口销毁取消共享 Token，遗留 `running` 在下次打开时对账。Compactor 半成品永不提交。

## Project Metadata 与一次性切换

Project Metadata 当前通用格式为 `schemaVersion = 2`。v1 表示可丢弃旧应用状态，只在 writer lease、规范化项目根和递归 reparse 检查通过后重置一次。小说 Markdown、素材、图片、Provider 设置、UI 偏好、无关项目与 Windows 凭据不变。

## Gate 状态

NR-G1 已获产品批准。NR-101～NR-106 已完成，当前停止在 `NR-G2-technical-submission`，`nrG2Approved = false`。NR-G2 只以本地 Loopback、长 History/Segment 规模测试、预算检查、发布/IPC 闭包与完整 `pnpm quality` 形成技术证据；未执行真实 Provider Live Smoke，不声明真实模型摘要语义质量，不进入 NR-2。
