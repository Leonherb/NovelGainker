# NR-4 实施与验证记录

批准：2026-09-05，NR-4-D1 / R01～R04；顺序 NR-401 → NR-406，NR-G5 技术提交后等待产品批准。最小只读 Live Smoke 明确放弃，真实 Provider 兼容性风险接受；不执行真实凭据/网络测试。

## NR-401 — 已完成

完成时间：2026-09-05 04:27 +08:00。

- `canon_change.rs`：五类目标、Mutation/来源/回执、严格 nullable 字段、完整读取及方法证据、身份保留、风险预检、有界变更记录、本地 CAS 决策和预算账本；无文件写入。
- 新建候选 `id: new`，由后续 Host 分配身份；outline 元数据合同明确，模板由 NR-403 挂载。
- 双 Provider wire 映射和 Schema 序列化测试已添加；生产 Registry 仍为唯一只读工具。
- `cargo test -p app-core canon_change --locked --offline`：15 passed。
- `cargo test -p app-core nr401_mutation --locked --offline`：1 passed。
- `cargo clippy -p app-core --all-targets --all-features --locked --offline -- -D warnings`：通过。
- `pnpm quality`：重跑后 exit 0，164 app-core passed / 4 subprocess fixtures ignored，104 frontend passed，离线 E2E、Debug/Release Build + Smoke、Canon/Recovery Kernel 全通过。

首次质量门因 NR-306 检查器硬编码“NR-4 尚未形成”失败；已改为验证明确的后续阶段批准、Live 放弃及章节分期，加入撤销任一批准/缺少 Live 决定的拒绝反例。NR-G4 历史报告机器声明不变；阶段进入后的回归输出单独写 `target/qa/nr-306/nr-g4-regression-manifest.json`，不覆盖历史技术 Manifest。

`canon_context.rs` 与 `conversation_runtime.rs` 在 NR-401 结束时的 SHA-256 与开始时一致，既有 DeepSeek 修复未被重写。仅合同完成，不声称 AI 保存或 Undo 已交付。

## NR-402 — 已完成

验证日期：2026-09-05。

- 新增持久变更库与具体 Executor：不可变 before/after 字节快照、五目录批次写入、幂等键、配额、同工程共享协调锁及 dirty revision。
- 持久提交回执是真相；Task 中断恢复会核对提交结果，不把已保存误报取消。外部修改导致无法回滚时保留文件与证据并暂停进一步写入。
- 13 项存储/事务测试通过，包含逐步故障、取消、重开、双 Coordinator、Task 落后、待确认配额和 Windows 重解析点拒绝。
- `pnpm quality`：exit 0；离线测试、Clippy、前端测试、Debug/Release Build 与 Smoke、Canon/Recovery Kernel 全通过。
- 生产聊天尚未挂载写工具；NR-403 接入有界循环，NR-404 接入交互，不能把核心执行器完成等同用户端自动保存已经交付。

## NR-403 — 核心实施与质量门完成

验证日期：2026-09-05。

- 应用投影 1.1.0，九能力 / 37 资源；新增应用自有 outline 模板，五类维护方法绑定 Host 回执。上游 0.2.0 / commit / archive 来源不变。
- 两次读、一次变更、四 Agent 加一次 Compactor；真实 Provider Body 与原生工具定义计费，写入前预留最终回复结果封装空间，提交后不再提供工具。
- `canon_change_read.rs` 只依据实际交付的完整方法与原文发放回执；分段拼接须连续、完整且 Hash 一致。截断方法/超限结果不发放证据；来源目录提供实际持久消息 ID/Hash。
- 双 Provider Loopback 经同一个 Host 核心编排分别完成空工程新建、已有角色先读后更新，真实 Markdown 和回执一致；错配、重放和额外读取拒绝。机械来源反例沿用 NR-401 合同测试，不声称验证了真实模型语义质量。
- `pnpm quality` exit 0：104 前端测试、Rust 全量测试、Clippy、离线回归、Debug/Release 构建与 Smoke、恢复内核全部通过。
- NR-G4 技术报告与历史 Pack 声明冻结不变；当前投影独立记录在历史合同回归 Manifest 的 `currentProjection`，不能冒称当前应用仍只具备历史合同。
- 桌面 Host 的自动保存挂载与编辑器保护、结果交互在 NR-404 一起接入，当前仅核心入口已具备能力，不声称用户端交互已经交付。

## NR-404 — 已完成

验证日期：2026-09-05。

- 桌面生产聊天挂载两工具运行时；只读会话仍走只读回退，普通聊天不要求调用工具。
- 新增四项封闭 IPC（当前总数 28），本地 CAS 确认/放弃、实际原文/候选 Diff、Host 保存状态内联；局部状态读取失败不禁用聊天。
- Host 先登记编辑保留状态，再把内容交给可编辑文本框；干净失焦释放，脏草稿保留保护。正在聚焦编辑的干净文件也保守视为编辑占用，因此 AI 可能转待确认。
- 旧 revision、重复确认、来源变化、磁盘变化、dirty/revision 冲突均有反例；旧候选不能强制覆盖，需重新准备。确认无需 Provider。
- `pnpm quality` exit 0：111 前端测试、Rust 全量测试、边界检查、离线回归、Debug/Release Build + Smoke 和恢复内核均通过。新增专项测试的客户端名称、窗口桩和焦点时序问题已修正后重跑。
- NR-G1～G4 当前回归输出使用独立 regression Manifest；冻结历史合同仍为 24 IPC，当前 28 IPC 独立记录，批准缺失反例拒绝。

## NR-405 — 已完成

- 持久逆事务保留原始 before/after 快照，先全批次 afterHash/dirty/CAS 校验；撤销创建只移除本次文件，更新恢复原文字节。
- Undo 提交标记前中断恢复到先前 committed；提交后维持 undone。外部改动不覆盖，保留 recovery_required，显式本地“重试安全恢复”仍只接受已知版本，不提供 force-overwrite。
- 配额统计含遗留临时文件及实际磁盘字节，保留事务状态更新空间；缺失快照不猜测或删除 Canon。
- 核心变更测试 38 passed，包括七个 Undo 故障位置、混合批次、CAS、取消、重开、外部编辑、只读不一致标记与配额反例；UI 失败后也重新加载真实状态。
- `pnpm quality` exit 0：112 前端测试、Rust 全量/Clippy、离线回归、Debug/Release Build + Smoke、恢复内核全部通过。

## NR-406 — 已完成

最终质量完成：2026-09-05 06:27:17 +08:00。

- 五目录在合成工程中逐类完成实际创建、更新/upsert、no_change 和 Undo；超限候选在记录及 Canon 落盘前拒绝。
- 双 Provider 实际完整链均为四次 Agent 加一次 Compactor、累计请求输出 17,408 tokens；新摘要持久化且用于主请求。测得最大请求体 DeepSeek 67,798 / OpenAI 67,801 字节，自动化外部请求 0。
- 普通零工具聊天、非法参数后自然回复、已提交后 Provider 503、Windows 大小写别名编辑保护反例均通过。没有把脚本输出等同真实模型语义能力。
- `pnpm quality`：25 步全部 exit 0；112 项前端测试、Rust 全量/Clippy、67 项 NR-406 定向检查、历史合同回归、离线编辑、Debug/Release 构建及启动 Smoke 全通过。
- `node scripts/nr406-nrg5-e2e.mjs --submit`：exit 0；验证 Hash、质量窗口与当前源码匹配后形成 `target/qa/nr-406/nr-g5-manifest.json`，`nrG5Approved=false`。
- 已同步现行产品、架构、开发、测试、BMAD 和根规则文档；保留历史 Gate 机器声明及既有 Pre-NR-4 删除，不改写 Codex 自动记忆。UI 保留三栏布局与内联渐进 Diff。

已停在 [NR-G5 技术提交](novelist-agent-runtime-nr-g5-technical-submission.zh-CN.md)，等待用户批准 Gate；未进入 NR-5、未执行真实 Live Smoke、未创建 Git commit。
