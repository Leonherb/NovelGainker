# NR-G5 技术提交 — NR-4 Canon 维护、Diff 与 Undo

日期：2026-09-05。当前状态：NR-401～406 已完成，NR-G5 技术提交完成，等待用户批准。产品 Gate：`nrG5Approved=false`。

## 范围与决定

NR-4-D1 / R01～R04 已批准，按 NR-401 → NR-406 顺序实施。六项 Story 各自完整 `pnpm quality` 已通过。NR-G4 保持 passed/approved；不恢复 Pre-NR-4，不进入 NR-5～NR-7。

用户明确放弃本次最小只读 Provider Live Smoke，接受真实工具兼容性未验证风险；NR-6 完整验证保留。正式章节独立来源延至 NR-5，NR-4 仍支持用户明确引用章节的五类资料任务。

## 技术交付

- 写权限工程的生产运行时已挂载 `novelist.read_context` 与 `novelist.propose_change`；模型自主选择内置方法，无 Host 关键词分流。
- 九能力 / 37 资源，应用投影 1.1.0；新大纲模板为应用自有资源，上游 novelist 0.2.0 来源与许可证不变。
- 五类资料真实创建、更新、无变化、Diff、本地 CAS 确认/放弃及按变更集 Undo。
- 引用、方法、完整读取、目标身份、Hash、编辑 revision、路径、Writer Lease、配额均由 Host 校验；不可变快照先于 Canon，提交回执先于“已保存”。
- Undo 有持久逆向意图；中断未提交则恢复撤销前 Committed，外部修改不覆盖，恢复未决保留证据并阻止新写。
- 保存结果与聊天终态独立；工具失败可继续自然回复，提交后 Provider 失败不能否认保存。待确认草稿不增加关窗阻断。
- 四项新增封闭 IPC 与原有三栏相容；Host 编辑保护握手完成前不能输入，脏草稿失焦也不释放保护。Windows 大小写路径别名共用登记并有反例。

## 验收矩阵

| 验收 | 可复验证据 | 限定 |
| --- | --- | --- |
| 五目录创建/更新/无变化/Undo | canon_change_store 的 nr406_all_five_directories 测试；实际临时 Markdown/Hash/数量断言 | 每次至多四目标，小文件预算 |
| 来源四类 | canon_change 合同与 Store 反例 | 用户决定/接受建议、AI 建议、章节拒绝分别验证；不证明真实模型语义判断 |
| 完整读后写 | conversation_nr403_tests 与实际 read Executor 分段测试 | 来源和方法/原文回执来自实际 Host 返回 |
| 最大链 | 双 Provider 均实际捕获 4 Agent + 1 Compactor / 17,408 requested tokens | 新 Memory 落盘并在主请求中消费；完整 HTTP JSON body 按字节检查 |
| 零写入/失败保文 | 普通零工具、非法写参数、超限候选、提交后 Provider 503 | 正常回复保留，不把 Task 成功当已保存 |
| 冲突、取消、重开 | 源 Hash/CAS/dirty、逐文件故障、提交前后、Undo 七阶段及外部编辑测试 | 复合中断包含持久状态重建；底层 Kernel 另有真实子进程终止测试 |
| 发布与权限 | 28 IPC 五层精确闭包；Pack integrity；Secret/retirement 边界 | 未增加任意 Shell、URL、文件写通道或凭据读取 |
| UI/关闭 | CanonChangeView、ProjectWorkspace、ConversationView 回归 + 离线桌面 Smoke | Smoke 验证构建启动，不冒充真实 Provider 人工交互验证 |

最大调用链定向观测：DeepSeek 最大请求体 67,798 字节，OpenAI 67,801 字节，均低于 262,144 字节硬上限。最终数值以 `validation.json#loops` 为准。双 Provider 自动化外部请求均为 0。

## 命令与产物

- 定向验证：`pnpm e2e:nr406`，聚合 Rust 合同/事务/Loopback/恢复测试，要求非零测试及实际极值链标记。
- 完整验证：`pnpm quality`，包含前端、边界、全部历史回归、NR-406、Rust、离线编辑、Debug/Release Build + Smoke。
- 技术提交：`node scripts/nr406-nrg5-e2e.mjs --submit`，只有当前源码 Hash 与同一次完整质量记录匹配才写正式 Manifest。
- 验证记录：`target/qa/nr-406/validation.json`。
- 完整质量记录：`target/qa/nr-406/quality.json`。
- 技术 Manifest：`target/qa/nr-406/nr-g5-manifest.json`（已生成，产品批准为 false）。
- Story 级记录：[NR-4 实施日志](novelist-agent-runtime-nr4-implementation-log.zh-CN.md)。

历史 NR-G1～G4 的机器声明不改写；当前回归使用独立 regression Manifest。技术提交不包含自动 Git commit、真实凭据测试或产品 Gate 批准。

### 本次实测结果

- 完整质量窗口：2026-09-05 06:24:11～06:27:17 +08:00；`pnpm quality` exit 0，25 步全部 exit 0。
- 前端：20 个测试文件、112 项测试通过。Rust 全量测试、格式与 Clippy 通过；NR-406 单独聚合 67 项检查（包含于全量测试，不重复计算成新增测试总数）。
- NR-406 检查分组通过数：Canon Change 41、对话 Loopback 4、Provider 连续配对 2、Canon Apply 5、Recovery 7、Coordination 2、Task Store 6；受控子进程 Fixture 在父测试中运行。
- Debug/Release Tauri 构建和进程窗口启动 Smoke 均通过。没有执行新界面的真人操作或真实 Provider 测试。
- `node scripts/nr406-nrg5-e2e.mjs --submit` exit 0；校验验证时间处于上述质量窗口，且报告声明、当前发布源码与完整质量 Hash 一致。
- 本地版本：应用 0.1.0；Node 24.11.1；pnpm 11.19.0；rustc 1.97.1（8bab26f4f）；cargo 1.97.1（c980f4866）。
- 发布源码 SHA-256：`25cfca2303df20921354cbc67cfd48821a095a751513f12b10d28d99f558c34c`。
- Pack SHA-256：`d3cb171976129f0b92ed7de237a0d45ca94664ee2b0dafdbc88fe2860ce38929`。
- 工作区基础 HEAD：`1cf11b9dda4143d8c376e94196c9aa319490a444`；交付位于未提交工作区，HEAD 不是本次实现的提交号。

## 已知限制与风险

1. 真实 DeepSeek/OpenAI 工具兼容性、是否正确理解“只读/假设/接受建议”、方法选择、长篇摘要与写作质量未正式验证。受控 Fixture 不能代替这些结论；本次风险已明确接受。
2. 正式章节写入、独立章节来源、图像执行、出版与完整连续性闭环未开放，方法资料存在不等于可执行能力。
3. 每变更四目标/候选合计 8 KiB、每文件 8 KiB 起步；无法证明原始磁盘字节与规范化文本一致时拒绝 AI 更新。手动编辑的较大文件能力不扩大 AI 写权限。
4. 聚焦的干净编辑器也保守占用，可能使 AI 转待确认。旧候选遇到编辑 revision 变化需重新准备，不支持强制覆盖。
5. 多文件一致性仅在应用协调范围内；外部编辑会产生无法安全自动回滚的状态。保留 Canon 与快照，显式核对/安全重试，恢复后重开刷新写权限；不能清空状态绕过。
6. 变更库满额不自动驱逐 Undo；NR-4 未提供记录清理 UI。缺失或损坏快照不能重建历史 Undo，文件本身不因此删除。
7. 本次为实现者自检与自动化回归，未声称独立人员评审；本轮没有启动额外代理。

## Gate Decision

- 技术提交：完成，完整质量和匹配源码的 Manifest 已生成。
- 产品批准：未批准，`nrG5Approved=false`。
- 下一步：停止实施，等待用户批准 NR-G5；不得自动进入 NR-5。

## 冻结机器声明

以下为技术提交合同，不代表产品批准；须与实际验证输出完全一致。

<!-- NR-G5-MANIFEST-CLAIMS:START -->
```json
{
  "status": "NR-G5-technical-submission",
  "nrG5Approved": false,
  "nrG4Approved": true,
  "productionCommandCount": 28,
  "conversationCommandCount": 3,
  "productionTools": [
    "novelist.read_context",
    "novelist.propose_change"
  ],
  "capabilityCount": 9,
  "resourceCount": 37,
  "projectionVersion": "1.1.0",
  "canonDirectories": [
    "characters",
    "world",
    "materials",
    "plot",
    "outlines"
  ],
  "maximumAgentAttempts": 4,
  "maximumCompactorAttempts": 1,
  "maximumReadRounds": 2,
  "maximumMutationRounds": 1,
  "cumulativeRequestedOutputTokens": 17408,
  "maximumProviderBodyBytes": 262144,
  "maximumContinuationBytes": 163840,
  "maximumTargets": 4,
  "maximumCandidateBytes": 8192,
  "changeStoreBytes": 67108864,
  "maximumChangeRecords": 100,
  "maximumPendingChanges": 10,
  "independentChapterSourceImplemented": false,
  "realProviderLiveSmokeExecuted": false,
  "providerSemanticQualityValidated": false,
  "automatedExternalRequests": 0,
  "liveSmokeDecision": "explicitly_waived_risk_accepted"
}
```
<!-- NR-G5-MANIFEST-CLAIMS:END -->
