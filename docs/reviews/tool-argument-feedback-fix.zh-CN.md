# 工具参数失败与本轮限制说明修复（2026-09-08）

## 问题与修改

NR-5 的写入参数错误此前只返回 `agent.tool_arguments_invalid`。两次失败计入既有变更次数后，下一次模型请求不再携带写入工具，容易被模型解释为环境没有保存能力。

- 严格 schema 校验失败时返回最多四条 `argumentIssues`，指出缺失字段、未知字段所在对象或类型/枚举错误的字段路径。路径只来自 Host schema，不回显模型提供的字段名、参数值或正文；诊断限制为 2048 字节。
- NR-5 语义校验保留现有具体错误码，区分方法未读、来源不匹配、原文未完整读取、目标或候选无效，并提供对应的纠正说明。
- Host 系统说明明确两次变更包含失败调用，工具列表缩减是本轮次数/安全预留限制，与 API 余额、应用保存权限无关；还明确 `project.md` 不属于 AI 写入工具的支持范围。参数结构以当前 tools schema 为准。
- 次数、Token、原生请求大小和写入前最终回复预留保持原值；没有增加自动 Provider 重试，没有放宽来源、回执、Hash、路径或事务校验。
- `ToolResult` 的错误诊断仅进入有界 Provider 结果；Task 仍只保存摘要 Hash/字节数及受控状态。成功/取消结果不能携带参数错误诊断，已有提交的持久化回执仍是保存事实。

## 验证范围

新增离线回归使用临时合成工程和 loopback 服务，分别捕获 DeepSeek/OpenAI 原生请求：首次遗漏必填可空字段后收到具体字段提示，第二次修正后产生真实 committed 回执；连续两次失败时无文件写入，下一次仍可聊天且写入工具按原上限移除。另验证诊断条数、状态边界及不回显参数值/未知字段名。

本次不是 Story/Gate 验收。`pnpm quality` 停在仓库既有 29 个文件的 Prettier 检查问题；没有格式化无关文件或更改检查门槛。500 章规模压力测试因耗时中止，剩余定向回归单独执行。没有调用真实 Provider、使用真实小说工程作测试或自动分发安装包。

实际结果：`cargo test -p app-core --locked --offline -- --skip nr502_five_hundred_real_commits_exceed_legacy_record_limit_without_dropping_undo` 通过 296 个单元测试和 2 个集成测试，4 个既有忽略项、1 个过滤项；三项新增回归均通过。`cargo clippy -p app-core --all-targets --all-features --locked --offline -- -D warnings`、本次格式检查与 `git diff --check` 通过。Release 无安装包构建通过，程序复制至 `target/test-build/novelgainker-desktop.exe`，包含用户此前的会话滚动修复；未替换或停止用户正在运行的旧版本。
