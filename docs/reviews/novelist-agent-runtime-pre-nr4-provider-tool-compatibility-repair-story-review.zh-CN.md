# Pre-NR-4 Provider Tool Compatibility Repair Story 独立技术评审

## 0. 评审状态

- Story：`PRE-NR4-PTC-001`
- 评审对象：`Provider Tool 传输身份、参数合同与真实兼容性修复` Draft 2
- 评审日期：`2026-09-04`
- 技术评审：**通过**
- 评审决定：`ready_for_product_decision`
- 剩余 Finding：**0 blocking / 0 major / 0 minor**
- 产品批准：**未批准**
- 宽范围实施授权：**未授权**
- 窄修复实施：**已获用户直接授权、已离线完成并通过质量门**
- 正式 Provider Live Smoke：**未授权、未执行**
- NR-G4：**继续保持 passed / approved，不重开**
- `NR-4～NR-7`：**未进入、未形成 Story Packet**

本决定只证明 Story 的问题陈述、合同、安全边界、验收条件与阶段路由已经达到产品决策质量；它不构成宽范围实施、读取 Credential、执行真实 Provider 请求或 NR-4 形成/进入授权。其后用户仅直接授权了 DeepSeek 空人物检查的窄修复；该实现已通过离线回归与质量门，但不改变本评审对其余宽范围的结论。

---

## 1. 评审范围与依据

评审只使用内容无关且处于当前授权范围内的依据：

1. `docs/stories/novelist-agent-runtime-pre-nr4-provider-tool-compatibility-repair-story.zh-CN.md`；
2. 当前 `provider_core.rs`、`canon_context.rs`、`tool_runtime.rs` 与 `capability_pack.rs` 的只读合同核对；
3. 已批准的 NR-3 Story Packet 与 NR-2～NR-7 Delivery Plan；
4. 用户主动使用应用形成的局部真实观察及受控 Task/Conversation 终态；
5. DeepSeek 与 OpenAI 的官方 Function Calling 文档。

本轮没有调用真实 Provider，没有读取 Credential，没有保存或复述原始 Arguments、Provider Payload、用户 Prompt、小说内容、Query、路径或 Result。

---

## 2. 事实边界复核

| 事项                       | 评审结论                                                                                                              |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| 首个真实 DeepSeek 失败     | dotted internal name 被放入 Provider function name，属于 wire identity 兼容缺陷                                       |
| wire name 暂定修正后的行为 | Provider 产生了可配对 Tool 请求，Host 创建了 Task 并完成名称恢复/Registry lookup                                      |
| 是否实际调用了工具         | **没有**。参数在 JSON/Schema/领域语义校验阶段失败，Task 未进入 `running`，Executor、Pack Resource 与 Canon I/O 均为 0 |
| Conversation 是否被阻断    | 没有。Host 形成受控失败 Tool Result，Attempt 2 继续，Conversation 最终 `completed`                                    |
| 是否已知具体非法参数       | 不知道。持久化只有原始参数 SHA-256、字节数、状态与受控错误码；“遗漏 required-null 字段”仍只是强假设                   |
| 是否属于 NR-G4 追溯失败    | 不属于。真实 Provider Live 原本不在 NR-G4 已批准证据范围内                                                            |
| 是否阻断 NR-4              | 默认阻断 NR-4 实施/完成入口；只有双 Provider Live 通过或产品明确书面风险豁免才可解除                                  |

Story 没有把“Tool request 产生”误写成“工具已执行”，也没有把 Conversation 的 `completed` 错写成 Tool Task 成功。

---

## 3. Draft 1 Findings 与关闭结果

| ID      | 严重度   | Draft 1 Finding                                                     | Draft 2 关闭方式                                                                                            | 状态   |
| ------- | -------- | ------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ------ |
| PTC-R01 | blocking | AC06 把 0 个 Canon operations 列为反例，与 Capability-only 合同冲突 | 明确 `canonOperations = []` 为正例；反例改为 9 项、非法 kind 或缺少语义必填字段                             | closed |
| PTC-R02 | major    | Live 成功与产品风险豁免两条 NR-4 入口规则互相冲突                   | 冻结默认 Live-verified 关闭路径与书面风险豁免路径；豁免不等于 Live 成功、不关闭为 verified、不自动授权 NR-4 | closed |
| PTC-R03 | major    | DeepSeek wire schema 没有冻结 required/optional 集合                | 增加根对象、Resource item、list/search/read 初次与分页分支的字段级矩阵                                      | closed |
| PTC-R04 | major    | AC08 要求请求体不变，却又要求修改 name/schema/description           | 改为预算与字节上限不变；请求体只允许 Story 明列的三类表示变化                                               | closed |
| PTC-R05 | major    | 禁止裁剪 ID 与现有 `capabilityId.trim()` 行为冲突                   | 要求 Capability/Resource ID 原值精确匹配，并加入首尾空白反例                                                | closed |
| PTC-R06 | minor    | list/search cursor 被误写成必须为空                                 | 改为省略表示首屏；非空合法 cursor 继续支持分页                                                              |
| PTC-R07 | minor    | `novelist.read_context@1` 容易误成复合名称                          | 改为独立 `name = novelist.read_context, version = 1`                                                        | closed |
| PTC-R08 | minor    | Live Evidence 的“摘要”可能被理解为文本摘要                          | 明确只允许 SHA-256，并显式禁止文本摘要                                                                      | closed |
| PTC-R09 | minor    | 缺少 dotted inbound 与未知 alias 的零执行反例                       | 加入 Registry/Executor 前拒绝及 Provider Core Evidence                                                      | closed |

第二轮对抗复核确认上述 9 项全部关闭，修订没有引入新的 blocking、major 或 minor Finding。

---

## 4. 核心合同复核

| 合同                       | Draft 2 结果                                                                          | 结论 |
| -------------------------- | ------------------------------------------------------------------------------------- | ---- |
| 单一产品 Tool              | canonical name/version 仍唯一，wire alias 不进入 Registry 或 Catalog                  | pass |
| 双 Provider identity       | Definition、stream/non-stream Call 与 Continuation 使用同一可逆映射；非法入站名零执行 | pass |
| Provider schema projection | OpenAI 保持 strict required-all；DeepSeek normal endpoint 使用字段级可省略投影        | pass |
| 纯语法规范化               | 只补固定 null/empty；不猜 Capability、Query、Scope、Path、Limit、Cursor 或类型        | pass |
| canonical 安全裁决         | 规范化后仍通过 required-all Schema 与 Executor 二层领域校验                           | pass |
| 独立空集语义               | Capability resources 与 Canon operations 可分别为空；空工程结果合法                   | pass |
| Agent Loop 上限            | 单 Tool、单 Execution、单 Round、最多两个主 Agent Attempts、自动重试为 0              | pass |
| 失败不阻断聊天             | 参数失败保持 Executor/I/O=0，但受控失败 Result 可继续自然回复                         | pass |
| 隐私与数据边界             | 原始参数先算 Hash/Bytes；规范化对象只在内存，原始内容不持久化、不进 UI/Evidence       | pass |
| Canon 与发布面             | Canon Writes、新 IPC/Event/UI、外部插件、Beta endpoint 与模型切换均为 0               | pass |
| 阶段边界                   | NR-G4 不重开；产品批准、实施授权、Live 授权/豁免和 NR-4 授权彼此独立                  | pass |

---

## 5. 剩余风险与后续证据

以下是已知但已被 Story 正确约束的风险，不是本次 Story 形成质量的 Finding：

1. 当前持久化证据不能确定真实 DeepSeek 参数失败的具体字段；获批实施后应先增加内容无关诊断分类和离线失败边界测试，不能按猜测做宽泛容错。
2. 离线 Loopback 不能证明真实 Provider 兼容；只有另行授权后的 DeepSeek/OpenAI Live Matrix 可以形成 Live-verified 证据。
3. 产品若选择书面风险豁免，只能解除明确指定的 NR-4 入口条件，必须保留 `live-waived` 或 `live-pending` 事实，不能宣称兼容性已经实网验证。

本轮是文档形成与评审，没有实现代码，因此没有运行 `pnpm quality`，也没有生成 Pre-NR-4 技术 Manifest。完整质量门属于获批实施后的 AC11。

---

## 6. 评审决定与下一步

`PRE-NR4-PTC-001` Draft 2 的独立技术评审通过，状态为：

- `formation_authorized = true`；
- `review_status = completed`；
- `review_decision = ready_for_product_decision`；
- `product_approved = false`；
- `implementation_authorized = false`（宽范围 Story）；
- `implementation_completed = false`（宽范围 Story；DeepSeek 空人物检查窄修复已直接授权并离线完成）；
- `formal_provider_live_smoke_authorized = false`；
- `formal_provider_live_smoke_executed = false`；
- `entry_blocker_for_nr4 = true`；
- `nrG4Approved = true`；
- `nr4EnteredOrPacketFormed = false`。

下一步只等待产品负责人决定：

1. 是否批准该 Story 的产品范围；
2. 是否授权离线实施；
3. 离线通过后，是另行授权双 Provider Live Matrix，还是书面接受 `live-waived` 风险。

任何一项都不得从另一项自动推导。
