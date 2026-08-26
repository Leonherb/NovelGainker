# NovelGainker Desktop

基于 Tauri 2 + React/TypeScript 的 Windows 桌面工作台。

当前界面包含可折叠的 Key-only API 左栏、流式对话中栏，以及可收起、可调宽的右侧项目文件树与 Markdown 源编辑器。中栏额外展示构建内置的 Workflow Pack v1、唯一“创建人物”入口和项目级工作流状态卡；用户可填写人物简报，先离线审查 Context Manifest，再通过独立按钮确认向固定 Provider 发送。应用保留本地项目创建/打开、显式原子保存、外部修改检测、结构诊断、离线搜索、最近项目与布局恢复；无 Key、无网络或 Provider 故障时，离线编辑仍可使用。

NG-201～NG-207 已通过 G2。Sprint 3 Story Packet Review Gate 已通过，S3-Q001～S3-Q005 全部按建议接受；NG-301、NG-302 已获产品验收。NG-303 已实现严格 `CharacterBriefV1`、仅 `project.md`/根目录 `AGENTS.md` 的有界 Context、Hash 绑定 Manifest、固定 Prompt Compiler、离线 `prepare`、显式确认 `generate`、源变化阻断和敏感内存清理，并通过统一质量门，等待产品负责人验收。NG-304 的 Proposal、Hash、Diff、确认写入和原子 Apply 尚未授权。

开发、测试和构建命令见 [docs/development.md](../../docs/development.md)。
