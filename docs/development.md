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

~~~powershell
pnpm install --frozen-lockfile
cargo fetch --locked
pnpm quality
~~~

前两步是显式的依赖准备阶段，允许从锁文件下载缺失依赖。`pnpm quality` 本身不安装依赖，并为 Cargo 子命令设置离线模式；因此质量门不会在测试过程中临时访问网络。

若依赖缓存已准备好，可验证离线恢复：

~~~powershell
pnpm install --frozen-lockfile --offline
pnpm quality
~~~

## 唯一完整质量入口

日常提交前只需要运行：

~~~powershell
pnpm quality
~~~

聚合入口遇到第一个失败即返回非零，固定顺序如下：

| 顺序 | 阶段 | 独立命令 |
| --- | --- | --- |
| 1 | 前端格式检查 | `pnpm format:check` |
| 2 | ESLint，零警告 | `pnpm lint` |
| 3 | TypeScript 类型检查 | `pnpm typecheck` |
| 4 | React/TypeScript 单元测试 | `pnpm test:frontend` |
| 5 | Sprint 1～5 至 NG-507 的网络、文件、状态、凭据、Provider、Workflow Pack、Manager、Context、结构化输出、Router、Proposal、连续性、章节 Diff/Apply、状态迁移、Ledger、双 Provider G5 证据、恢复、Canary 和权限边界 | `pnpm quality:boundaries` |
| 6 | Rust 格式检查 | `pnpm format:rust` |
| 7 | Rust Clippy，`-D warnings` | `pnpm lint:rust` |
| 8 | Rust 离线锁定测试 | `pnpm test:rust` |
| 9 | Sprint 1 无 Key、断网 E2E | `pnpm e2e:sprint1` |
| 10 | Tauri Debug 内部构建 | `pnpm build:test:debug` |
| 11 | Debug 主窗口 Smoke Test | `pnpm smoke:desktop:debug` |
| 12 | Tauri Release 内部构建 | `pnpm build:test` |
| 13 | Release 主窗口 Smoke Test 与 G2 截图 | `pnpm smoke:desktop` |
| 14 | Sprint 2 Mock Contract、Canary Leak、DOM、离线编辑与 G2 证据 E2E | `pnpm e2e:sprint2` |
| 15 | Sprint 3 双 Provider Safe Apply、取消/冲突/恢复、Canary Leak 与 G3 技术证据 E2E | `pnpm e2e:sprint3` |
| 16 | NG-401 离线项目创建 E2E | `pnpm e2e:ng401` |
| 17 | NG-402 设定创建 E2E | `pnpm e2e:ng402` |
| 18 | NG-403 素材创建 E2E | `pnpm e2e:ng403` |
| 19 | NG-404 主剧情创建/替换 E2E | `pnpm e2e:ng404` |
| 20 | NG-405 人物 v2 创建/完善/升级 E2E | `pnpm e2e:ng405` |
| 21 | NG-406 确定性命令、Router 同意/分类/选择器与离线回归 E2E | `pnpm e2e:ng406` |
| 22 | NG-407 五工作流 Golden、双 Loopback、故障/泄漏矩阵与真实 G4 窗口证据 | `pnpm e2e:ng407` |
| 23 | NG-501 只读 Context Builder、路径/符号链接与稳定 Manifest E2E | `pnpm e2e:ng501` |
| 24 | NG-502 六字段 Reader Mental Model、预算、请求 Hash 与来源复核 E2E | `pnpm e2e:ng502` |
| 25 | NG-503 双 Provider Chapter Proposal、取消/修复/过期与零写入 E2E | `pnpm e2e:ng503` |
| 26 | NG-504 本地连续性规则、warning 确认、来源/目标过期与零 Provider/零写入 E2E | `pnpm e2e:ng504` |
| 27 | NG-505 章节 Diff、显式确认、事务恢复、原稿保全与 Ledger 不变 E2E | `pnpm e2e:ng505` |
| 28 | NG-506 独立状态确认、Ledger Diff/确认、pending 恢复、失败隔离与零后台重试 E2E | `pnpm e2e:ng506` |
| 29 | NG-507 DeepSeek/OpenAI 共用 Storytelling 状态机、失败路径零写入、手动编辑保护、G5 Manifest/Review/截图与未授权 Live 记录 | `pnpm e2e:ng507` |

两个构建命令只生成未签名内部测试二进制，不生成安装器。产物路径分别是 `target/debug/novelgainker-desktop.exe` 和 `target/release/novelgainker-desktop.exe`。Smoke Test 隐藏启动固定路径，要求进程在 15 秒内创建主窗口，然后按精确进程 ID 关闭；Release 步骤将窗口证据写入已忽略的 `target/g2-evidence/tauri-g2-window.png`。Sprint 2 E2E 生成 `target/g2-evidence/security-report.json`；Sprint 3 E2E 生成 `target/g3-evidence/security-report.json` 与 `target/g3-evidence/tauri-g3-window.png`。证据不得包含 Key、Prompt、Response、Usage、用户路径或真实手稿。NG-301～NG-303 的独立真实 Tauri 入口证据分别使用 `pnpm smoke:ng301`、`pnpm smoke:ng302` 与 `pnpm smoke:ng303`，生成到对应的 `target/ng30x-evidence/tauri-ng30x-window.png`，并拒绝近乎全白的未渲染截图。NG-304 的通用 Proposal UI 由完整预览/Diff 组件测试、严格 IPC 契约、合成 create/replace 与故障注入测试覆盖；NG-305 再以两家 Loopback 结构化输出 Contract、真实临时项目 Proposal 零写入/collision、前端修复/确认/刷新打开组件测试覆盖生产接线；NG-306 通过双 Provider 成功路径、取消/过期/冲突零写入、Applying 子进程中断/重启对账与高熵 Canary 扫描汇总 G3 技术证据。默认完整质量流程不需要 API Key 或外部网络，也不伪造真实 Provider Proposal 截图。NG-401～NG-407 各自可通过 `pnpm e2e:ng401`～`pnpm e2e:ng407` 独立重放；NG-406 生成脱敏 `target/ng406-evidence/security-report.json`，覆盖两家 Loopback、六个精确中文命令、同意前零请求、项目零读写、一次结构修复上限、固定选择器、App State v8 原文零持久化和普通对话/离线编辑回归。NG-407 生成 `target/g4-evidence/security-report.json` 与 `target/g4-evidence/tauri-g4-window.png`，汇总固定 Pack Hash、五工作流资源/Golden、双 Loopback、项目事务、负面写入、Canary 和 Sprint 1～3 回归；统一质量入口通过同次运行时间标记拒绝复用陈旧的 NG-401～NG-406 报告。NG-501～NG-507 分别通过 `pnpm e2e:ng501`～`pnpm e2e:ng507` 重放；NG-505 证据覆盖 create/revise Diff、确认前零写入、源 Hash 冲突、同目录事务、故障/崩溃恢复、原稿保全、Ledger 不变和零 Provider 请求。NG-506 生成 `target/ng506-evidence/lifecycle-ledger-matrix.json`，覆盖独立状态与 Ledger 两次确认、draft→final 阻断、可见 pending 恢复、固定 Ledger 目标、Ledger 失败隔离、零后台重试与零 Provider 请求。NG-507 生成 `target/g5-evidence/` 下的双 Provider Loopback、章节/Ledger 事务、取消/恢复、Manifest、Review Report、Live 未授权记录与 `tauri-g5-window.png`；默认入口不执行 Live Smoke。

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

~~~powershell
rustup show
rustup component add rustfmt clippy --toolchain 1.97.1-x86_64-pc-windows-msvc
~~~

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

G5 已获产品批准并关闭 Sprint 5。[Sprint 6 Story Packet](./stories/sprint-6-story-packet.zh-CN.md) 已创建，但 S6-Q001～Q007 和 Review Gate 尚未通过，NG-601 未授权；因此当前源码、命令清单、Workflow Pack 和质量入口尚不包含 analytical-review、图片导入或 EPUB 的 Sprint 6 运行时能力。

- 真实 API Key 只能由 Rust CredentialVault 写入 Windows Credential Manager；项目、SQLite、前端持久状态、日志和错误不得保存 Key 或其掩码、尾号、Hash、指纹；
- 项目访问必须绑定 Rust 持有的不透明会话与项目相对路径；
- Provider 网络只能由 Rust Provider Core 在用户显式连接测试或发送后访问固定 TLS Host、协议与模型；不得由 WebView 发起、自动切换或跨 Provider Fallback；
- Workflow Pack 只能来自编译期固定资源；运行时不得读取 Plugin Cache、Novelist、`SKILL.md`、Python、用户目录或网络，WebView 只能调用只读 `workflow_registry_load`；
- 不启用 Shell、Process 或 Opener 权限；
- 不打包 Python 或 Novelist 插件内容。

`pnpm quality:boundaries` 会检查五十一条精确 Command/Capability、Key-only UI 与提交前 Buffer 清理、受控文件路径、原子保存、单 Writer、脱敏恢复、只读搜索、App State Schema v8 及 v3/v4/v5/v6/v7 历史保留、固定 Windows CredentialVault/Provider Core、无 Key 读回、运行中切换锁、同 Provider Retry、NG-207 Leak/E2E 与独立 Live Smoke 边界、NG-301～NG-407 的固定 Workflow Pack、Manager、Context、结构化输出、Router、Proposal/Apply/Recovery、双 Loopback/Golden、G3/G4 Canary 与证据脱敏边界，以及 NG-501～NG-507 的 31 项固定 Pack 资源、只读 Context Manifest/Request Snapshot、24/48/24 KiB 预算、六字段 Reader Mental Model、双 Provider Chapter Proposal、本地连续性规则、warning/unknown 精确作者确认、确定性章节 Diff、三个独立最小 Apply DTO、源状态重检、同目录可恢复事务、写后严格校验、原稿保全、确认前零写入、严格状态矩阵、可见 Ledger Pending、固定 Ledger 目标、失败隔离、G5 Canary/Manifest/Review 和零后台重试；同时继续禁止浏览器网络、自动 Fallback、Stronghold Guest、Shell、Process、自动跨 Provider 请求或未授权 Live Smoke。

## Tauri Command 登记规则

当前登记五十一条命令：`health_check`、`app_state_load`、`app_state_save_ui`、`workflow_registry_load`、`router_resolve_local`、`router_run_start`、`router_run_cancel`、`workflow_run_status_load`、`workflow_source_catalog`、`workflow_run_prepare`、`workflow_run_generate`、`workflow_proposal_apply`、`workflow_run_cancel`、`workflow_run_abandon`、`story_context_catalog`、`story_context_build`、`story_context_request_preview`、`story_chapter_generate_start`、`story_chapter_status_load`、`story_chapter_generate_cancel`、`story_continuity_check`、`story_continuity_status_load`、`story_continuity_warning_confirm`、`story_chapter_patch_preview`、`story_chapter_patch_status_load`、`story_chapter_patch_apply`、`story_chapter_state_preview`、`story_chapter_lifecycle_status_load`、`story_chapter_state_apply`、`story_ledger_update_preview`、`story_ledger_update_apply`、`provider_status_load`、`provider_key_save`、`provider_key_delete`、`provider_select`、`provider_connection_test`、`provider_chat_start`、`provider_chat_cancel`、`project_scan`、`project_create_preview`、`project_create`、`project_open`、`project_open_recent`、`project_list_files`、`project_read_file`、`app_state_set_current_file`、`project_watch_file`、`project_save_file`、`project_validate_draft`、`project_search` 和 `project_close`。前端只能通过各自的类型化客户端调用。Story Context、Chapter Proposal 和连续性命令分别暴露严格的清单、请求、运行与规则 DTO；Patch Preview、状态 Preview 与 Ledger Preview 返回绑定 Hash 和 Diff，三类 Apply 都只提交各自不透明标识、绑定 Hash 与显式确认，不接受正文、任意路径或工具。Workflow Registry 客户端只获得展示字段；通用 Workflow Run prepare/generate/Apply 仍遵循各自最小 Hash 绑定 DTO。WebView 不获得完整 Prompt、原始 Provider Response、任意项目绝对路径或可伪造的写入正文；Provider 客户端不接收 Key 特征或 `credential_ref`；项目根绝对路径只留在 Rust。Router 三条 IPC 不接受项目会话、文件句柄、Context、Prompt、工具或写入参数。

新增或修改 Command 必须在同一个 Story 中同时完成以下审查点：

1. 在 `apps/desktop/src-tauri/src/commands.rs` 实现窄职责、带类型的 Rust Command；
2. 在 `apps/desktop/src-tauri/src/command_manifest.rs` 登记命令名，使 `build.rs` 生成对应的 `allow-*`/`deny-*` 权限；
3. 在 `apps/desktop/src-tauri/src/lib.rs` 的唯一 `generate_handler!` 列表注册实现；
4. 在 `apps/desktop/src-tauri/capabilities/default.json` 明确引用所需的最小 `allow-*` 权限；
5. 在前端建立无任意命令名入口的类型化客户端、运行时解析和共享契约测试；
6. 为成功、受控错误、未登记命令和敏感字段泄漏补充正负测试。

禁止登记通用 Shell、Process、Opener 或“执行任意命令”能力。前端不得向 Command 传入可执行路径、命令行或任意 URL。命令注册表、handler、Capability 和双端契约四处不一致时，变更不得合并。
