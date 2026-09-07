# 未实际调用工具却声称已提交：调查与修复进度

日期：2026-09-08。本记录是用户问题修复，不是 NR-604 或 Gate 验收。

## 已确认的现象

只读核对用户当前工程后，最近四轮均为 completed，agentRequests=1、toolDispatched=false，没有对应 Task，也没有世界观 Markdown 或变更回执。历史助手所说的“已提交、等待 Host 回执”不是程序执行事实。

更早一轮确实有两次读取和两次 `agent.tool_arguments_invalid` 变更失败。参数诊断修复解决错误反馈；它不能保证下一轮模型会选择发起工具调用。未保存现象不能统一解释为 API 余额不足。

当前写入范围也不包含 `project.md`。项目元数据不能混入世界观等受支持资料的 AI 变更批次。

## 程序缺口与本次调整

- 之前下一轮会继续发送历史助手文字，却未附上对应的 Host 执行事实。历史错误叙述可能延续模型的错误判断；这属于有依据的推测，尚未通过真实 Provider 实验确认模型因果。
- NR-5 系统上下文加入当前会话最近八个已结束轮次的受控工具事实，每轮最多七项工具名、状态和错误码；不含参数、正文、凭据或绝对路径。读取失败明确为 unavailable，旧轮次缺少重试状态为 null，不把未知说成未执行。读取不修改 Task 文件。
- 明确通过原生 function/tool call 执行操作，同轮读取结果后再答复；文字不是提交、历史助手自述不是回执。实际已提交状态仍以 durable committed 为准。
- 对话界面在已结束轮次、Host 明确 toolDispatched=false 且无对应任务时显示“Gainker 核对：本轮未执行工具，回复文字不会保存文件。”来源是本地事实，与模型回复独立。
- 保持模型语义选择、普通聊天不强制工具调用、不增加自动重试、不扩大写入权限或请求预算。

## 已完成验证

- `cargo test -p app-core recent_facts --locked --offline`：1 项通过，覆盖有界事实、会话隔离、未知状态及不回显参数、不改写存储。
- `cargo test -p app-core tool_feedback --locked --offline`：1 项通过；合成历史先包含虚假的“已提交”，捕获两个 Provider 的原生请求核对 Host 事实；保留参数修正后的真实 committed/Markdown 断言及连续失败不写入的断言。Provider 回复由离线 loopback 脚本提供，不能证明真实模型会选择工具。
- `ConversationView.test.tsx`：11 项通过，包含明确无工具、已调度工具、旧数据未知三种投影；提示不触发额外 Provider 请求。
- 类型检查、ESLint、app-core 全目标全特性 Clippy 和 `git diff --check` 通过。
- `pnpm quality` 停在既有 29 个文件的格式检查问题，未改变这些无关文件或检查标准。
- Release `--no-bundle --locked --offline` 构建成功，并通过原生界面启动。

测试程序：`target/tool-execution-test/novelgainker-desktop.exe`。

SHA256：`3164695B53921AABA8BA6C903D2E546BF189DCB10B18B9D12298D8D42C588B5C`。

## 尚待验证

追加桌面命令检查：`cargo test -p app-core nr506_desktop_ipc_creates_project --locked --offline` 通过。该测试现在在首次对话前，通过实际 IPC handler 依次加载空会话、查询变更与出版记录，并确认写入协调器为 NR-5；两个合成 Provider 分支均通过。它排除了“新建空工程必然缺少写入协调器”的解释。原生窗口中出现过“保存结果暂无法核对”，用户按 Esc 停止电脑控制后未继续操作；尚未取得这次 UI 查询失败的具体错误码，不能认定它导致了此前的零工具调用。并发查询可能争用受控锁，但现有证据不足以将其认定为本次原因。

以上为此前测试包的证据，不证明真实模型已恢复执行工具。后续用户明确要求先在开发版跑通，并指定使用现有 `test2` 工程，授权范围内的实际测试如下。没有制作新的安装包或发布包。

## 2026-09-08：test2 原生开发版对照

用户随后要求修复重复打开工程产生只读会话的问题。修改后，原生双窗口实测第二次打开 test2 会激活既有可写窗口，不创建只读副本。另见 `project-window-reuse-fix.zh-CN.md`。

在可写的 test2 默认会话内执行保存请求，轮次 `faf82925-3ec1-4d42-b072-795b92767962` 仍只返回准备保存的文字，无 Task、无 committed 回执、无世界观文件。只读锁不是这一现象的根因，单纯补充 Host 事实提示未解决实际问题。

在生产 Rust 请求路径加入仅 Debug 的受控诊断（注册工具名称、profile、消息数量、请求字节数、结束枚举、有无工具调用、文本字节数），不打印请求/响应正文、密钥、路径、隐藏提示或 Continuation，不存入项目。

随后在同一工程中比较相同的只读请求：

- 默认会话 `bd59ff43-ce5e-4357-a26e-e9021f62afe7`：实际提供 NR-5 三个工具，47 条消息、67206 字节；返回 Stop、tool=false、103 字节文字，无工具任务。
- 新建“工具诊断（新会话）”会话：实际提供同样三个工具，初始 2 条消息、15148 字节；三次 Agent 请求，先返回 ToolCall，参数校验失败后修正，再次 ToolCall 读取成功，最后 Stop。持久 Task 为读取失败（agent.tool_arguments_invalid）及读取成功；world 目录为空。

这组真实对照支持“旧历史上下文影响工具选择”的判断，尚不足以证明唯一因果，也不证明写入已恢复。没有靠改用新会话宣称原问题修好。

## 待实测的历史上下文修正

- NR-5 将已结束轮次作为标明原发言角色的历史引用数据传给模型，不再把缺失原生工具调用/结果的历史助手文字直接重放为 assistant 协议消息。当前用户请求仍保持原文，并且只出现一次；当前轮次真实 native tool exchanges 保持原协议。
- 每个引用片段带真实 messageId、完整原消息 contentHash、片段字节范围及 Host 调度事实；未知旧记录保持未知。引用内容不改写磁盘 History、Canon 或 Memory，也不提升助手建议为用户授权。
- 历史引用的附加字节计入上下文选择预算，保留消息数量、正文大小、实际请求体、工具次数、输出 Token 和落盘前安全预留校验。没有按关键词强制调用或增加自动 Provider 重试。
- 修复另一处来源可用性缺口：旧逻辑将“单批最多八条引用”同时用作“只认识最近八条消息”。NR-5 现在最多解析最近 64 条真实消息的 ID/hash，近期简短目录仍最多八条、每个变更仍最多八条来源。提交和本地确认都会按相同范围重新核对磁盘原消息；当前用户、来源角色、方法回执、完整读取和哈希校验保留。
- 历史 NR-4 请求投影与来源范围保持原合同。

用户按 Esc 停止电脑控制时，新历史修正尚未在原生开发窗口加载、未在默认会话重测。此项仍待真实 Provider 验证，test2 世界观 Markdown 保存仍未通过。不得将离线脚本回复或新会话只读成功替代该验收项。

## 后续离线回归（2026-09-08）

用户明确回复“先不控制电脑”，因此没有重启窗口或继续真实 Provider 调用。

- `cargo test -p app-core nr5_history --locked --offline`：两项通过，包含历史原文不改写、来源哈希、近期目录与解析范围分别有界。
- `cargo test -p app-core tool_feedback --lib --locked --offline`：通过。扩展后的用例在原建议之后插入八轮“只说读取”的历史，分别捕获 DeepSeek/OpenAI 原生请求中的引用来源；原建议不在八条近期目录内，但可与当前用户接受消息一起通过真实协调器重新核验，参数修正后产生 committed 及 Markdown。方法回执失败分支保持无文件。模型响应是离线脚本，不证明模型会自主调用。
- `cargo test -p app-core memory::tests --lib --locked --offline`：12 项通过；新增 NR-5 投影同时覆盖一万条可见消息及大量转义字符，两个 Provider 的请求边界校验通过。
- `cargo test -p app-core repeated_project_open --lib --locked --offline`：通过。
- 前端 ProjectWorkspace / ConversationView / 错误展示测试：41 项通过；`pnpm typecheck`、`pnpm lint` 通过。
- `cargo check --workspace --all-targets --all-features --locked --offline` 与 app-core 全目标全特性 Clippy 通过；格式及 diff 空白检查通过。
- `pnpm quality` 仍停在原有 29 个文件的格式检查；没有批量格式化无关文件。独立 `quality:boundaries` 无法运行到结论，因为本地缺少历史 `target/qa/nr-602/quality.json`，没有伪造历史证据。
- `cargo test -p app-core --lib --locked --offline` 完整库测试结束：302 项通过、0 项失败、4 项按设计忽略的辅助夹具，耗时 1710.83 秒。最后的 500 次实际提交、记录总数与撤销检查通过。运行期间保留其进程，将占用的测试可执行文件在 `target/debug/deps` 内改名，以便编译补充的定向测试，未终止长测试；之后增加的旧消息保存、转义字符边界用例及重复打开用例已单独通过。

未完成项仍是：新历史投影在真实 DeepSeek 默认会话的工具选择，以及 test2 保存后 committed 回执与实际文件三者一致。该验证受用户“先不控制电脑”的最新指示暂停，不能用离线成功替代。没有封包或更新 Gate 完成声明。
