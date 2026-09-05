# NovelGainker 测试与 Fixture 约定

## 放置与命名

- React/TypeScript 测试与被测源码同目录，命名为 `*.test.ts` 或 `*.test.tsx`。
- Rust 单元测试优先放在源码的 `#[cfg(test)] mod tests`；跨模块闭环放在对应 Crate 的 `tests/*.rs`。
- IPC 合同 Fixture 放在 `contracts/<area>/`，由 Rust 与 TypeScript 共同验证。
- 测试名称描述可观察行为，不使用 `test1`、`works` 等无语义名称。

## 数据与安全

- Fixture 必须是 UTF-8、确定性、最小化的合成数据。
- 不得包含真实 API Key、用户目录、机器用户名、真实手稿或外部网络 URL。
- 文件系统测试只使用带唯一前缀的隔离临时目录，并在删除前验证规范化父目录与名称。
- Provider 测试只使用本机 Loopback；完整质量门以 Cargo 离线模式运行，不读取真实 Credential。
- 小说 Markdown、素材与图片不得作为旧应用状态清理或测试重建的目标。

## 当前关键回归

- `ConversationView.reconciliation.test.tsx`：listener readiness、终态 Event 丢失、四种 Store 终态、同步失败与重试。
- `project::tests`：Project Metadata v1→v2 一次性 Clean Slate、二次打开、损坏/未知 schema、失败注入与 reparse 边界。
- `nr_g1_r02_clean_slate.rs`：同根旧 Project ID 的全局精确清理，以及 UI/Provider/无关项目状态保留。
- `nr-g1-gate-lib.mjs`：递归发布面、五层 IPC 闭包、临时违规生产文件与 Report 漂移负向验证。
- `tool_runtime::tests` / `task_store::tests`：严格 Tool 合同、历史空 Registry/当前封闭两工具注册、Task Revision/CAS、重复 Call 与中断恢复。
- `provider_core::tests` / `conversation_runtime::tests`：双 Provider 原生续接、单 Tool Round、取消、部分文本和累计预算。
- `ConversationView.reconciliation.test.tsx`：Task Snapshot/Event 只按更大 Revision 合并，终态不可回退。
- `nr206-nrg3-e2e.mjs`：零合成 Tool 发布泄漏、24/3 IPC、冻结预算与 NR-G3 机器声明。
- `canon_change` / `canon_change_store`：五类来源/读取合同、实际事务、失败注入、配额、CAS、Undo 与外部修改保护。
- `conversation_nr403_tests.rs`：双 Provider 真实回执驱动的读后写、4 Agent + 1 Compactor 极值链、普通聊天与失败续聊；本文件由 include! 引用，单独使用 rustfmt 检查。
- `CanonChangeView.test.tsx` / `canon-change.test.ts` / `ProjectWorkspace.test.tsx`：Host 真相、局部 Diff、封闭 IPC 和编辑保护握手。
- `nr406-nrg5-e2e.mjs`：五目录真实事务、请求体预算观测及技术提交证据；当前 28/3 IPC，旧 24/3 仅是历史合同。

## 运行入口

```powershell
pnpm typecheck
pnpm test:frontend
cargo test --workspace --locked --offline
pnpm e2e:sprint1
pnpm e2e:nr206
pnpm quality
```

`pnpm quality` 是唯一完整质量门；包含格式、lint、TypeScript、React、NR-001～NR-006/106/206/306/406 Evidence、Rust、离线编辑闭环、Tauri Debug/Release 构建和 Windows 离线 Smoke。NR-4 实施已授权，但真实 Provider Live Smoke 本次明确放弃，NR-G5 产品批准另行进行。合成测试不能冒充真实模型语义验证。
