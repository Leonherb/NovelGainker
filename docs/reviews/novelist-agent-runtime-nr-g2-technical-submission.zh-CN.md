# NovelGainker NR-G2 技术提交

提交日期：2026-09-01  
实施范围：NR-101 → NR-106；NR-G2-R01 → NR-G2-R03 Remediation  
技术提交时状态：`NR-G2-technical-submission`  
当前 Gate 状态：`passed`  
产品批准：`nrG2Approved = true`（2026-09-01）

## 1. 技术结论

NR-1 已按批准顺序完成实现，并停止在 NR-G2 技术提交。工程对话的完整 History 仍是事实源；Host 现在可在主回复前按字节压力最多调用一次内部 Compactor，把可丢弃的自然语言 Memory 持久化到 `.novelgainker/memory/state.json`，再以固定系统指令、有效 Memory、最近精确 History 和当前消息组装主请求。

Compactor 不产生可见气泡。普通压缩失败会保留旧 Memory 并继续主回复；压缩阶段停止会中断在途 HTTP 并阻止主回复。两条 Provider 路径均为单次 HTTP attempt、零重试，并共享同一个 Turn Cancellation Token。Remediation 已补齐动态 System 指令端到端保真、有效 Memory 禁止降级近期重建，以及五栏目自然语言摘要的严格结构校验。

## 2. Story 交付

| Story     | 状态      | 结果                                                                             |
| --------- | --------- | -------------------------------------------------------------------------------- |
| NR-101    | completed | 工程绑定 Memory Store、Segment Cursor、连续覆盖、源 Hash、原子替换和无效缓存降级 |
| NR-102    | completed | 字节阈值、近期优先重建、前向衔接和每次至少一个 Segment 的渐进 Backfill           |
| NR-103    | completed | 当前 Provider/凭据下的纯文本 Compactor；单 attempt、可取消、零重试、校验后提交   |
| NR-104    | completed | 固定优先级 Context Assembler；主回复回退、当前用户消息一次且仅一次               |
| NR-105    | completed | 沿用完整中栏 Conversation、停止/重开/关窗语义；无新增 History/Memory UI          |
| NR-106    | completed | 中文长对话规模回归、候选 Profile 比较、参数冻结和 NR-G2 Manifest                 |
| NR-G2-R01 | completed | 双 Provider 动态 System 保真、四条实际请求体捕获和非法合同零出站                 |
| NR-G2-R02 | completed | 有效 Memory 增量不可执行时保留原字节并禁止近期重建                               |
| NR-G2-R03 | completed | 五栏目严格行语法、Prompt 对齐、反例矩阵及无效摘要回退                            |

## 3. 冻结 Profile

| 参数                     |       冻结值 |
| ------------------------ | -----------: |
| 压缩触发阈值             |       64 KiB |
| 受保护最近精确尾部       |       24 KiB |
| Context Segment 内容上限 |        6 KiB |
| Compactor 输入上限       |       72 KiB |
| Backfill 输入预留        |        8 KiB |
| Memory 摘要上限          |       12 KiB |
| Compactor 请求输出上限   | 1,024 tokens |
| 主回复请求输出上限       | 4,096 tokens |
| 单 Turn 累计请求输出上限 | 5,120 tokens |

选择 `balanced-64k`：在 Provider 的 64 消息、96 KiB 请求和 32 KiB 单消息硬边界内，即使同时存在最大当前消息和最大 Memory，仍能确定性容纳 24 KiB 受保护精确尾部。8 KiB Backfill 预留大于一个最大 6 KiB Segment 及标签开销，保证旧历史积压时每次成功压缩至少向后推进一个 Segment。

较早触发的 48 KiB 候选机械可行，但摘要空间偏紧；80 KiB 尾部优先候选也机械可行，但最大当前消息和摘要并存时余量更窄。此次选择只证明机械边界，不声称已经验证真实模型的摘要语义质量。

## 4. 机器声明

<!-- NR-G2-MANIFEST-CLAIMS:START -->

```json
{
  "status": "NR-G2-technical-submission",
  "nrG2Approved": false,
  "storiesCompleted": ["NR-101", "NR-102", "NR-103", "NR-104", "NR-105", "NR-106"],
  "remediationsCompleted": ["NR-G2-R01", "NR-G2-R02", "NR-G2-R03"],
  "dynamicSystemInstructionSerializationPathsTested": 4,
  "invalidSystemContractOutboundAttempts": 0,
  "validMemoryIncrementalUnavailableCompactorAttempts": 0,
  "summaryHeadingsRequired": 5,
  "selectedProfileId": "balanced-64k",
  "maximumOutboundAttemptsPerTurn": 2,
  "belowThresholdOutboundAttempts": 1,
  "compactionTriggeredOutboundAttempts": 2,
  "automaticRetriesPerTurn": 0,
  "skillCallsPerTurn": 0,
  "toolCallsPerTurn": 0,
  "canonOperationsPerTurn": 0,
  "visibleHistoryFixtureMessages": 10000,
  "providerMessageHardLimit": 64,
  "providerRequestHardLimitBytes": 98304,
  "currentMessageHardLimitBytes": 32768,
  "responseHardLimitBytes": 8388608,
  "longAssistantFixtureBytes": 54000,
  "longAssistantSegmentCount": 9,
  "currentUserMessageOccurrences": 1,
  "backfillMinimumProgressSegments": 1,
  "automatedExternalRequests": 0,
  "realProviderLiveSmokeExecuted": false,
  "realProviderSummarySemanticsVerified": false
}
```

<!-- NR-G2-MANIFEST-CLAIMS:END -->

该机器声明区块冻结技术提交生成时的事实，因此保留 `nrG2Approved = false`。产品负责人于 2026-09-01 作出的批准属于提交后的 Gate Decision，记录在本文页首、本节之后的 Gate Decision 与 BMAD 当前状态中，不重写历史 Manifest。

## 5. 验证范围

本地自动化覆盖：阈值前后、Memory 原子写入与损坏降级、UTF-8 Segment Cursor、超过 32 KiB 的助手消息、失败/取消/未生成语义、近期优先重建、重复增量压缩、Backfill 进度、10,000 条可见消息、Compactor 成功/普通失败/在途取消、主回复继续和两条零重试 Provider 原语。Remediation 进一步通过四条实际 Loopback 请求体证明双 Provider 动态 System 保真，通过有效 Memory 无前向/预算不足反例证明零 Compactor 降级，通过 LF/CRLF、缺失、重复、乱序、空栏目、前导文本、顶层 JSON、围栏和超限矩阵证明五栏目结构合同。

完整 `pnpm quality` 已通过：Prettier、ESLint、TypeScript、18 个 React 测试文件（102 项测试）、Workspace/NR-001～NR-006/NR-106 边界与证据检查、Rust Format、Clippy、Workspace Rust Tests、离线 E2E、Debug/Release 构建、Debug/Release 桌面窗口冒烟和未挂载 Canon 安全 Kernel 均为 PASS。

NR-106 自动化只使用 `127.0.0.1` Loopback，清除 Provider/Secret 环境变量并启用 Cargo Offline；桌面冒烟只验证本地应用窗口可启动。`automatedExternalRequests = 0`，未读取真实 Key，未执行真实 Provider Live Smoke。

## 6. 保持不变的产品边界

- 用户仍只在三栏布局的中栏自然聊天；没有 Anchor、Memory 卡片、History 右栏、压缩按钮或重建按钮。
- Memory 只是对话连续性的可丢弃派生状态，不是角色、世界观、设定、章节或 Canon。
- NR-1 不加载 Novelist Skill，不读取 Canon 作为模型上下文，不调用 Tool，也不写小说 Markdown。
- 未保存的手动 Markdown 仍是唯一关窗阻断；其他情况下取消活动 Turn 并直接关闭。

## 7. Gate Decision

`NR-G2-R01`～`NR-G2-R03` 已按批准顺序完成。产品负责人于 2026-09-01 明确批准本次 NR-G2 技术提交并确认 Gate 通过。

```yaml
status: passed
reviewStatus: approved
nrG2Approved: true
realProviderLiveSmokeExecuted: false
nr2Authorized: false
```

已批准 Remediation Packet：`docs/stories/novelist-agent-runtime-nr-g2-remediation-story-packet-draft-2.zh-CN.md`。

当前停止于 NR-G2 Gate；不执行真实 Provider Live Smoke，不形成或进入 NR-2，后续动作等待产品负责人另行授权。
