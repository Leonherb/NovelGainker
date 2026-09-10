# NovelGainker 开发与验证

更新：2026-09-09。当前实现见[架构](architecture.zh-CN.md)，目标行为见[产品策划](product-spec.zh-CN.md)。本文的命令来自当前 `package.json` 和 `scripts/quality.mjs`，历史测试通过不代替改动后的实际验证。

## 环境与常用命令

开发环境为Windows 11、Node.js 24+、pnpm 11+、Rust MSVC及Tauri 2系统依赖。仓库packageManager固定pnpm版本；按锁文件安装依赖。Rust验证使用 `--locked --offline`。

```powershell
pnpm install --frozen-lockfile
pnpm dev
pnpm typecheck
pnpm lint
pnpm test:frontend
cargo test --workspace --locked --offline
pnpm e2e:sprint1
pnpm quality
```

`pnpm build:test` 生成无安装器Release构建，`pnpm build:test:debug` 为Debug。当前Tauri关闭bundle；构建EXE不能称为已验证的干净安装或升级包。首次安装依赖与离线验证是不同步骤，未授权的真实Provider调用不能作为构建排错方式。

## 检查与证据

完整质量门为 `pnpm quality`。当前脚本含34步：前端格式/lint/类型/测试、生产边界、NR-0～4历史合同回归、内置Pack一致性、NR-501～506与NR-601～603定向链路、Rust格式/Clippy/测试、离线编辑器往返、Debug/Release构建和桌面smoke、Canon恢复内核。

按改动选择定向验证；声明某实现Story或Gate技术完成前，必须通过完整质量门。文档整理只验证事实、引用、状态和差异，不因此生成新的实现完成或质量通过声明。

| 入口 | 证据目的 |
| --- | --- |
| `e2e:nr001`～`e2e:nr406`及`pack:nr301:check` | 历史合同、内置方法及回归；不能改写冻结Gate的原始声明 |
| `e2e:nr501`～`e2e:nr505` | 章节合同/提交/连续性、非线性循环预算、本地出版 |
| `e2e:nr506` | test-only Tauri MockRuntime下实际桌面命令与worker链；不等于原生GUI或真实Provider |
| `e2e:nr601`～`e2e:nr603` | 会话隔离、管理、原预算内显式重试与attempt对账 |
| `e2e:sprint1` / `e2e:canon-kernel` | 离线编辑往返及文件事务恢复边界 |
| `smoke:desktop:debug` / `smoke:desktop` | 当前开发机上的桌面启动检查 |

证据位于 `target/qa/`。完整质量脚本当前写入 `nr-603/regression-quality.json`；既有Story完成时的 `quality.json` 及Gate manifest为各自时点的证据。原始技术提交、当时的human-approved字段和Hash保持冻结，后续回归独立记录当前源码、IPC和Pack。某目录存在不代表证据与当前源码匹配。

生产构建复验要记录源码提交及工作区改动、构建时间、EXE校验值和测试所用确切包。仅凭文件名或上次会话中的“已修复”不能证明当前EXE包含补丁。正式试用、安装/升级和最终包影响复验参见[测试方案](testing/novelist-agent-runtime-nr6-multi-user-test-plan.zh-CN.md)。

## 运行和接口维护

生产共38个受限Tauri命令。原Conversation load/start/cancel保持职责，另有会话管理、显式重试等封闭入口；流式事件为 `conversation-turn-event`。修改IPC必须同步Manifest、Handler、生成Permission、Capability和全部生产TS客户端及边界测试，不能只改一端。

运行入口在 `apps/desktop/src-tauri/src/commands_current.rs`，模型循环在 `crates/app-core/src/conversation_runtime.rs` 和 `conversation_change_runtime.rs`；工具注册在 `tool_runtime.rs`，来源校验、事务和回执在 `canon_change*.rs` 与其子模块。当前工具为read_context/propose_change/publish，当前预算以[架构](architecture.zh-CN.md)及对应源码为准，不套用NR-4旧限额。

默认History/Memory/Task仍保留 `conversation/session.json`、`memory/state.json`、`tasks/session.json`，新会话分别位于对应目录下 `sessions/{UUID}.json`。不得把默认旧会话挪走、复制成两份或误认为所有会话都在新目录。同工程共享Canon、一个活动运行；结果归属使用worker捕获的会话和attempt。

元数据当前为严格schemaVersion 2。旧v1一次性重置仅清理经验证的内部派生状态，保留稿件、资产、设置和凭据；它不是通用升级方案。不要引入v1兼容读、转换器、别名、替代Workflow版本或功能开关。将来新增阶段状态必须另行规定版本与恢复，不能借用旧重置流程清空用户内容。

## 测试边界

- 自动化只用验证过的隔离合成工程及127.0.0.1 loopback合成密钥，不碰真实稿件、凭据或远程Provider。质量脚本清理Provider/secret环境变量。
- 密钥只在Rust CredentialVault中使用，不通过IPC读回、脱敏回显后缀/指纹或写入日志、项目及测试证据。
- 稿件Markdown与受控资产为唯一Canon，内部History/Memory/Task/Change不进入稿件索引或出版。
- 当前Conversation调用零自动重试的单次Provider路径；显式重试由 `conversation_retry.rs` 约束，不是连接测试的重试函数。新目标Q9批准有限自动恢复，实施时须更新运行/预算/幂等合同及测试，不能只解除现有检查。
- 真实用户已经反馈过故障，但正式多用户C/S/J/L及安装/升级/最终版矩阵仍需实际证据。参与者自行配置Key并自愿提供脱敏反馈，助手不自动招募、分发或执行Live批次。
- 脚本不能证明真实模型正确理解“讨论”“接受建议”“改写范围”等语义；语义成功率须来自定义清楚的真实任务与评分。

## 恢复与本地操作

只有持久Committed表示文件已保存。待确认不是已保存；任务成功和模型文字不是回执。过期版本、外部修改、dirty editor或来源不全要求重新读取准备，不能force-overwrite。

Undo要求所有目标匹配afterHash，并保留不可变快照和逆事务意图。未决恢复时保留稿件与变更库，核对Diff及外部编辑后安全恢复；不得删除记录、重置项目或清空Canon消除警告。恢复未决阻止新写，不应据此阻断普通聊天；恢复后重开可刷新写权限。存储满额不自动清除历史或撤销材料。

仅手工未保存Markdown可阻断关闭并给保存/放弃/取消选项。选择前不取消AI；确认关闭后立即退出，不等待网络。当前流式片段未逐段持久化，不承诺异常退出能恢复全部已显示文字。

当前授权及未完成项见[交付状态](sprint-plan.zh-CN.md)。历史Gate保持历史意义，最终NR-G7仍由用户决定；本轮策划访谈不构成新增架构的实施或验收完成。
