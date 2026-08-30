# NovelGainker 开发指南

## Windows 11 x64 前置条件

- Visual Studio 2022（“使用 C++ 的桌面开发”与 Windows SDK）
- Microsoft Edge WebView2 Runtime
- Node.js 24.11.1
- pnpm 11.19.0
- rustup

仓库通过 `rust-toolchain.toml` 固定 Rust 1.97.1、`x86_64-pc-windows-msvc`、rustfmt 和 Clippy。

## 环境准备

```powershell
pnpm install --frozen-lockfile
cargo fetch --locked
pnpm quality
```

依赖准备允许按锁文件下载。`pnpm quality` 自身不安装依赖，并把 Cargo 固定为 `--locked --offline`。缓存完整时可使用：

```powershell
pnpm install --frozen-lockfile --offline
pnpm quality
```

## 唯一完整质量入口

提交前运行：

```powershell
pnpm quality
```

它按以下顺序运行，任一步失败立即返回非零：

| 顺序 | 阶段                                              | 独立命令                           |
| ---: | ------------------------------------------------- | ---------------------------------- |
|    1 | Prettier 检查                                     | `pnpm format:check`                |
|    2 | ESLint 零警告                                     | `pnpm lint`                        |
|    3 | TypeScript 类型检查                               | `pnpm typecheck`                   |
|    4 | React/TypeScript 测试                             | `pnpm test:frontend`               |
|    5 | 35 Command / 14 Agent IPC 与静态安全边界          | `pnpm quality:boundaries`          |
|    6 | 九项 bundled Skill 资源锁                         | `pnpm skills-v2:check`             |
|    7 | Agent Protocol V2 Read Contract 锁                | `pnpm agent-v2-read-runtime:check` |
|    8 | Agent Protocol V2 Canon Contract 锁               | `pnpm agent-v2-write-runtime:check` |
|    9 | 保留写入 Kernel 合约锁                            | `pnpm agent-write-kernel:check`    |
|   10 | Rust 格式检查                                     | `pnpm format:rust`                 |
|   11 | 全 workspace/target/feature Clippy，`-D warnings` | `pnpm lint:rust`                   |
|   12 | Rust 离线锁定测试                                 | `pnpm test:rust`                   |
|   13 | Sprint 1 工程离线回归                             | `pnpm e2e:sprint1`                 |
|   14 | Tauri Debug 构建                                  | `pnpm build:test:debug`            |
|   15 | Debug 主窗口 Smoke                                | `pnpm smoke:desktop:debug`         |
|   16 | Tauri Release 构建                                | `pnpm build:test`                  |
|   17 | Release 主窗口 Smoke                              | `pnpm smoke:desktop`               |
|   18 | Provider/Vault/Canary 安全回归                    | `pnpm e2e:sprint2`                 |
|   19 | 保留 Agent 写入安全 Kernel                        | `pnpm e2e:agent-write-kernel`      |
|   20 | 保留 Canon/连续性安全 Kernel                      | `pnpm e2e:canon-kernel`            |
| 21–27 | NG-R101～NG-R106 与 GAR1 回归                    | `pnpm e2e:ngr101`～`e2e:gar1`      |
| 28–35 | NG-R201～NG-R208                                  | `pnpm e2e:ngr201`～`e2e:ngr208`    |
|   36 | GAR2 技术提交生成                                 | `pnpm e2e:gar2`                    |

真实 Provider Live Smoke 不属于默认质量门。只有产品负责人明确授权并已在应用内保存相应 Windows Credential Manager 凭据时，才允许运行 `pnpm smoke:provider:live`。

## 关键独立命令

```powershell
pnpm format
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test:frontend
pnpm quality:boundaries
pnpm skills-v2:check
pnpm agent-v2-read-runtime:check
pnpm agent-write-kernel:check
pnpm format:rust
pnpm lint:rust
pnpm test:rust
```

两个 Tauri 构建只生成未签名内部二进制，不生成安装器：

- `target/debug/novelgainker-desktop.exe`
- `target/release/novelgainker-desktop.exe`

Smoke Test 隐藏启动固定路径，要求进程创建主窗口，再按精确 PID 关闭。它不读取 Provider Key、不发送真实请求。

## GAR1 / GAR2 证据

证据写入已忽略的 `target/qa/`：

- `ng-r101`～`ng-r106`：逐 Story JSON 证据；
- `ng-r104/ng-r104-evidence.json`：120 章、50 万字与 48 个真值查询规模结果；
- `ng-r106/v1-retirement-manifest.json`：旧产品层调用者清零与等价回归映射；
- `agent-write-kernel/evidence.json`：未挂载写入安全回归；
- `gar1/gar1-manifest.json`：GAR1 技术提交 Manifest；
- `gar1/gar1-review-report.md`：技术评审报告。
- `ng-r201`～`ng-r208`：AM-R2 逐 Story JSON 证据；
- `ng-r208/ng-r208-evidence.json`：120 章、540,960 中文字符、48 查询与 72 个写入场景；
- `gar2/gar2-manifest.json`：GAR2 技术提交 Manifest；
- `gar2/gar2-review-report.md`：GAR2 技术评审报告。

GAR2 Manifest 必须保持 `gar2Approved=false`、`amR3Authorized=false`、`sprint6Authorized=false` 和 `realProviderLiveSmokeExecuted=false`，直至产品负责人作出独立决定。

## 测试与 Fixture

- TypeScript 测试与源码共置，使用 `*.test.ts` / `*.test.tsx`；
- Rust 单元测试使用 `#[cfg(test)]`；
- 退役产品命令只可在测试构建的公开 Harness 中直接调用，不能注册为生产 IPC；
- 所有 Fixture 必须是确定性合成数据，不得包含真实 Key、用户路径、真实手稿或联网依赖；
- Cargo 过滤器必须实际执行非零测试，证据脚本不得把 0 tests 记为通过；
- 路径、Hash、取消、恢复、事务、Undo、Canary 与双 Provider 负向回归不得弱化。

## 当前安全边界

- 小说 Canon 只来自受支持的工程 Markdown 与资源；
- `.novelgainker/agent/**` 保存工程内 History/Memory，但从 Canon 搜索和出版输入排除；
- Agent Protocol V2 分离 Conversation、Control、Content、Canon 和 Persistence；
- Host 独占路径、权限、预算、History 校验、事务和写入决定；
- AM-R2 只允许 Host 将来源绑定的作者决定或可见正文写入受支持的 Canon/Chapter Draft；长内容 Composition 仍固定为 0；
- DeepSeek/OpenAI 使用同一 V2 状态机和固定 Preset，不自动 Fallback；
- API Key 只由 Rust CredentialVault 写入 Windows Credential Manager；
- bundled Skill Pack 只读取编译期锁定资源，不读取外部插件缓存、用户 `SKILL.md` 或 Python；
- WebView 不获得 Shell、Process、Opener、任意 URL、绝对路径、原始 Provider Payload 或隐藏推理；
- 写入、事务、Undo、Serializer 和 Mark 已经由 V2 Host 协议挂载；旧入口保持退役，安全 Kernel 回归继续保留。

## Tauri Command 登记

当前生产命令固定 35 条，其中 Agent V2 IPC 固定 14 条。精确列表以 `apps/desktop/src-tauri/src/command_manifest.rs` 为唯一代码来源，并由以下位置逐项锁定：

1. `command_manifest.rs`
2. `lib.rs` 的 `generate_handler!`
3. 自动生成 permissions
4. `capabilities/default.json`
5. TypeScript typed client
6. `scripts/check-sprint0-boundaries.mjs`

任何新增或修改必须在同一 Story 中同步这六处并增加成功、受控错误、未登记命令和敏感数据负向测试。禁止登记通用 Shell、Process、Opener 或任意命令执行能力。

## 常见问题

### Cargo 离线缺包

在依赖准备阶段执行 `cargo fetch --locked`，不要从质量命令移除 `--locked` / `--offline`。

### 格式、lint 或类型失败

格式问题运行 `pnpm format` 后审查变更。不要关闭规则、增加 lint 例外、放宽警告阈值或用无依据断言掩盖错误。

### Windows linker message

MSVC 创建 Tauri 导入库时可能输出本地化 linker message。严格 Clippy 仍以 `-D warnings` 运行；不得用 lint suppression 绕过。

### Tauri Smoke 失败

先确认 C++ Build Tools、Windows SDK 与 WebView2 Runtime，再分别运行 `pnpm build:test` 和对应 `smoke:*` 命令区分构建与窗口创建问题。
