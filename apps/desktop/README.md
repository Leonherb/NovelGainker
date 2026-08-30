# NovelGainker Desktop

基于 Tauri 2 + React/TypeScript 的 Windows 桌面工作台。

当前界面包含可折叠的项目/文件树/Key-only API 左栏、按句呈现对话的统一 Agent Timeline 中栏，以及可收起、可调宽的右侧 Markdown 源文件与 Agent 修改审查栏。打开项目后，所有自然语言输入都进入同一个项目隔离 Session；中栏显示 Provider、Skill、Turn 与读写 Tool 轨迹，右栏显示完整逐文件 Diff、风险、来源/结果/事务 Hash，并提供确认和拒绝，Turn Undo 仍留在中栏。应用继续提供本地项目创建/打开、显式原子保存、外部修改检测、结构诊断、离线搜索、最近项目与布局恢复；选择文件或出现待确认 Diff 时会打开右栏。无 Key、无网络或 Provider 故障时，Agent 输入立即拒绝，但离线编辑仍可使用。

AM2 生产路径内置 9 项受锁 Skill，注册 `project.list_markdown`、`project.read_markdown`、`project.search_markdown`、`project.validate_markdown` 四个受限读取工具，以及 `project.create_markdown`、`project.patch_markdown` 两个按 Skill/目录治理的写工具。三种策略为 `plan_only`、`review_every_write` 与默认 `auto_apply_safe`；每 Turn 最多一个八文件原子事务，Pending 正文只在内存，Undo 正文使用 Windows 当前用户 DPAPI 保护。DeepSeek/OpenAI 共用严格 Action Schema、预算、同 Provider 单次修复和禁止跨 Provider Fallback 的停止条件。G3～G5 后端仅保留回归，旧 Conversation/Router/表单不再挂载；迁移遗留的孤儿 Workflow 生成状态会在没有真实 Provider Run 时安全终止，恢复记录作为不阻塞、不展示且不自动删除的兼容历史保留，失败发送继续保留输入。

当前界面和 V1/AM2 Runtime 仍是待迁移的 as-built 基线。RB0 与 Agent Protocol V2 已获产品批准，目标改为独立 Conversation/Control、项目内完整可见 History、Host 增量 Memory 和可重建 Context；这些能力尚未实施。当前唯一待评审实施输入是 [AM-R1 Story Packet](../../docs/stories/agent-conversation-authoring-am-r1-story-packet.zh-CN.md)。未批准前不修改 Runtime、不删除旧安全回归，也不执行 Sprint 6 或真实 Provider Live Smoke。开发、测试和构建命令见 [docs/development.md](../../docs/development.md)。
