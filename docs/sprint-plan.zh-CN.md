# NovelGainker NR-3 交付与 NR-G4 技术评审状态

更新时间：2026-09-04

## 当前结论

NR-201 → NR-206、NR-G3-R01 → NR-G3-R03 与 NR-301 → NR-306 均已按批准顺序完成。NR-G3 与 NR-G4 均已获产品批准。NR-G4 后续 Remediation、Follow-up、Second Remediation 与 Corrigendum 补丁链已清理，不再作为独立产品 Story、证据入口或历史评审层级：

- `NR-G3.status = passed`
- `NR-G3.reviewStatus = approved`
- `nrG3Approved = true`
- `NR-G4.status = passed`
- `NR-G4.reviewStatus = approved`
- `nrG4Approved = true`
- `realProviderLiveSmokeExecuted = false`
- `nr4EnteredOrPacketFormed = false`

产品负责人已批准 `NR-3 Story Packet Draft 2 Final` 与 NR-301 → NR-306。九项 Novelist Capability 继续以应用自有、编译期嵌入的只读 Pack 运行；当前实现中的严格 Schema、可继续分页和可配对失败续聊均作为 NR-3 基线行为评审，不再拆成 Gate 后补丁编号。测试通过不等于 NR-G4 产品批准。

## NR-3 当前规划状态

- Packet：`NR-3-D2-FINAL`
- Packet 状态：`approved / implemented`
- Gate 状态：`NR-G4 passed / approved`
- 已批准：`NR3-D2-R01～R06`、`FR01～FR02`
- 已完成：`NR-301～NR-306`
- 产品批准：`nrG4Approved = true`
- 真实 Provider Live Smoke：`not executed`
- NR-4～NR-7：`not entered / no Packet formed`
- 文档：`docs/stories/novelist-agent-runtime-nr3-capability-canon-read-story-packet-draft-2-final.zh-CN.md`

## 已完成 Story

| Story  | 状态      | 核心交付                                                                 |
| ------ | --------- | ------------------------------------------------------------------------ |
| NR-201 | completed | ToolDefinition/Call/Result、空生产 Registry、独立 Task Store 与 Revision |
| NR-202 | completed | DeepSeek/OpenAI 原生 Tool Call、Result 续接与类型化 Continuation         |
| NR-203 | completed | 单 Tool Round Host Agent Loop、测试 Executor、Deadline 与取消监督        |
| NR-204 | completed | Runtime View、Task Activity Event、中栏受控内联投影与 Revision 合并      |
| NR-205 | completed | 去重、CAS、恢复、终态独立、完整请求与累计输出预算                        |
| NR-206 | completed | 双 Provider/取消/恢复/UI/发布面回归与 NR-G3 Manifest/Report              |

## 已完成 NR-G3 Remediation

| Story     | 状态      | 核心交付                                                             |
| --------- | --------- | -------------------------------------------------------------------- |
| NR-G3-R01 | completed | Tool Result 严格配对、Task 状态矩阵、错误码白名单和非法恢复拒绝      |
| NR-G3-R02 | completed | 封闭测试 Registry、具体 Executor 合规资格和不合格注册反例            |
| NR-G3-R03 | completed | 同核心编排的 Compactor + Tool 三调用组合测试及新 Memory 实际消费证据 |

## 已完成 NR-3 Story

| Story  | 状态      | 核心交付                                                           |
| ------ | --------- | ------------------------------------------------------------------ |
| NR-301 | completed | 内置 Novelist Pack、九项 Capability、36 项 Resource 与来源完整性   |
| NR-302 | completed | 常驻 Novelist 身份、紧凑目录、唯一只读生产 Tool 与零 Host Router   |
| NR-303 | completed | Canon Allowlist、严格请求语义与有界 `list/search/read`             |
| NR-304 | completed | 来源化 Context Bundle、24 KiB 结果上限与 40 KiB Agent Loop Reserve |
| NR-305 | completed | 普通聊天、工具成功/失败续聊、Pack 原子降级与双终态闭环             |
| NR-306 | completed | 双 Provider Loopback、规模/反例/零写入回归与 NR-G4 Manifest/Report |

## 冻结 Profile

| 参数                                            |                         值 |
| ----------------------------------------------- | -------------------------: |
| Provider HTTP attempts / Turn                   |                        ≤ 3 |
| Agent attempts / Turn                           |                        ≤ 2 |
| Tool executions / rounds                        |                    ≤ 1 / 1 |
| Compactor requested output                      |               1,024 tokens |
| Each Agent requested output                     |               4,096 tokens |
| Cumulative requested output                     |               9,216 tokens |
| Tool deadline                                   |                 30 seconds |
| Qualified Executor cancellation acknowledgement |                    ≤ 2 sec |
| Tool Definitions / Arguments                    |                16 / 16 KiB |
| Tool Result / Continuation                      |                24 / 48 KiB |
| Production Tool                                 | `novelist.read_context` v1 |
| Capability Resources / Turn                     |               ≤ 8，ID 唯一 |
| Canon operations / unique files                 |                    ≤ 8 / 8 |
| Canon list paths / search matches               |                  ≤ 64 / 24 |
| Context Bundle                                  |                   ≤ 24 KiB |
| Agent Loop Reserve                              |                     40 KiB |
| Complete Provider Body                          |                   ≤ 96 KiB |

自动重试、修复模型、后台 Tool、并行 Tool 和多 Tool Round 均为 0。生产 Registry 精确包含一个只读工具；Canon 写工具与 Host 关键词 Router 均为 0。

## 产品边界

NR-3 只增加常驻 Novelist Capability Pack 与 Agent 自主请求的 Canon 只读上下文。用户仍在中栏自然聊天；Host 不按关键词选择 Capability，也不会因为 Provider 未请求工具而阻断普通回复。当前不创建、更新或删除小说 Markdown，不包含 Diff、确认、Undo 或冲突处理。

## Gate 证据

- Packet：`docs/stories/novelist-agent-runtime-nr3-capability-canon-read-story-packet-draft-2-final.zh-CN.md`
- 技术提交：`docs/reviews/novelist-agent-runtime-nr-g4-technical-submission.zh-CN.md`
- 当前精简重新评审：`docs/reviews/novelist-agent-runtime-nr-g4-gate-rereview.zh-CN.md`
- 当前唯一机器 Manifest：`target/qa/nr-306/nr-g4-manifest.json`
- 完整验收入口：`pnpm quality`

NR-G4 Manifest 中的 `nrG4Approved = false` 保留为批准前技术证据快照；当前 BMAD 状态与精简重新评审记录正式产品批准。流程停止于已通过的 NR-G4；不执行真实 Provider Live Smoke，也不进入或形成 NR-4～NR-7 Story Packet。
