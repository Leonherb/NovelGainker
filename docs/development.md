# NovelGainker 开发环境

## Windows 11 x64 前置条件

- Microsoft Visual Studio 2022，包含“使用 C++ 的桌面开发”和 Windows SDK；
- Microsoft Edge WebView2 Runtime；
- Node.js 24.11.1；
- pnpm 11.19.0；
- rustup。

仓库通过 `rust-toolchain.toml` 固定 Rust 1.97.1、`x86_64-pc-windows-msvc`、rustfmt 和 Clippy。首次执行 Rust 命令时，rustup 会按声明安装精确工具链和组件。

Tauri 官方前置要求：<https://v2.tauri.app/start/prerequisites/>。

## 干净环境准备

在仓库根目录运行：

```powershell
pnpm install --frozen-lockfile
cargo fetch --locked
pnpm quality
```

前两步是显式的依赖准备阶段，允许从锁文件下载缺失依赖。`pnpm quality` 本身不安装依赖，并为 Cargo 子命令设置离线模式；因此质量门不会在测试过程中临时访问网络。

若依赖缓存已准备好，可验证离线恢复：

```powershell
pnpm install --frozen-lockfile --offline
pnpm quality
```

## 唯一完整质量入口

日常提交前只需要运行：

```powershell
pnpm quality
```

聚合入口遇到第一个失败即返回非零，固定顺序如下：

| 顺序 | 阶段                                                                                                                          | 独立命令                         |
| ---- | ----------------------------------------------------------------------------------------------------------------------------- | -------------------------------- |
| 1    | 前端格式检查                                                                                                                  | `pnpm format:check`              |
| 2    | ESLint，零警告                                                                                                                | `pnpm lint`                      |
| 3    | TypeScript 类型检查                                                                                                           | `pnpm typecheck`                 |
| 4    | React/TypeScript 单元测试                                                                                                     | `pnpm test:frontend`             |
| 5    | G1～G5 与 AM0～AM2 的网络、文件、状态、凭据、Provider、Agent、Skill/Tool、事务、Undo、恢复、Canary 和 Command/Capability 边界 | `pnpm quality:boundaries`        |
| 6    | AM0 NG-A001 Skill Pack 锁                                                                                                     | `pnpm skills:check`              |
| 7    | AM0 NG-A002 Session/Turn 契约锁                                                                                               | `pnpm agent-contracts:check`     |
| 8    | AM0 NG-A003 Action/Tool 契约锁                                                                                                | `pnpm agent-actions:check`       |
| 9    | AM0 NG-A004 Governance 契约锁                                                                                                 | `pnpm agent-governance:check`    |
| 10   | AM1 只读 Agent Runtime 契约锁                                                                                                 | `pnpm agent-runtime:check`       |
| 11   | AM2 受治理写入 Agent Runtime 契约锁                                                                                           | `pnpm agent-write-runtime:check` |
| 12   | Rust 格式检查                                                                                                                 | `pnpm format:rust`               |
| 13   | Rust Clippy，`-D warnings`                                                                                                    | `pnpm lint:rust`                 |
| 14   | Rust 离线锁定测试                                                                                                             | `pnpm test:rust`                 |
| 15   | Sprint 1 无 Key、断网 E2E                                                                                                     | `pnpm e2e:sprint1`               |
| 16   | Tauri Debug 内部构建                                                                                                          | `pnpm build:test:debug`          |
| 17   | Debug 主窗口 Smoke Test                                                                                                       | `pnpm smoke:desktop:debug`       |
| 18   | Tauri Release 内部构建                                                                                                        | `pnpm build:test`                |
| 19   | Release 主窗口 Smoke Test 与 G2 截图                                                                                          | `pnpm smoke:desktop`             |
| 20   | Sprint 2 Mock Contract、Canary Leak、DOM、离线编辑与 G2 证据 E2E                                                              | `pnpm e2e:sprint2`               |
| 21   | Sprint 3 双 Provider Safe Apply、取消/冲突/恢复、Canary Leak 与 G3 技术证据 E2E                                               | `pnpm e2e:sprint3`               |
| 22   | NG-401 离线项目创建 E2E                                                                                                       | `pnpm e2e:ng401`                 |
| 23   | NG-402 设定创建 E2E                                                                                                           | `pnpm e2e:ng402`                 |
| 24   | NG-403 素材创建 E2E                                                                                                           | `pnpm e2e:ng403`                 |
| 25   | NG-404 主剧情创建/替换 E2E                                                                                                    | `pnpm e2e:ng404`                 |
| 26   | NG-405 人物 v2 创建/完善/升级 E2E                                                                                             | `pnpm e2e:ng405`                 |
| 27   | NG-406 确定性命令、Router 同意/分类/选择器与离线回归 E2E                                                                      | `pnpm e2e:ng406`                 |
| 28   | NG-407 五工作流 Golden、双 Loopback、故障/泄漏矩阵与真实 G4 窗口证据                                                          | `pnpm e2e:ng407`                 |
| 29   | NG-501 只读 Context Builder、路径/符号链接与稳定 Manifest E2E                                                                 | `pnpm e2e:ng501`                 |
| 30   | NG-502 六字段 Reader Mental Model、预算、请求 Hash 与来源复核 E2E                                                             | `pnpm e2e:ng502`                 |
| 31   | NG-503 双 Provider Chapter Proposal、取消/修复/过期与零写入 E2E                                                               | `pnpm e2e:ng503`                 |
| 32   | NG-504 本地连续性规则、warning 确认、来源/目标过期与零 Provider/零写入 E2E                                                    | `pnpm e2e:ng504`                 |
| 33   | NG-505 章节 Diff、显式确认、事务恢复、原稿保全与 Ledger 不变 E2E                                                              | `pnpm e2e:ng505`                 |
| 34   | NG-506 独立状态确认、Ledger Diff/确认、pending 恢复、失败隔离与零后台重试 E2E                                                 | `pnpm e2e:ng506`                 |
| 35   | NG-507 DeepSeek/OpenAI 共用 Storytelling 状态机、失败路径零写入、手动编辑保护、G5 Manifest/Review/截图与未授权 Live 记录      | `pnpm e2e:ng507`                 |
| 36   | NG-A001 未激活 Skill Pack 证据                                                                                                | `pnpm e2e:nga001`                |
| 37   | NG-A002 Session/Turn 契约证据                                                                                                 | `pnpm e2e:nga002`                |
| 38   | NG-A003 Action/Tool/Provider 契约证据                                                                                         | `pnpm e2e:nga003`                |
| 39   | NG-A004 授权/持久化/Undo 契约证据                                                                                             | `pnpm e2e:nga004`                |
| 40   | NG-A101 统一 Agent Timeline 证据                                                                                              | `pnpm e2e:nga101`                |
| 41   | NG-A102 有界只读工具与 Canon 树零变化证据                                                                                     | `pnpm e2e:nga102`                |
| 42   | NG-A103 Skill Router 与 Tool Loop 证据                                                                                        | `pnpm e2e:nga103`                |
| 43   | NG-A104 有界恢复与双 Provider 证据                                                                                            | `pnpm e2e:nga104`                |
| 44   | GA1 技术提交 Manifest                                                                                                         | `pnpm e2e:ga1`                   |
| 45   | NG-A201 受治理 Markdown Preview 证据                                                                                          | `pnpm e2e:nga201`                |
| 46   | NG-A202 原子事务与 Undo 证据                                                                                                  | `pnpm e2e:nga202`                |
| 47   | NG-A203 授权策略与 Pending 证据                                                                                               | `pnpm e2e:nga203`                |
| 48   | NG-A204 Skill 路由写入 Loop 证据                                                                                              | `pnpm e2e:nga204`                |
| 49   | NG-A205 Timeline Diff 与 Undo 证据                                                                                            | `pnpm e2e:nga205`                |
| 50   | NG-A206 统一入口与双 Provider 证据                                                                                            | `pnpm e2e:nga206`                |
| 51   | GA2 技术提交与待产品批准 Manifest                                                                                             | `pnpm e2e:ga2`                   |

两个构建命令只生成未签名内部测试二进制，不生成安装器。产物路径分别是 `target/debug/novelgainker-desktop.exe` 和 `target/release/novelgainker-desktop.exe`。Smoke Test 隐藏启动固定路径，要求进程在 15 秒内创建主窗口，然后按精确进程 ID 关闭；Release 步骤将窗口证据写入已忽略的 `target/g2-evidence/tauri-g2-window.png`。Sprint 2 E2E 生成 `target/g2-evidence/security-report.json`；Sprint 3 E2E 生成 `target/g3-evidence/security-report.json` 与 `target/g3-evidence/tauri-g3-window.png`。证据不得包含 Key、Prompt、Response、Usage、用户路径或真实手稿。NG-301～NG-303 的独立真实 Tauri 入口证据分别使用 `pnpm smoke:ng301`、`pnpm smoke:ng302` 与 `pnpm smoke:ng303`，生成到对应的 `target/ng30x-evidence/tauri-ng30x-window.png`，并拒绝近乎全白的未渲染截图。NG-304 的通用 Proposal UI 由完整预览/Diff 组件测试、严格 IPC 契约、合成 create/replace 与故障注入测试覆盖；NG-305 再以两家 Loopback 结构化输出 Contract、真实临时项目 Proposal 零写入/collision、前端修复/确认/刷新打开组件测试覆盖生产接线；NG-306 通过双 Provider 成功路径、取消/过期/冲突零写入、Applying 子进程中断/重启对账与高熵 Canary 扫描汇总 G3 技术证据。默认完整质量流程不需要 API Key 或外部网络，也不伪造真实 Provider Proposal 截图。NG-401～NG-407 各自可通过 `pnpm e2e:ng401`～`pnpm e2e:ng407` 独立重放；NG-406 生成脱敏 `target/ng406-evidence/security-report.json`，覆盖两家 Loopback、六个精确中文命令、同意前零请求、项目零读写、一次结构修复上限、固定选择器、App State v8 原文零持久化和普通对话/离线编辑回归。NG-407 生成 `target/g4-evidence/security-report.json` 与 `target/g4-evidence/tauri-g4-window.png`，汇总固定 Pack Hash、五工作流资源/Golden、双 Loopback、项目事务、负面写入、Canary 和 Sprint 1～3 回归；统一质量入口通过同次运行时间标记拒绝复用陈旧的 NG-401～NG-406 报告。NG-501～NG-507 分别通过 `pnpm e2e:ng501`～`pnpm e2e:ng507` 重放；NG-505 证据覆盖 create/revise Diff、确认前零写入、源 Hash 冲突、同目录事务、故障/崩溃恢复、原稿保全、Ledger 不变和零 Provider 请求。NG-506 生成 `target/ng506-evidence/lifecycle-ledger-matrix.json`，覆盖独立状态与 Ledger 两次确认、draft→final 阻断、可见 pending 恢复、固定 Ledger 目标、Ledger 失败隔离、零后台重试与零 Provider 请求。NG-507 生成 `target/g5-evidence/` 下的双 Provider Loopback、章节/Ledger 事务、取消/恢复、Manifest、Review Report、Live 未授权记录与 `tauri-g5-window.png`；默认入口不执行 Live Smoke。

AM0 证据写入 `target/qa/ng-a001`～`ng-a004`；AM1 Story 证据写入 `target/qa/ng-a101`～`ng-a104`，GA1 Manifest 与 Review Report 写入 `target/qa/ga1/`。GA1 默认入口不读取真实 Key、不访问外部网络，并保留技术提交时的 `ga1Approved=false` 与 Agent 项目写入数为 0。

AM2 Story 证据写入 `target/qa/ng-a201`～`ng-a206`，GA2 Manifest 与 Review Report 写入 `target/qa/ga2/`。AM2 Runtime Contract Set Hash 为 `812b8d9208f46e70899ebb64681878f13a5a9854baf64f7afabd473e4dd64fa7`；默认入口断言 9 项 Skill、4 读 + 2 写工具、App State v10、60 条 Tauri Command、Windows 当前用户 DPAPI Undo、内存 Pending 正文、旧入口不可达、孤儿旧 Workflow `Generating` 状态只在无真实 Provider Run 时安全终止、`RecoveryRequired` 作为保留但不阻塞项目/Provider 且不展示的惰性兼容历史、拒绝输入可恢复、真实运行态继续受保护、JSON-object Provider 获得精确 `AgentActionPayloadV1` 形状、异步失败在 Session Reload 后仍可见且恢复原输入、Agent Provider 完成请求可在响应头与正文阶段中断且取消后零重试/零新工具/零新写入、窗口关闭复用取消信号、`ga2Approved=false` 与 `sprint6Authorized=false`。GA2 revision 6 还把当前 Release 路径、大小与 SHA-256 写入 Manifest，以便产品验收排除旧构建。这些入口只使用 Loopback/合成项目和脱敏证据，不读取真实 Key，不访问外部网络。

真实 Provider Live Smoke 与默认质量门分离。只有产品负责人已在应用中把两家 Key 保存到固定 Windows Credential Manager 目标并明确授权时，才运行 `pnpm smoke:provider:live`；该入口拒绝命令行参数，删除疑似 Secret 环境变量，只输出 Provider、Preset、固定模型、结果和时间。若缺少 Key、额度或网络，G2 必须明确记录一次 Live Waiver。

需要自动修复受 Prettier 管理的文件时运行 `pnpm format`。质量门自身的故障阻断验证使用 `pnpm quality:verify-gates`，会依次放入并清理四个保留名临时探针；报告写入已忽略的 `target/qa/ng-004-failure-gates.json`。

## 测试与 Fixture 规范

- TypeScript 测试与源码共置，使用 `*.test.ts` 或 `*.test.tsx`；
- Rust 单元测试使用 `#[cfg(test)]`，跨模块测试使用 Crate 的 `tests/*.rs`；
- 跨语言契约使用 `contracts/<area>/*.vN.json`；
- 后续跨 Workspace Fixture 使用 `tests/fixtures/<area>/`；
- 所有 Fixture 必须是确定性合成数据，不得含真实 Key、用户路径、真实手稿或联网依赖。

完整约定见 `tests/README.md`。

## 常见失败排查

### pnpm、cargo、rustfmt 或 Clippy 找不到

确认 Node.js、pnpm 和 rustup 已加入当前终端的 `PATH`，然后运行：

```powershell
rustup show
rustup component add rustfmt clippy --toolchain 1.97.1-x86_64-pc-windows-msvc
```

### Cargo 离线模式提示缺少包

质量门故意禁止临时联网。先在依赖准备阶段运行 `cargo fetch --locked`，再重新执行 `pnpm quality`。

### Prettier、ESLint 或类型检查失败

- 格式问题运行 `pnpm format` 后审查 Diff；
- 不通过关闭 ESLint 规则、内联 disable 或放宽警告阈值来绕过；
- 类型错误在源码修复，不使用无依据的断言掩盖。

### 锁文件不一致

依赖清单变化后使用 pnpm/Cargo 的正常依赖命令更新锁文件并审查 Diff。不要删除锁文件，也不要在质量命令中移除 `--locked` 或 `--frozen-lockfile`。

### Tauri 构建或 Smoke Test 失败

确认 Visual Studio C++ Build Tools、Windows SDK 和 WebView2 Runtime 可用。Smoke Test 失败会保留非零退出码；先直接运行 `pnpm build:test` 区分编译失败与窗口创建失败。

MSVC 为 Tauri `cdylib` 创建导入库时可能输出本地化 linker message。该消息没有被 allow 或忽略；Clippy 仍以 `-D warnings` 运行并必须通过。

## 当前已实施安全边界

Sprint 2 已在 Sprint 1 离线边界上增加 App State Schema v2、Windows Credential Manager、Key-only UI、固定 DeepSeek/OpenAI Provider Core、Loopback Contract、单活动流式 Run、取消、运行间显式切换、同 Provider 手动重试，以及 Canary/G2 E2E。NG-301 又加入 App 自有、构建时验证并编译进二进制的 Workflow Pack v1，只暴露已实现的 `novelgainker.character-create`。NG-302 把 App State 事务迁移到 v3，并加入确定性单 Manager、脱敏 Workflow Run 检查点、启动 `recovery_required` 分类、共享 Provider 请求锁，以及 status/cancel/abandon UI。NG-303 增加严格 `CharacterBriefV1`、只读 `project.md`/根目录 `AGENTS.md` 的纯 Context Builder、有界明文 Manifest、来源与清单 Hash、固定八段 Prompt、离线 prepare、Hash 重检和显式 confirmed generate。NG-304 增加严格单文件 Canon Proposal、版本化 Canonical JSON/Hash、完整预览、replace Diff、两步确认和可恢复原子 Apply；正文只留在 Rust 内存，Apply IPC 不接受正文、路径或工具。NG-305 已接入严格 `CharacterProposalV1`、DeepSeek/OpenAI 固定结构化输出、最多一次同 Provider 有界修复、安全中文文件名与确定性 Markdown Serializer；只允许对 missing 目标形成 create Proposal，确认 Apply 后刷新并打开新文件。NG-306 把两家 Loopback 的确认写入、取消/过期/冲突零写入、Applying 子进程中断与启动恢复、App State 回归、高熵 Canary 扫描和真实 G3 窗口证据纳入统一质量门。普通对话和离线编辑契约保持不变。

Sprint 4 已实现离线 `project-create`、严格 `setting-create`、`material-create`、固定 `plot-create`、`character-create` v2 和确定性 Router。五个工作流共享 Rust 独占身份/路径/序列化与 G3 Proposal/Apply 边界；NG-406 的六个精确中文命令零 Provider 请求，含糊分类须显式同意且只发送当前文本与固定目录。App State v8 只保存终态 Router 元数据，禁止保存原文、Prompt、Response、项目内容或远端 Payload；分类结果永不自动 Prepare、Generate、Confirm 或 Apply。NG-407 只增加离线 G4 聚合与脱敏证据，不注册 Sprint 5 工作流，也不扩大 Provider、项目或文件权限。

Sprint 5 至 NG-507 已加入不可路由、可在工作台直接执行的 `novelgainker.storytelling`：NG-501/502 构建只读 Context Manifest、固定预算与六字段 Reader Mental Model；NG-503 通过当前 DeepSeek/OpenAI Provider 生成进程内严格 Chapter Proposal；NG-504 执行零 Provider、零写入的确定性连续性检查；NG-505 允许在完整 Diff 与一次显式确认后事务性创建或修订一个章节文件。NG-506 再把正文 Patch、状态迁移和 Ledger 更新冻结为三个独立确认/事务面，只允许 `draft → accepted → final`；接受时把绑定当前章节身份与 Proposal/Manifest/Request Hash 的可见 `Ledger Pending` 写入章节 Markdown，Ledger 只从该记录向固定 `materials/000.story-ledger.md` 生成独立 Diff。Ledger 失败不回滚章节，“稍后处理”不创建后台重试。NG-507 证明两家 Provider 共用完整状态机与错误边界，并补齐 unknown 作者确认：unknown 保留原分类，只有当前会话内精确确认后才可进入 Diff，blocking 仍不可绕过。Workflow Registry 当前为 `executionAvailable=true`、`routable=false`，Router 范围不变。

G5、GA0 与 GA1 已获产品批准。AM2 的 NG-A201～NG-A206 已完成：统一 Timeline 现有 9 项 Skill、4 个读取工具、2 个受治理写工具、三种授权策略、八文件原子事务、Hash 绑定确认、DPAPI Undo 与 App State v10；GA2-R001～GA2-R005、revision 6 完整质量门和 GA2 技术提交通过。当前 `ga2Approved=false`，正在等待独立产品验收。[Sprint 6 Story Packet](./stories/sprint-6-story-packet.zh-CN.md) 已暂停，NG-601、analytical-review 写入迁移、图片导入和 EPUB 新运行时能力均未授权。

- 真实 API Key 只能由 Rust CredentialVault 写入 Windows Credential Manager；项目、SQLite、前端持久状态、日志和错误不得保存 Key 或其掩码、尾号、Hash、指纹；
- 项目访问必须绑定 Rust 持有的不透明会话与项目相对路径；
- Provider 网络只能由 Rust Provider Core 在用户显式连接测试或发送后访问固定 TLS Host、协议与模型；不得由 WebView 发起、自动切换或跨 Provider Fallback；
- Workflow Pack 只能来自编译期固定资源；运行时不得读取 Plugin Cache、Novelist、`SKILL.md`、Python、用户目录或网络，WebView 只能调用只读 `workflow_registry_load`；
- 不启用 Shell、Process 或 Opener 权限；
- 不打包 Python 或 Novelist 插件内容。

`pnpm quality:boundaries` 会检查六十条精确 Command/Capability、Key-only UI 与提交前 Buffer 清理、受控文件路径、原子保存、单 Writer、脱敏恢复、只读搜索、App State Schema v10 及 v1～v9 历史迁移、固定 Windows CredentialVault/Provider Core、无 Key 读回、运行中切换锁、同 Provider Retry、NG-207 Leak/E2E 与独立 Live Smoke 边界、NG-301～NG-407 的固定 Workflow Pack、Manager、Context、结构化输出、Router、Proposal/Apply/Recovery、双 Loopback/Golden、G3/G4 Canary 与证据脱敏边界，以及 NG-501～NG-507 的 31 项固定 Pack 资源、Storytelling 事务、连续性与 Ledger 回归；还检查 AM0 四组不可变契约、AM1 的统一 Timeline/只读 Agent 基线，以及 AM2 的 9 项 Skill、4 读 + 2 写工具、严格 Patch、三种策略、八文件原子事务、来源/结果/事务 Hash、内存 Pending、DPAPI Undo、旧入口不可达、孤儿旧状态精确对账、`RecoveryRequired` 惰性兼容与 UI 隐藏、真实运行态保护、精确 Agent Action JSON 指引、异步失败保留、稳定错误代码、原输入恢复、可中断 Agent Provider HTTP、取消安全点、可访问取消反馈、窗口关闭取消钩子和 GA2 脱敏证据。边界门继续禁止浏览器网络、自动 Fallback、Stronghold Guest、Shell、Process、任意路径、完整聊天持久化、自动跨 Provider 请求和未授权 Live Smoke。

## Tauri Command 登记规则

当前登记六十条命令：`health_check`、`app_state_load`、`app_state_save_ui`、`agent_skill_registry_load`、`agent_session_load`、`agent_turn_start`、`agent_turn_cancel`、`agent_session_clear`、`agent_policy_set`、`agent_write_confirm`、`agent_write_reject`、`agent_undo_apply`、`workflow_registry_load`、`router_resolve_local`、`router_run_start`、`router_run_cancel`、`workflow_run_status_load`、`workflow_source_catalog`、`workflow_run_prepare`、`workflow_run_generate`、`workflow_proposal_apply`、`workflow_run_cancel`、`workflow_run_abandon`、`story_context_catalog`、`story_context_build`、`story_context_request_preview`、`story_chapter_generate_start`、`story_chapter_status_load`、`story_chapter_generate_cancel`、`story_continuity_check`、`story_continuity_status_load`、`story_continuity_warning_confirm`、`story_chapter_patch_preview`、`story_chapter_patch_status_load`、`story_chapter_patch_apply`、`story_chapter_state_preview`、`story_chapter_lifecycle_status_load`、`story_chapter_state_apply`、`story_ledger_update_preview`、`story_ledger_update_apply`、`provider_status_load`、`provider_key_save`、`provider_key_delete`、`provider_select`、`provider_connection_test`、`provider_chat_start`、`provider_chat_cancel`、`project_scan`、`project_create_preview`、`project_create`、`project_open`、`project_open_recent`、`project_list_files`、`project_read_file`、`app_state_set_current_file`、`project_watch_file`、`project_save_file`、`project_validate_draft`、`project_search` 和 `project_close`。前端只能通过各自的类型化客户端调用。九条 Agent IPC 暴露固定 Skill 注册表、项目会话摘要/终态索引、严格 Turn 事件、取消、本机状态清空、策略切换、Hash 绑定确认/拒绝和受结果 Hash 保护的 Undo；确认与撤销 DTO 不接受正文、任意路径、完整聊天或模型提供的授权。Pending 正文只存在 Rust 进程内，重启、拒绝、项目关闭、Provider 切换或 Session 清空都会丢弃且保持零写入。Story Context、Chapter Proposal 和连续性命令分别暴露严格的清单、请求、运行与规则 DTO；旧 Patch/状态/Ledger Apply 继续只接受各自不透明标识、绑定 Hash 与显式确认。WebView 不获得完整 Prompt、原始 Provider Response、任意项目绝对路径、Undo 明文或可伪造的写入正文；Provider 客户端不接收 Key 特征或 `credential_ref`；项目根绝对路径只留在 Rust。

新增或修改 Command 必须在同一个 Story 中同时完成以下审查点：

1. 在 `apps/desktop/src-tauri/src/commands.rs` 实现窄职责、带类型的 Rust Command；
2. 在 `apps/desktop/src-tauri/src/command_manifest.rs` 登记命令名，使 `build.rs` 生成对应的 `allow-*`/`deny-*` 权限；
3. 在 `apps/desktop/src-tauri/src/lib.rs` 的唯一 `generate_handler!` 列表注册实现；
4. 在 `apps/desktop/src-tauri/capabilities/default.json` 明确引用所需的最小 `allow-*` 权限；
5. 在前端建立无任意命令名入口的类型化客户端、运行时解析和共享契约测试；
6. 为成功、受控错误、未登记命令和敏感字段泄漏补充正负测试。

禁止登记通用 Shell、Process、Opener 或“执行任意命令”能力。前端不得向 Command 传入可执行路径、命令行或任意 URL。命令注册表、handler、Capability 和双端契约四处不一致时，变更不得合并。
