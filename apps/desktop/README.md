# NovelGainker Desktop

基于 Tauri 2 + React/TypeScript 的 Windows 桌面工作台。

当前界面包含可折叠的 Key-only API 左栏、流式对话中栏，以及可收起、可调宽的右侧项目文件树与 Markdown 源编辑器。中栏展示构建内置的 Workflow Pack v1，可通过明确入口创建项目、设定、素材、主剧情和人物，也可启动不可路由的 Storytelling 章节流程。应用保留本地项目创建/打开、显式原子保存、外部修改检测、结构诊断、离线搜索、最近项目与布局恢复；无 Key、无网络或 Provider 故障时，离线编辑仍可使用。

截至 G5，实体工作流共用固定 Context、结构化 Proposal、Diff、显式确认和可恢复 Apply；确定性 Router 只覆盖已批准的五个实体入口。`novelgainker.storytelling` 已支持可解释 Context、Reader Mental Model、双 Provider Chapter Proposal、本地连续性检查、章节 Diff/Apply、章节状态和独立 Ledger 事务，并保持 `executionAvailable=true`、`routable=false`。G5 已获产品批准并关闭 Sprint 5；Sprint 6 Story Packet 已创建但尚未通过 Review Gate，NG-601 未授权，当前构建不含评审、图片导入或 EPUB 新能力。

开发、测试和构建命令见 [docs/development.md](../../docs/development.md)，当前执行边界见 [Sprint 6 Story Packet](../../docs/stories/sprint-6-story-packet.zh-CN.md)。
