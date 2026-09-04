# NR-G3 Remediation Story Packet Draft 1

## 0. 文档状态

- Packet：`NR-G3 Remediation`
- 版本：`Draft 1`
- 状态：待评审
- 上游 Gate：`NR-G3 技术提交`
- 上游评审结论：`NRG3-R01～R03` 阻断，当前不得批准 `NR-G3`
- 实施授权：**未授权**
- 真实 Provider Live Smoke：**未授权**
- `NR-3`：**不得进入、不得形成 Story Packet**

本文档只定义关闭 `NRG3-R01～R03` 所需的最小补救范围。除非用户后续明确批准本 Packet 并授权实施，否则不得修改产品代码、测试、Gate 状态或机器证据。

---

## 1. 依据与目标

### 1.1 依据

本 Packet 以以下已批准或已提交材料为唯一上游依据：

1. `NR-2 Host Agent Loop / Tool Request-Result Story Packet Draft 2`；
2. 当前 `NR-G3 Technical Submission`；
3. 本轮 `NR-G3` Gate Review 对 `NRG3-R01～R03` 的评审结论。

若本文与已批准的 NR-2 合同冲突，以已批准的 NR-2 Story Packet 为准；本文不得借补救之名扩大产品能力。

### 1.2 补救目标

仅完成以下三项目标：

1. 在 Host 的 Task Store 终态写入与恢复读取边界闭合 Tool Result 严格配对合同；
2. 在 Tool Registry 注册边界闭合 Executor 取消合规合同，证明不合格 Executor 不可进入可执行 Registry；
3. 增加同一 Turn 内真实经过 `Compactor → Initial → Tool → Continuation` 的组合规模证据。

### 1.3 明确不做

本 Packet 不包含：

- 注册任何生产 Tool；
- 实现 Novelist Skill、Canon/Chapter 写入或其他 NR-3 能力；
- 修改三栏布局、Conversation UI 或 Task Activities UI；
- 增加工具轮次、重试次数或并行执行；
- 修改双 Provider 产品范围或 Credential 行为；
- 执行真实 Provider Live Smoke；
- 清理、迁移或兼容历史用户数据；
- 进入或形成 `NR-3～NR-7` Story Packet。

---

## 2. 冻结合同

以下既有 NR-2 合同保持不变：

- 每个用户 Turn 最多一个 Tool Call、一个 Tool Round、零重试；
- 无 Tool 时最多一次主 Provider 调用；
- 有 Tool 时最多两次主 Provider 调用；
- 同一 Turn 如先触发 Compactor，总 Provider 调用上限为三次；
- Tool Result 只进入受控 Provider Continuation，不作为普通用户消息；
- Conversation 与 Task Activities 分离持久化、分离展示；
- Production Registry 工具数保持 `0`；
- 用户停止、窗口关闭和工程关闭仍遵循现行 NR-0/NR-1 取消边界；
- `nrG3Approved` 在用户最终批准前始终为 `false`。

---

## 3. NRG3-R01：闭合 Tool Result 与 Task Store 严格配对

### 3.1 问题

当前正常 Agent Loop 会使用原 Tool Call 构造 Tool Result，但 Task Store 的终态转换边界没有完整拒绝以下输入：

- Result 的 `callId` 与 Task 保存的 Call ID 不一致；
- Result 的工具名或版本与 Task 不一致；
- Result 的状态与目标 Task 终态不一致；
- 持久化 Task 含有不可能的状态/字段组合；
- 标称 SHA-256 的字段长度正确但不是规范十六进制摘要。

仅依赖上游调用者“目前会传正确值”不足以构成持久化合同。

### 3.2 设计边界

#### A. Call ID 按不透明原始值配对

- `callId` 是不透明字符串，不做大小写转换、裁剪或 Unicode 归一化；
- Task 创建时继续只保存原始 `callId` 的 SHA-256，不落原始 Provider Payload；
- 写入 Tool Result 前，Host 必须以同一算法计算 `result.callId` 的 SHA-256，并与 Task 中的 `call_id_hash` 精确比较；
- 不一致时拒绝事务，不增加 Revision，不部分写入结果。

#### B. Tool 身份和终态一致

- `result.toolName` 必须等于 Task 的 `tool_name`；
- `result.toolVersion` 必须等于 Task 的 `tool_version`；
- 正常执行终态必须满足：
  - `ToolResult.succeeded → Task.succeeded`；
  - `ToolResult.failed → Task.failed`；
  - `ToolResult.cancelled → Task.cancelled`；
- 不允许用一个状态的 Tool Result 写入另一种 Task 终态。

#### C. 恢复取消是唯一无 Tool Result 的终态例外

进程或窗口中断后的恢复逻辑可能只有尚未完成的 `queued/running` Task，而没有可验证的 Tool Result。该路径只能：

- 由专用恢复函数执行；
- 将 `queued/running` 转为 `cancelled`；
- 写入既有、稳定、受控的恢复错误码；
- 不伪造 Tool Result、Result Hash 或 Result Bytes。

普通实时终态转换不得借用这一例外。

#### D. 严格持久化状态矩阵

加载和写回 Task Store 时至少验证：

| Task 状态 | Result Hash / Bytes | Error Code |
|---|---|---|
| `queued` | 必须不存在 | 必须不存在 |
| `running` | 必须不存在 | 必须不存在 |
| `succeeded` | 必须成对存在 | 必须不存在 |
| `failed` | 必须成对存在 | 必须存在且受控 |
| 实时 `cancelled` | 必须成对存在 | 必须存在且受控 |
| 恢复 `cancelled` | 必须同时不存在 | 必须是受控恢复错误码 |

`result_sha256` 与 `result_bytes` 不得只出现其中之一。所有 SHA-256 字段必须匹配规范的小写十六进制格式 `[0-9a-f]{64}`。

### 3.3 验收条件

1. 正确 Call ID、工具名、版本和状态可以完成一次 CAS 终态转换；
2. 错误 Call ID 被拒绝，Revision 和磁盘内容不变；
3. 错误工具名或版本被拒绝；
4. Tool Result 状态与 Task 目标终态不一致时被拒绝；
5. 非十六进制 Hash、半组 Result 字段及每一种非法状态组合在加载时被拒绝；
6. 恢复取消仍可处理没有 Tool Result 的遗留 `queued/running` Task；
7. Store 继续不保存原始 Provider Tool Payload；
8. 新增正例与反例测试，且现有 Task Revision/CAS、项目绑定和原子替换测试保持通过。

---

## 4. NRG3-R02：闭合 Executor 注册与取消合规

### 4.1 问题

当前 Host 会发出取消信号并等待确认，但在线程作用域内运行的 Executor 如果不响应取消，仍可能延迟作用域结束。仅验证 Tool Definition 无法证明 Executor 满足“取消后两秒内退出”。

NR-2 当前没有生产 Tool，因此本项只补齐 Registry 基础合同，不引入生产 Executor 或新隔离架构。

### 4.2 设计边界

#### A. 原始 Executor 不可直接注册

- Registry 的可执行注册入口保持封闭；
- 原始、未经确认的 Executor 不得仅凭 Tool Definition 进入可执行映射；
- 当前生产 Registry 继续由 `empty_production()` 构造，工具数严格为 `0`；
- 测试 Registry 只能接收由仓库内受控注册路径明确列入的合成 Executor。

#### B. 合规证据与可执行注册绑定

每一种允许注册的合成 Executor 必须具有同版本、同实现路径的确定性合同测试，证明：

- 正常完成可以退出；
- 主动失败可以退出；
- 收到取消后在两秒内确认并退出；
- 退出后不会继续产生 Task 写入、Conversation 提交或其他可观察副作用。

只有具有上述合规证据的受控 Executor 类型才能由注册路径生成可执行条目。后续生产 Executor 若要加入，必须另行评审并重复同一门槛；本 Packet 不预先批准它们。

#### C. 不合格 Executor 反例

增加一个故意忽略取消或超过两秒才退出的合成 Executor。它必须在注册/构造可执行 Registry 的边界被拒绝，且：

- 不得进入可解析工具列表；
- 不得被 Agent Loop 调用；
- 不产生 Task 或 Conversation 副作用。

本项不以“实际运行后让 Host 无限等待”作为反例测试方式；反例应在执行前被注册边界拦截，避免测试自身失控。

### 4.3 验收条件

1. 三种合格合成 Executor 的完成、失败和取消合同测试均通过；
2. 取消合规测试使用单调时钟，确认取消信号后两秒内完成；
3. 不合格 Executor 无法进入可执行 Registry；
4. 不合格 Executor 没有被执行，也没有产生持久化副作用；
5. Registry 仍拒绝重复工具名/版本和 Provider 可见名称冲突；
6. Production Registry 工具数仍为 `0`；
7. 不新增子进程、沙箱服务、后台守护进程或生产 Tool。

---

## 5. NRG3-R03：补齐 Compactor + Tool 单 Turn 组合规模证据

### 5.1 问题

现有测试分别证明了 Memory/Compactor 与 Tool Loop，但没有用同一个 Turn 证明：

`长 History → Compactor → Initial → Tool → Continuation → Conversation/Task Commit`

因此“同一 Turn 累计最多三次 Provider 调用”目前缺少组合运行证据。

### 5.2 设计边界

#### A. 仅增加测试注入缝

- 生产运行入口继续使用空 Production Registry；
- 可以增加仅测试可见的受控 Registry 注入缝，以便组合验证完整编排；
- 测试注入缝不得暴露为 Tauri Command、前端 API 或生产公共注册接口。

#### B. 单 Turn 三阶段脚本

Loopback Provider 在同一 Turn 中按确定顺序返回：

1. Compactor 请求返回有效五栏目 Memory；
2. Initial 请求返回一个合法 Tool Call；
3. Tool 成功后，Continuation 返回最终自然语言回复。

使用一个已通过 R02 合规注册的合成 Executor，且只允许执行一次。

#### C. 组合断言

测试必须同时证明：

- 累计 Provider 调用恰好为 `3`，顺序为 Compactor、Initial、Continuation；
- 主 Provider Attempt 仍恰好为 `2`，没有第四次调用或隐藏重试；
- Tool Executor 恰好执行一次；
- Task 依次进入 `queued → running → succeeded`；
- Continuation 只接收受控 Tool Result，不接收原始 Tool Payload；
- Compactor 的五栏目输出写入 Memory，但不作为 Assistant 消息显示；
- Conversation 只提交一次最终自然语言回复；
- Task Activities 与 Conversation 仍分离；
- 请求体、Memory、Task 与 Conversation 的大小边界继续生效。

该组合测试验证 Provider 无关的 Host 编排；双 Provider 各自的 Initial/Continuation 请求体捕获仍由现有双方 Adapter 测试负责，本项不重复扩成两套完整 UI E2E。

### 5.3 验收条件

1. 长 History 确实越过既有 Compactor 阈值，不允许直接伪造“已压缩”状态；
2. 同一 Turn 内恰好观察到三次 Provider 请求；
3. 请求顺序、系统指令类别与请求体类型均可由测试捕获并断言；
4. 最终 Memory、Conversation 和 Task Store 可重载且通过严格解析；
5. 没有产生第二个 Tool Round、重试或额外 Assistant 占位消息；
6. 测试不连接真实 DeepSeek/OpenAI 服务，不读取真实 Credential。

---

## 6. 实施顺序与停止条件

若本 Packet 后续获批并获得明确实施授权，唯一允许的连续顺序为：

1. `NR-G3-R01`：严格配对与持久化状态矩阵；
2. `NR-G3-R02`：Executor 注册与取消合规；
3. `NR-G3-R03`：Compactor + Tool 组合规模证据；
4. 更新 NR-G3 技术提交和机器 Manifest；
5. 重新提交 `NR-G3`，然后停止。

任一项失败时不得跳过、弱化断言或提前声明 Gate 通过。

---

## 7. 重新提交 NR-G3 所需证据

重新提交必须至少包含：

### 7.1 自动化测试

- R01 的正确配对正例与全部关键反例；
- R02 的三类合格 Executor 证据及不合格注册反例；
- R03 的同 Turn 三次 Provider 调用组合测试；
- 现有 NR-0、NR-1、NR-2 回归测试；
- 工作区既有格式、静态检查、单元测试、离线 E2E 和本地构建 Gate。

### 7.2 机器 Manifest

Manifest 应以机器可验证字段证明至少以下事实：

- `strictToolResultCallPairing = true`；
- `strictTaskStateMatrixValidated = true`；
- `unqualifiedExecutorRegistrationRejected = true`；
- `productionRegistryTools = 0`；
- `compactorToolCombinedProviderAttempts = 3`；
- `mainProviderAttemptsWithTool = 2`；
- `toolExecutions = 1`；
- `realProviderLiveSmokeExecuted = false`；
- `nrG3Approved = false`。

字段名称可按现有 Manifest 结构做机械调整，但语义不得缺失或只由自然语言报告代替。

### 7.3 技术报告

技术报告必须：

- 对 `NRG3-R01～R03` 逐项给出代码与测试证据；
- 清楚区分“技术重新提交”与“用户批准 Gate”；
- 不宣称生产 Tool 已注册；
- 不宣称执行过真实 Provider Live Smoke；
- 不宣称已进入 NR-3。

---

## 8. Draft 1 评审问题

本轮只需评审以下三个设计决策：

- `NRG3-D1-R01`：是否接受 Call ID、工具身份、终态与持久化状态矩阵的严格校验边界；
- `NRG3-D1-R02`：是否接受“受控注册 + 同实现合规测试 + 不合格执行器注册前拒绝”的最小取消合规方案；
- `NRG3-D1-R03`：是否接受仅通过测试注入缝完成一次 Provider 无关的 `Compactor + Tool` 组合规模测试。

Draft 1 获得明确批准前，不得实施 `NR-G3-R01～R03`。
