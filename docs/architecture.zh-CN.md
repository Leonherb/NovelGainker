# NovelGainker 当前架构（NR-0）

## 产品边界

NovelGainker 是 Windows-first、local-first 的 Tauri 2 中文小说写作应用。界面保持三栏：左栏管理工程、文件与 Provider；中栏只承载自然对话；右栏显示真实 Markdown 文件编辑器。启动时只要求新建或打开工程，不要求预先建立人物、世界或章节。

NR-0 只建立可靠的普通对话基线。它不加载 Novelist Skill，不读取小说 Canon 作为模型上下文，不调用工具，也不由 AI 写入 Markdown。后续能力必须经过新的 Story Packet 和 Gate。

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
  ├─ Conversation Store
  ├─ Provider adapters
  ├─ CredentialVault
  └─ atomic persistence / recovery / validation kernels
```

生产 Command 固定为 24 条，其中 Conversation Command 固定为 `conversation_session_load`、`conversation_turn_start`、`conversation_turn_cancel`。流式事件为 `conversation-turn-event`。Command Manifest、Handler、Permission、Capability 与所有生产 TypeScript Client 必须严格相等。

## Conversation Store 与对账

工程内可见对话保存到 `.novelgainker/conversation/session.json`。Store 只保存工程/会话/Turn/Message 标识、顺序、用户可见文本、时间、Provider/模型和终态；不保存 Key、隐藏 Prompt、原始响应、绝对路径或未来能力状态。

Host 在调用 Provider 前先保存用户消息与 `running`。流式字符只保留在进程内，Turn 到达终态时一次保存完整结果；遗留 `running` 在重新打开时变成 `本次回复未生成。`，不会自动重试。

Event 只负责低延迟 Delta 和终态通知，Store 是最终事实源。UI 必须先完成 listener 注册才允许发送；活动 Turn 使用单一非重叠的 1000ms Session reconciliation。终态 Event 会触发立即读取 Store；Event 丢失、延迟、重复或乱序不会产生第二次 Provider 调用或永久活动状态。listener/Store 同步失败会显示“状态同步失败”，并只允许重建 listener 或重新读取 Store。

## Provider 路径

DeepSeek 与 OpenAI 共用普通文本请求模型和同一终态规则。每个 Turn 只调用一次 Provider，Skill/Tool/Canon 操作均为 0。自动化测试只使用本地 Loopback，不读取真实凭据、不访问真实 Provider。生产调用只在用户主动发送时由 Rust Host 临时使用 Windows Credential Manager 中的密钥。

普通闲聊、空工程、小说讨论与直接创作走同一路径；缺少人物卡、设定或章节不得阻断回复。任何非空模型文本都必须保留；异常结束时附加 `本次内容未生成完成。`，零文本失败显示 `本次回复未生成。`。

## Project Metadata 与一次性切换

Project Metadata 当前通用格式为 `schemaVersion = 2`。打开工程时 Host 先读取有大小上限的最小信封：v2 才进入严格 Metadata 解析；缺失 Metadata 沿用确认创建路径；损坏、缺失 schema 或未知版本返回稳定错误且不删除内容。

v1 表示本次可丢弃的旧应用状态。只有在取得 writer lease、确认规范化项目根和拒绝 `.novelgainker` 内 symlink/reparse point 后，Host 才以已准备的 v2 状态替换精确 `.novelgainker/**`，生成新 Project ID。失败不会留下可合法打开的 v1/v2 混合状态；成功后二次打开保持新 ID 与 Conversation，不再重置。

全局 App State 在单一事务中删除同一规范化根关联的旧 `recent_projects` 与 `project_files`，随后只记录新 Project ID。Provider Settings、UI Preferences、无关项目记录和 Windows Credential Manager 凭据不在重置范围；小说 Markdown、素材与图片逐字节保持不变。

## 关闭生命周期

原始窗口关闭请求只检查手动 Markdown 是否未保存。只有这种情况会显示 `保存 / 放弃 / 取消`：保存成功或放弃后关窗，保存失败或取消则留在窗口且当前 AI Turn 继续。确认关窗后直接销毁窗口，不等待项目关闭、Provider 或 Conversation flush；窗口销毁事件负责取消活动请求。

## 发布 Gate

NR-G1 Gate 递归发现 `apps/desktop/src` 的生产 TS/TSX、Tauri/Rust 生产源码、当前 contracts 和 Tauri 明确配置的 production resources；测试和历史文档不计入退役产品引用。Gate 同时比较五层 IPC 集合、读取上游测试 Evidence、校验当前 Report 声明，并用临时生产文件证明新增退役入口会被拒绝。

## Gate 状态

NR-G1 已获产品批准。当前只形成并评审 NR-1 Story Packet Draft 1；Memory、自动压缩和长对话 Context 尚未实施，Novelist Skill、工具与文件操作仍属于更后阶段。真实 Provider Live Smoke 未执行。
