# NovelGainker NR-G3 技术提交

提交日期：2026-09-03  
实施范围：NR-201 → NR-206；NR-G3-R01 → NR-G3-R03 Remediation  
提交类型：NR-G3 Remediation 技术重新提交  
当前状态：`NR-G3-passed`  
产品批准：`nrG3Approved = true`

## 1. 技术结论

NR-2 已实现 Host Agent Loop / Tool Request-Result 基础设施；本次重新提交已经评审并获 NR-G3 产品批准，当前停止于通过后的 Gate。双 Provider Adapter 可用原生协议表达一次 Tool Request 与一次 Tool Result 续接；Host 在本地完成定义、Schema、参数、结果、调用次数、请求字节、输出预算、取消和 Task 状态校验。Conversation 仍只保存自然语言，Provider Continuation 仅在当前 Turn 内存中存在。

生产 Registry 仍为空，因此当前发布版不会执行任何 Tool，也不会产生 Canon、文件、网络、Shell、Process、Skill 或发布副作用；普通自然对话继续走 NR-1 单次主回复路径。NR-2 交付的是后续能力可安全接入的执行底座，不是小说领域能力本身。

本次重新提交已关闭首次 NR-G3 评审的三个阻断：Task Store 在写盘前严格配对 Call ID、工具身份、Result/Task 终态和错误码，并在恢复前拒绝非法 Store；测试 Registry 改为封闭的具体 Executor 合规集合，不再存在通用测试注册入口；Memory/Compactor 与 Tool Loop 通过同一核心 Turn 编排函数完成三次 Provider 调用的组合规模验证。

## 2. Story 交付

| Story  | 状态      | 核心结果                                                                 |
| ------ | --------- | ------------------------------------------------------------------------ |
| NR-201 | completed | 严格 Tool 合同、空生产 Registry、独立 Task Store、Revision CAS 与恢复    |
| NR-202 | completed | DeepSeek/OpenAI 原生 Tool Request/Result、类型化 Continuation 与字节边界 |
| NR-203 | completed | Attempt 1 → 单 Tool → Attempt 2 有界循环、测试 Executor 与取消监督       |
| NR-204 | completed | 独立 Runtime View、Task Activity Event、中栏受控内联状态与乱序合并       |
| NR-205 | completed | 单 Round/单执行、零重试、去重、终态不可回退、30 秒/2 秒与累计预算        |
| NR-206 | completed | 双 Provider/失败/取消/恢复/UI/发布面规模回归与 NR-G3 Manifest            |

### 2.1 NR-G3 Remediation

| Story     | 状态      | 核心结果                                                                 |
| --------- | --------- | ------------------------------------------------------------------------ |
| NR-G3-R01 | completed | Call ID/工具身份/终态严格配对、Task 状态矩阵、错误码白名单与非法恢复拒绝 |
| NR-G3-R02 | completed | 封闭测试 Registry、具体 Executor 合规绑定及不合格注册反例                |
| NR-G3-R03 | completed | 同核心编排的 Compactor + Tool 三调用组合测试及新 Memory 消费证据         |

## 3. 冻结边界

- 每 Turn：Compactor 0/1、Agent Attempt 1/2、Provider HTTP Attempt 最多 3、Tool Execution 最多 1、Tool Round 最多 1、自动重试 0。
- 请求输出：Compactor 1,024 tokens；每次 Agent Attempt 4,096 tokens；累计上限 9,216 tokens。
- Tool：Definitions 16，Arguments 16 KiB，Result 24 KiB；单 Continuation Item 32 KiB，累计 48 KiB；完整 Provider Body 继续服从 96 KiB。
- 执行：Tool Deadline 30 秒；两秒取消确认只承诺给封闭 Registry 中已由同版本确定性测试验证的具体 Executor。Host 不宣称可以强制终止任意进程内代码。
- 第二次 Attempt 再请求 Tool 时零执行，并以 `agent.tool_round_limit` 结束 Conversation；已经完成的 Task 不回滚。

## 4. 机器声明

<!-- NR-G3-MANIFEST-CLAIMS:START -->

```json
{
  "status": "NR-G3-technical-submission",
  "nrG3Approved": false,
  "storiesCompleted": ["NR-201", "NR-202", "NR-203", "NR-204", "NR-205", "NR-206"],
  "providerAdapters": 2,
  "nativeToolRequestBodiesCaptured": 4,
  "maximumProviderAttemptsPerTurn": 3,
  "maximumAgentAttemptsPerTurn": 2,
  "maximumToolExecutionsPerTurn": 1,
  "maximumToolRoundsPerTurn": 1,
  "automaticRetriesPerTurn": 0,
  "compactorRequestedOutputTokens": 1024,
  "eachAgentAttemptRequestedOutputTokens": 4096,
  "cumulativeRequestedOutputTokens": 9216,
  "toolDeadlineSeconds": 30,
  "cancellationAcknowledgementMs": 2000,
  "maximumToolDefinitions": 16,
  "maximumToolArgumentsBytes": 16384,
  "maximumToolResultBytes": 24576,
  "maximumContinuationItemBytes": 32768,
  "maximumContinuationBytes": 49152,
  "providerRequestHardLimitBytes": 98304,
  "strictToolResultCallPairing": true,
  "strictTaskStateMatrixValidated": true,
  "taskErrorCodeAllowlistValidated": true,
  "invalidTaskStoreRecoveryRejected": true,
  "unqualifiedExecutorRegistrationRejected": true,
  "productionRegistryTools": 0,
  "sharedProductionTestTurnOrchestrator": true,
  "compactorMemoryConsumedByInitial": true,
  "compactorToolCombinedProviderAttempts": 3,
  "mainProviderAttemptsWithTool": 2,
  "toolExecutions": 1,
  "syntheticToolIdsInProduction": 0,
  "productionCommandCount": 24,
  "conversationCommandCount": 3,
  "skillCallsPerTurn": 0,
  "canonReadsPerTurn": 0,
  "canonWritesPerTurn": 0,
  "externalToolCallsPerTurn": 0,
  "automatedExternalRequests": 0,
  "realProviderLiveSmokeExecuted": false,
  "nr3EnteredOrPacketFormed": false
}
```

<!-- NR-G3-MANIFEST-CLAIMS:END -->

## 5. 验证范围

本地证据覆盖双 Provider Initial/Continuation 请求体、单 Call 聚合、未知/多 Call/跨 Provider/超限拒绝、一个 Tool Round、第二次 Tool Call 拒绝、Task 创建与三次 Revision、重复 Call ID、Tool Result 错配拒绝、严格 Task 状态矩阵、错误码白名单、非法 Store 恢复拒绝、封闭 Executor 注册、两秒取消、部分文本保留、Runtime View 严格解析、Snapshot/Event 乱序合并和受控 UI 文案。

组合规模测试使用同一个核心 Turn 编排函数，完成 Compactor → Initial → Tool → Continuation。捕获证据确认累计 Provider 调用恰好三次、Tool 执行一次、新五栏目 Memory 被 Initial 和 Continuation 使用、已压缩早期原文不重复发送、Compactor 输出不进入可见 Conversation。

所有 Provider 自动化只使用 `127.0.0.1` Loopback，并清除 Provider/Secret 环境变量；Cargo 使用 Locked + Offline。Debug/Release 桌面 Smoke 只验证本地窗口启动，不读取真实 Key，不调用真实 Provider。

完整 `pnpm quality` 已通过：Prettier、ESLint、TypeScript、18 个 React 测试文件（104 项测试）、NR-001～NR-006、NR-106、NR-206、Rust Format、Clippy、Workspace Rust Tests（app-core 109 项通过、4 项按授权边界忽略，其他 workspace/integration tests 全绿）、离线编辑闭环、Debug/Release Tauri Build、Debug/Release Windows 本地窗口 Smoke 与 Canon/Recovery Kernel 均为 PASS。Windows linker 仅报告生成 `.lib/.exp` 的提示性 warning，无测试或构建失败。

## 6. 产品边界

- 用户仍在三栏布局的中栏自然聊天；没有 Anchor、关键词 Router、Control JSON、第四栏、Task Center 或技术卡片。
- Conversation 与 Task 分别落终态；Task JSON 不进入可见消息或 Memory。
- `.novelgainker/tasks/session.json` 只保存关联 Hash、字节数、受控标签和状态，不保存原始 Arguments、Result、Continuation、Key 或绝对路径。
- NR-2 不加载 Novelist Skill，不读取或写入 Canon，不执行真实 Provider Live Smoke，不形成或进入 NR-3～NR-7。

## 7. Gate Decision

以下机器声明与 Manifest 保留技术提交生成时的冻结事实；产品负责人已于 2026-09-03 明确批准 NR-G3。当前 Gate Decision 为：

```yaml
status: passed
reviewStatus: approved
nrG3Approved: true
realProviderLiveSmokeExecuted: false
nr3ToNr7PacketAuthorized: false
```

NR-G3 已通过并停止于当前 Gate。该批准不授权真实 Provider Live Smoke，不授权形成或进入 NR-3～NR-7 Story Packet；后续动作另行授权。
