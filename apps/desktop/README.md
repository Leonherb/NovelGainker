# NovelGainker Desktop

基于 Tauri 2 + React/TypeScript 的 Windows 桌面小说工作台。

NR-0 界面保持三栏：左栏是工程、文件树与 Provider 设置；中栏只显示工程内自然对话、输入、停止和终态；右栏是实际 Markdown 编辑器。启动时只需新建或打开工程，不要求预先创建人物、世界或章节。

用户发送后，Rust Host 先保存用户消息与 `running`，再使用当前已配置 Provider 发起一次普通文本请求。DeepSeek/OpenAI 流式输出立即显示；Conversation Store 是终态真值，活动 Turn 每 1000ms 自动对账，即使终态 Event 丢失也不会永久停在“正在回复/正在停止”。对话保存在 `.novelgainker/conversation/session.json`，不进入小说索引或出版输入。

Project Metadata 当前为 `schemaVersion = 2`。v1 工程在 writer lease 与安全路径检查后只重置一次精确 `.novelgainker/**`，生成新 Project ID；正文、设定、素材、图片、Provider 凭据、UI 偏好和无关项目状态保持不变。

本阶段不加载 Novelist Skill、不读取小说文件作为 AI 上下文、不调用工具、不自动写入 Markdown。右栏手动编辑仍保留原子保存、source hash、外部修改检测、结构诊断、离线搜索和单 Writer 安全。

窗口关闭只有脏 Markdown 可以阻断，并提供 `保存 / 放弃 / 取消`。确认关闭后窗口立即销毁，不等待项目或 Provider；活动请求在窗口销毁时取消。

生产 IPC 为 24 条，其中 Conversation Command 为 3 条。开发、测试和构建命令见 [docs/development.md](../../docs/development.md)。NR-G1 已获产品批准；当前只等待批准 NR-1 Draft 2 Final，未授权实施，也未执行真实 Provider Live Smoke。
