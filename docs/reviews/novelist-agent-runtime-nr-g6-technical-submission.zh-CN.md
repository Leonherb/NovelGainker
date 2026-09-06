# NR-G6 技术提交 — NR-5 小说创作闭环

日期：2026-09-06（Asia/Shanghai）。结论：NR-501～506 分别通过完整质量门，NR-G6 技术检查通过。初次技术快照 nrG6Approved=false，不冒充新的用户 Gate 批准；依据已记录的连续实施批准，技术证据核验后直接进入 NR-601，无中间人工等待。最终 NR-G7 仍待用户决定。

## 验收与证据

- 当前生产 30 条 IPC，内置 Pack 1.4.0 / 9 能力 / 37 资源，Pack SHA256 319be9d883968cfd225974c20eb890f79bd5a836c9996c8f699f7aa0658e0d2e。工具为 read_context / propose_change / publish，无任意文件系统、Shell、URL 或 Opener。
- 章节提交、用户明确版本定稿、正式章事实维护五类资料、来源过期、Diff/Undo、本地 EPUB 预检/构建均已挂载。单 Turn 一章；4 读 / 2 变更 / 1 出版，8 Agent + 1 Compactor，33792 requested output tokens，最多七次串行工具、末轮无工具。
- NR-506 通过实际桌面源码的 Tauri IPC handler 和后台 worker，执行双协议合成 HTTP：新建工程→讨论→人物/设定→十二章保存并定稿→章节来源素材→显式评审→EPUB→重开→Undo 后出版失效。核对实际落盘字节、回执、产物 Hash、排除隐藏状态及重开一致性。
- 双协议实际桌面故障矩阵验证零文本、保留部分文本、工具参数失败仍可回复、待确认不写入/放弃、已保存后最终 503 不否认提交。独立 React 操作回归验证异常后可继续输入、取消、Diff/Undo、出版回执失败与原文分离。
- NR-505 在本轮继续执行 8 条常规原生出版链、2 条真正 8+1 最大链以及独立 .NET ZIP/XML 结构检查；不是测试 stand-in，也不是完整 EPUB 认证。
- 历史 NR-G5 技术报告、Manifest 与 NR-501～505 完成证据保持冻结；回归输出另存。

## 本轮修复与评审

端到端回归发现 Windows Task 快照读取与原子替换竞争（OS 5），会形成 tool_executor_failed。已对同工程 Task 完整读取/更新加短时协调，不跨 HTTP 或工具执行；100 项三状态更新与三路快照读取、双协议十二章连续四轮均通过。测试夹具另修复重复 Call ID、DTO 取值和闭合风险枚举，生产防重复及参数规则保留。

完整质量首次因旧 NR-006 脚本漏认 Runtime 泛型声明而停止；修正为精确识别并额外要求唯一声明，仍断言唯一聊天路径，再从头重跑。未跳过检查，临时诊断已移除。主实施者自审，不声称独立人工评审。

## 完整质量窗口

2026-09-05T16:58:50.758Z～2026-09-05T17:21:58.899Z：pnpm quality exit 0，31 步。132 项前端、257 项 app-core 通过；4 个 ignored 为父测试调用的子进程夹具。其余 workspace、历史阶段、五项 NR-506 定向测试、Debug/Release 构建和启动均通过。500 次实际提交、501 次拒绝和最早 Undo 通过；app-core 1139.91 秒不是 NR-6 交互性能达标声明。

- releaseScopeSha256：bfdc16cee6bafe591ae9cba81e139bdf965be970dc04521ea115c076eb0cdaf6
- target/qa/nr-506/quality.json：098ca628049e23eb684692bb42968f7489f28fd4cfc3a90f89ae1a0044683541
- target/qa/nr-506/validation.json：0518b7b083f5a5f3303da08f554e0e76b31b6ca2750e69b06bf564382730c264
- Debug EXE：8eb5f50ac43e051fba34837b319c61c75a3d707ea3eaa2a752cf8a9fd049932a
- Release EXE：8b37237aad96210eda71f3cf9a49223fe06295f8f5569f21dc95a43cfbe362a0

构建来自未提交工作区，以源码与产物 Hash 绑定，不虚构 Git 提交。后续构建会替换工作目录 EXE，上述值只表示本次已验证版本。

## 遗留限制和下一步

Tauri MockRuntime 替代窗口/选择结果，synthetic vault 和 localhost 服务替代外部 Provider；虽然执行真实 command/worker/磁盘链，它不是原生 WebView 点击链或真实模型语义证据。桌面启动冒烟使用现有启动流程，不冒充干净 Windows 安装测试。没有使用真实 Key、外部请求、分发、用户试用、完整 EPUB 阅读器认证或人工长篇评分。图像生成仍不可用。

NR-601～603 继续补齐会话分域、分支/恢复/导出和保守显式重试；NR-604/605 的参与者、干净 Windows 环境及实际反馈仍须用户安排，没有真实证据不得完成验收。无云同步、NR-7、Pre-NR-4 或重开旧 Gate。

## 机器声明

```json
{"schemaVersion":1,"gate":"NR-G6","technicalChecksPassed":true,"nrG6Approved":false,"continuousImplementationAuthorized":true,"humanWaitRequired":false,"nextStory":"NR-601","nrG7Approved":false,"productionCommandCount":30,"productionToolCount":3,"packVersion":"1.4.0","fullQualitySteps":31,"realProviderExecuted":false,"externalNetworkRequests":0,"multiUserTestingExecuted":false,"cloudSyncInScope":false}
```

