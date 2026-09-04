# NR-G4 技术提交：常驻 Novelist Capability Pack / Canon Read Context

## 0. 提交状态

- Gate：`NR-G4`
- 状态：**technical_submission / changes requested**
- 产品批准：**否**（`nrG4Approved = false`）
- 已完成 Story：`NR-301 → NR-306`
- Provider Evidence：仅 `127.0.0.1` 本地 Loopback
- 真实 Provider Live Smoke：**未执行**
- NR-4～NR-7：**未进入，未形成 Packet**

本报告是对已批准 NR-3 Story Packet 的原始技术提交，不是 Gate 自批准。产品负责人已暂不批准 NR-G4，并要求只形成 `NRG4-R01～R03` Remediation Packet；`NRG4-D1-R01～R03` 评审意见已接受，当前只能最终评审 Draft 2，不得实施补救、执行真实 Provider Live Smoke，也不得形成或进入 NR-4～NR-7。

## 1. 技术结果

NR-3 已把 `novelist@0.2.0` 落成应用自有、编译期嵌入、可校验的只读运行投影 `novelgainker.novelist/v1`。主 Agent 每轮获得固定 Novelist 身份与九项紧凑能力目录，但普通聊天仍可一次回复完成；Host 不扫描关键词、不预选 Capability。

生产 Registry 现在精确包含一个只读工具：Provider 可见名称 `novelist.read_context`，合同版本独立为 `1`。模型可在一次 Tool Round 中自主选择一项 Capability、最多八项唯一 Capability Resource，以及最多八个 Canon `list/search/read` 操作。Host 在 I/O 前完成通用 Schema 与 Executor 语义校验，然后只从 `project.md` 和八个小说源目录读取已保存 Markdown。

工具成功、可配对失败和普通不调用工具均保持 Conversation 自然语言闭环。工具数据只进入 Provider Continuation，不写入 History/Memory/Canon；参数越界或资源失败只终止对应 Task，Attempt 2 仍可完成 Conversation。

## 2. Story 交付对账

| Story    | 技术交付                                                                            | 结果 |
| -------- | ----------------------------------------------------------------------------------- | ---- |
| `NR-301` | 九能力 Pack Clean Cut、来源/License/Attribution、36 资源 Inventory 与静态完整性验证 | pass |
| `NR-302` | 固定 Agent 身份、紧凑目录、唯一封闭生产 Tool、零 Host Router                        | pass |
| `NR-303` | Agent-only Canon Allowlist 与有界 `list/search/read`，稳定快照和只读执行            | pass |
| `NR-304` | 来源标签、24 KiB Bundle、40 KiB Agent Loop Reserve、96 KiB Provider Body            | pass |
| `NR-305` | 普通聊天、工具成功/失败续聊、Pack 原子降级、Conversation/Task 双终态                | pass |
| `NR-306` | 双 Provider Loopback、规模/反例/零写入/IPC/UI 扫描与机器 Manifest                   | pass |

完整本地质量门 `pnpm quality` 已通过，覆盖前端测试、Rust 全量测试、既有 Gate 回归、NR-301/NR-306、Debug/Release 构建及本地窗口 Smoke；自动外部请求为 0。

## 3. 关键证据

### 3.1 Pack 与发布自包含

- 来源固定为 `novelist@0.2.0`，Commit `64e4a2e1473ff0bad5893eff394ccbf1fa1cd580`。
- 运行投影固定为 `novelgainker.novelist/v1`，九项 Capability 精确且唯一。
- Inventory 为 36 个资源、87,982 字节，Pack SHA-256 为 `0ea91a3028daffe09663a81ec344da61b4868b926557d68a023fed7ed418b50d`。
- 运行时通过静态 `include_str!` 资源表加载；不发现插件缓存、用户 `SKILL.md`、项目伪装能力、Shell、Process、Python 或网络资源。
- 篡改、重复、缺失、未知字段、Hash/大小不符均 Fail Closed；失败时目录、资源和 Registry 原子移除，固定 Novelist 对话身份保留。

### 3.2 模型选择与双 Provider

- DeepSeek 与 OpenAI 请求捕获均携带相同能力目录和同一个 `novelist.read_context` Definition。
- 两种 Adapter 的 Tool Call 与 Tool Result Continuation 均使用正式名称；`version = 1` 只作为 Host 合同字段，不拼进 Provider 名称。
- 同一用户文本原样进入双方 Adapter；请求中没有 `selectedCapability`、Router 结果或关键词标签。
- 两个不同合法 Capability 选择会得到各自不同的指令资源，Host 不替换模型选择。
- 所有自动 Provider Evidence 均来自本地 Loopback，不读取真实 Credential。

### 3.3 Canon 边界与零写入

- 根目录仅 `project.md`；另允许 `characters/**`、`materials/**`、`macguffins/**`、`plot/**`、`outlines/**`、`world/**`、`style/**`、`chapters/**`。
- `AGENTS.md`、其他根级 Markdown、`.novelgainker/**`、`assets/**`、`published/**`、绝对/穿越/反斜杠路径及非 UTF-8 来源不会成为 Canon Context。
- `list` 最多 64 路径，`search` 最多 24 命中；每 Turn 最多 8 操作、8 个唯一 Canon 文件。
- Canon 来源携带相对路径、内容 Hash、大小、范围与继续游标；读取后再次核对文件 Hash，避免混合两个版本。
- Prompt Injection 文字只作为 `canon_snapshot` 数据返回，不能扩张工具或权限。
- 测试前后 Canon 文件集合、内容、Hash 与 mtime 相同；生产 Canon 写路径为 0。

### 3.4 预算、失败与长期对话

- Capability 基础指令 3 KiB、可选参考合计 6 KiB、Canon 内容/片段合计 10 KiB、单片段 4 KiB。
- Context Bundle 硬上限 24 KiB，主请求预留 40 KiB，完整 Provider Body 硬上限保持 96 KiB。
- 长 History、有效 Memory、近最大用户消息与近 24 KiB Tool Result 的双 Provider Continuation 组合均在出站前通过完整 Body 校验。
- 10,000 条可见消息规模测试保持最近精确 History 优先，当前用户消息只出现一次。
- 0/8 项 Capability Resource 正例，以及 9 项、重复、未知资源和越权路径反例均通过；可配对失败进入 Attempt 2，Conversation 可保持 completed。

## 4. 未验证与明确不在本 Gate 的内容

- 未验证真实 DeepSeek/OpenAI 是否会在自然语言场景中稳定选择最佳 Capability 或 Tool 时机。
- 未验证真实 Provider 对正式 Tool Schema 的线上兼容性；这需要后续单独授权最小只读 Live Smoke，或由产品负责人明确接受不验证的风险。
- NR-3 没有 Canon 创建、更新、Diff、确认、Undo 或冲突处理；用户提出保存时，Agent 只能讨论或拟稿并说明尚未保存。
- 本 Gate 不包含 NR-4～NR-7 的设计或实施。

## 5. 评审建议

评审时重点确认：

1. 普通聊天是否仍是默认路径，而不是每轮强制读取 Canon；
2. Host 是否只机械执行模型请求，没有关键词或正则路由；
3. 唯一工具是否确实只读，路径、资源、数量和预算是否在 I/O 前收紧；
4. 工具失败是否只影响 Task，并允许 Attempt 2 正常完成 Conversation；
5. 是否接受在没有真实 Provider Live Smoke 的情况下批准 NR-G4，或要求另行授权该最小只读检查。

## 6. 机器声明

<!-- NR-G4-MANIFEST-CLAIMS:START -->

```json
{
  "status": "NR-G4-technical-submission",
  "nrG4Approved": false,
  "storiesCompleted": ["NR-301", "NR-302", "NR-303", "NR-304", "NR-305", "NR-306"],
  "sourcePackageId": "novelist",
  "sourcePackageVersion": "0.2.0",
  "sourceCommit": "64e4a2e1473ff0bad5893eff394ccbf1fa1cd580",
  "sourceArchiveSha256": "8b7c53e1832ee91fab29a320effe11efff9d3f07735b4ce8b3a541266bae85b8",
  "projectionId": "novelgainker.novelist",
  "projectionSchemaVersion": 1,
  "projectionContentVersion": "1.0.0",
  "capabilityCount": 9,
  "resourceCount": 36,
  "packBytes": 87982,
  "packSha256": "0ea91a3028daffe09663a81ec344da61b4868b926557d68a023fed7ed418b50d",
  "licensePresent": true,
  "attributionPresent": true,
  "productionRegistryTools": 1,
  "providerVisibleToolName": "novelist.read_context",
  "toolContractVersion": 1,
  "providerAdapters": 2,
  "providerRequestBodiesCaptured": 4,
  "providerEvidenceTransport": "127.0.0.1-loopback",
  "sameUserTextCapturedByBothAdapters": true,
  "providerCapabilityChoicesExecutedWithoutHostSubstitution": 2,
  "maximumCapabilityResourcesPerRequest": 8,
  "capabilityResourceIdsUnique": true,
  "canonOperationKindsCovered": 3,
  "maximumCanonOperationsPerTurn": 8,
  "maximumCanonFilesPerTurn": 8,
  "maximumCanonListPaths": 64,
  "maximumCanonSearchMatches": 24,
  "maximumContextBundleBytes": 24576,
  "maximumProviderBodyBytes": 98304,
  "agentLoopReserveBytes": 40960,
  "canonWritesPerTurn": 0,
  "keywordRouterBranches": 0,
  "userVisibleSkillSelectors": 0,
  "controlJsonMessages": 0,
  "newIpcCommands": 0,
  "productionCommandCount": 24,
  "conversationCommandCount": 3,
  "runtimeCapabilityExternalLoads": 0,
  "automatedExternalRequests": 0,
  "realProviderLiveSmokeExecuted": false,
  "nr4EnteredOrPacketFormed": false
}
```

<!-- NR-G4-MANIFEST-CLAIMS:END -->

机器 Manifest：`target/qa/nr-306/nr-g4-manifest.json`（由 `pnpm e2e:nr306` 生成，不提交临时 Target Evidence）。

## 7. 当前停止点

`NR-G4.status = technical_submission`，`NR-G4.reviewStatus = changes_requested`，`nrG4Approved = false`。原技术提交机器声明保持为不可改写快照；当前停止于 Remediation Draft 2 最终评审，不实施补救，不执行真实 Provider Live Smoke，不进入或形成 NR-4～NR-7 Story Packet。

## 8. Gate Review Decision

评审日期：2026-09-03。

结论：**NR-G4 暂不批准 / changes requested**。阻断范围精确为：

| Finding    | 阻断结论                                                                                             | 对应补救                                                     |
| ---------- | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| `NRG4-R01` | OpenAI `strict: true` 与当前非全字段 required Schema 不兼容，且 Provider 可见字段名偏离已批准 Packet | Strict-compatible Schema，并恢复唯一批准的 Provider 字段合同 |
| `NRG4-R02` | 合法 Context 请求可能因 24 KiB 组合大小整体失败；read/search 游标可能跳过内容                        | 单一实际序列化 Budget Writer 与无丢失分页游标                |
| `NRG4-R03` | 可配对 malformed arguments 在 Task 创建前被当作 Provider 协议失败，导致 Conversation 未完成          | 先配对建 Task、后解析，失败 Tool Result 进入 Attempt 2       |

当前补救产物：`docs/stories/novelist-agent-runtime-nrg4-remediation-story-packet-draft-2.zh-CN.md`。

`NRG4-D1-R01～R03` 评审结论已接受并关闭于 Draft 2；`NR-G4-R01～NR-G4-R03` 仍未获得实施授权。原机器声明区保留技术提交时事实，不因本评审决定回写为已批准或已补救。
