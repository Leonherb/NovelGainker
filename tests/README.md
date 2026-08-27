# NovelGainker 测试与 Fixture 约定

## 测试放置

- React/TypeScript 单元与组件测试和被测源码放在同一目录，命名为 `*.test.ts` 或 `*.test.tsx`。
- Rust 单元测试优先放在源码文件的 `#[cfg(test)] mod tests` 中；真正跨模块的集成测试放在对应 Crate 的 `tests/*.rs`。
- 跨语言 IPC 契约夹具放在 `contracts/<area>/*.vN.json`，版本号是契约的一部分，Rust 和 TypeScript 必须读取同一文件。
- 跨 Workspace 的工作流 Golden Test 放在根目录 `tests/<area>/`；NG-301 的 Pack 解析、负面 Fixture、Hash 与嵌入资源测试在 `novelgainker-workflows` Crate 内与校验器共置。

## 命名与数据规则

- 测试名称描述可观察行为，不使用 `test1`、`works` 等无语义名称。
- Fixture 必须是 UTF-8、确定性、最小化的合成数据。
- Fixture 不得包含真实 API Key、用户目录、机器用户名、真实手稿或依赖外部网络的 URL。
- 单元测试不得读写用户项目；需要文件系统的后续 Story 必须使用测试创建并负责清理的隔离临时目录。
- Windows 凭据集成测试只能使用唯一的合成 Canary 和固定测试命名空间，并负责删除测试凭据；不得使用真实开发者 Key。泄漏测试必须动态生成 Canary，并扫描 SQLite 原始字节和前端状态序列化结果。
- NG-004 质量门故障探针使用 `__quality_*_probe__` 保留名；验证脚本拒绝覆盖同名现有文件，并在每次运行后清理。

## 运行入口

日常完整验证只运行：

~~~powershell
pnpm quality
~~~

Sprint 1 的独立无 Key、断网编辑闭环可运行：

~~~powershell
pnpm e2e:sprint1
~~~

NG-301 的独立真实 Tauri Workflow 入口证据可运行：

~~~powershell
pnpm smoke:ng301
~~~

该命令使用 Release 内部测试二进制，验证主窗口已渲染而非空白 WebView，并把截图写入 `target/ng301-evidence/tauri-ng301-window.png`。

NG-407/G4 的五工作流 Golden、双 Loopback、事务/负面路径、Canary 与真实窗口证据可独立运行：

~~~powershell
pnpm e2e:ng407
~~~

该命令只使用隔离合成项目和 Loopback Provider，生成脱敏 `target/g4-evidence/security-report.json` 与 `target/g4-evidence/tauri-g4-window.png`；不读取真实 Key、真实手稿或启动 Sprint 5 工作流。

NG-507/G5 的双 Provider Storytelling 状态机、章节/Ledger 事务、取消/恢复、Canary 与真实窗口证据可独立运行：

~~~powershell
pnpm e2e:ng507
~~~

该命令只使用隔离合成项目和 DeepSeek/OpenAI Loopback Provider，生成脱敏 `target/g5-evidence/`；默认不读取真实 Key、真实手稿或执行 Live Smoke。G5 已获产品批准并关闭 Sprint 5。Sprint 6 目前只有 Story Packet，NG-601 未授权，因此尚无 Sprint 6 测试入口。

修改质量门本身后，额外运行：

~~~powershell
pnpm quality:verify-gates
~~~

第二条命令会故意制造并清理格式、TypeScript、React 和 Rust 失败，用来证明聚合入口确实以非零状态阻断。
