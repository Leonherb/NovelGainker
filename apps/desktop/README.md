# NovelGainker Desktop

基于 Tauri 2 + React/TypeScript 的 Windows 桌面工作台。

当前界面包含可折叠的 Key-only API 左栏、统一 Agent Timeline 中栏，以及可收起、可调宽的右侧项目文件树与 Markdown 源编辑器。打开项目后，所有自然语言输入都进入同一个项目隔离 Session；中栏显示 Provider、Skill、Turn、读写 Tool、来源、完整逐文件 Diff、风险与来源/结果/事务 Hash，并提供策略选择、确认、拒绝和 Undo。应用继续提供本地项目创建/打开、显式原子保存、外部修改检测、结构诊断、离线搜索、最近项目与布局恢复；无 Key、无网络或 Provider 故障时，Agent 输入立即拒绝，但离线编辑仍可使用。

AM2 生产路径内置 9 项受锁 Skill，注册 `project.list_markdown`、`project.read_markdown`、`project.search_markdown`、`project.validate_markdown` 四个受限读取工具，以及 `project.create_markdown`、`project.patch_markdown` 两个按 Skill/目录治理的写工具。三种策略为 `plan_only`、`review_every_write` 与默认 `auto_apply_safe`；每 Turn 最多一个八文件原子事务，Pending 正文只在内存，Undo 正文使用 Windows 当前用户 DPAPI 保护。DeepSeek/OpenAI 共用严格 Action Schema、预算、同 Provider 单次修复和禁止跨 Provider Fallback 的停止条件。G3～G5 后端仅保留回归，旧 Conversation/Router/表单不再挂载；迁移遗留的孤儿 Workflow 生成状态会在没有真实 Provider Run 时安全终止，恢复记录作为不阻塞、不展示且不自动删除的兼容历史保留，失败发送继续保留输入。

NG-A201～NG-A206 已通过；GA2-R001～GA2-R004 已修复，revision 5 完整质量门与 GA2 技术提交均已通过，等待独立产品验收。最新修订向 JSON-object Provider 明示精确 Agent Action 形状，并确保异步失败、稳定错误代码与原输入不会被会话刷新吞掉。完整聊天持久化、Sprint 6、评审、图片导入、EPUB 新能力和真实 Provider Live Smoke 均未授权。开发、测试和构建命令见 [docs/development.md](../../docs/development.md)，当前边界见 [AM2 Story Packet](../../docs/stories/agent-migration-am2-story-packet.zh-CN.md)。
