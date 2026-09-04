# NR-G3 Remediation Story Packet Draft 2

## 0. 文档状态

- Packet：`NR-G3 Remediation`
- 版本：`Draft 2`
- 状态：已批准并实施
- 上游 Gate：`NR-G3 技术提交`
- 上游评审结论：`NRG3-R01～R03` 阻断，当前不得批准 `NR-G3`
- Draft 1 评审结论：三个方向均接受；补充 R01、收窄 R02 的保证范围、补充 R03 的同路径与 Memory 消费证据
- 实施授权：**已授权并按 NR-G3-R01 → NR-G3-R02 → NR-G3-R03 完成**
- 真实 Provider Live Smoke：**未授权**
- `NR-3`：**不得进入、不得形成 Story Packet**

本文档只定义关闭 `NRG3-R01～R03` 所需的最小补救范围。产品负责人已于 2026-09-03 明确批准三个设计决策并授权连续实施；实现与机器证据现已完成并重新停止于 NR-G3 技术提交。该实施不构成 NR-G3 产品批准。

---

## 1. 依据与目标

### 1.1 依据

本 Packet 以以下已批准或已评审材料为唯一上游依据：

1. `NR-2 Host Agent Loop / Tool Request-Result Story Packet Draft 2`；
2. 当前 `NR-G3 Technical Submission`；
3. `NR-G3` Gate Review 对 `NRG3-R01～R03` 的评审结论；
4. `NR-G3 Remediation Story Packet Draft 1` 及其 `NRG3-D1-R01～R03` 评审结论。

若本文与已批准的 NR-2 合同冲突，以已批准的 NR-2 Story Packet 为准；本文不得借补救之名扩大产品能力。

### 1.2 补救目标

仅完成以下三项目标：

1. 在 Host 的 Task Store 终态写入与恢复读取边界闭合 Tool Result 严格配对合同；
2. 在封闭 Tool Registry 中把可执行资格绑定到已经证明取消合规的具体 Executor 实现；
3. 增加同一 Turn 内真实经过 `Compactor → Initial → Tool → Continuation`、且新 Memory 被 Initial 实际消费的组合规模证据。

### 1.3 明确不做

本 Packet 不包含：

- 注册任何生产 Tool；
- 实现 Novelist Skill、Canon/Chapter 写入或其他 NR-3 能力；
- 修改三栏布局、Conversation UI 或 Task Activities UI；
- 增加工具轮次、重试次数或并行执行；
- 修改双 Provider 产品范围或 Credential 行为；
- 为任意进程内代码提供强制终止能力；
- 引入子进程、沙箱服务、后台守护进程或新的执行隔离架构；
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
- 标称 SHA-256 的字段长度正确但不是规范十六进制摘要；
- 非法持久化记录被恢复逻辑误解释为一次普通中断。

仅依赖上游调用者“目前会传正确值”不足以构成持久化合同。

### 3.2 设计边界

#### A. Call ID 按不透明原始值配对

- `callId` 是不透明字符串，不做大小写转换、裁剪或 Unicode 归一化；
- Task 创建时继续只保存原始 `callId` 的 SHA-256，不落原始 Provider Payload；
- 写入 Tool Result 前，Host 必须以同一算法计算 `result.callId` 的 SHA-256，并与 Task 中的 `call_id_hash` 精确比较；
- 不一致时拒绝事务，不增加 Revision，不修改内存对象，不部分写入磁盘。

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
- 接受已经通过严格 Schema、项目绑定、Hash 和状态矩阵校验的 Store；
- 将合法的 `queued/running` 转为 `cancelled`；
- 写入既有、稳定、受控的恢复错误码；
- 不伪造 Tool Result、Result Hash 或 Result Bytes。

普通实时终态转换不得借用这一例外。

如果 Store 在恢复前已经违反 Schema、项目绑定、Hash 或状态矩阵，则必须返回受控 Store 错误，保持文件原样，不得自动改写、删除或包装成 `cancelled`。

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

#### E. Error Code 使用稳定白名单

- Task Store 不接受任意非空字符串作为错误码；
- 实时失败、实时取消和恢复取消分别只能使用 Host 已定义的稳定错误码集合；
- 未知错误码在加载和写回时都必须被拒绝；
- 错误码校验不读取或信任 Provider 自由文本。

### 3.3 验收条件

1. 正确 Call ID、工具名、版本和状态可以完成一次 CAS 终态转换；
2. 错误 Call ID 被拒绝，Revision、内存对象和磁盘内容不变；
3. 错误工具名或版本被拒绝；
4. Tool Result 状态与 Task 目标终态不一致时被拒绝；
5. 非十六进制 Hash、半组 Result 字段、未知错误码及每一种非法状态组合在加载时被拒绝；
6. 合法的恢复取消仍可处理没有 Tool Result 的 `queued/running` Task；
7. 非法 Store 不得被恢复逻辑改写为 `cancelled`；
8. Store 继续不保存原始 Provider Tool Payload；
9. 新增正例与反例测试，且现有 Task Revision/CAS、项目绑定和原子替换测试保持通过。

---

## 4. NRG3-R02：闭合封闭 Registry 的 Executor 取消合规

### 4.1 问题与保证范围

当前 Host 会发出取消信号并等待确认，但在线程作用域内运行的任意代码如果不响应取消，Host 本身不能承诺将其强制杀死。仅验证 Tool Definition 也无法证明 Executor 满足“取消后两秒内退出”。

因此本项不宣称 Host 可以强制终止任意进程内 Executor。NR-G3 能够证明的范围收窄为：

> 只有封闭 Registry 中明确批准、并由同一实现版本的确定性测试证明满足取消合同的具体 Executor，才具有可执行资格。

NR-2 当前没有生产 Tool；本项只闭合 Registry 的资格边界，不引入生产 Executor 或新的执行隔离架构。

### 4.2 设计边界

#### A. Registry 是封闭集合

- 不提供可由 Tauri Command、前端、Provider 响应、Skill 内容或普通运行时代码调用的通用动态注册 API；
- 原始 Executor 不得仅凭 Tool Definition 进入可执行映射；
- 可执行条目只能由模块内部受控构造路径创建；
- 资格绑定到具体 Executor 实现、实现版本和对应 Tool Definition，而不是只绑定工具名；
- 当前 Production Registry 继续由 `empty_production()` 构造，工具数严格为 `0`。

#### B. 合规证据与具体实现绑定

每一种允许进入测试 Registry 的合成 Executor，必须由同版本、同实现路径的确定性合同测试证明：

- 正常完成可以退出；
- 主动失败可以退出；
- 收到取消后在两秒内确认并退出；
- 退出后不会继续产生 Task 写入、Conversation 提交或其他可观察副作用。

受控构造路径只接受已经列入封闭合规集合的具体实现。修改该实现的取消行为、版本或注册定义时，原合规证据失效，必须重新运行并提交合同测试。

#### C. 不合格 Executor 反例

增加一个故意忽略取消或超过两秒才退出的合成 Executor。它没有封闭合规资格，并且必须在构造可执行 Registry 时被拒绝：

- 不得进入 Provider 可见工具定义；
- 不得进入可解析或可执行映射；
- 不得被 Agent Loop 调用；
- 不产生 Task、Conversation 或工程文件副作用。

反例应在执行前由注册边界拦截，不通过实际启动一个无法按时退出的线程来制造测试挂起。

#### D. 未来生产 Executor 不获得预授权

- 本 Packet 不批准任何未来生产 Executor；
- 后续加入生产 Executor 会改变 `productionRegistryTools = 0`，必须由对应 Story Packet 明确列出具体实现、权限、取消点和合规测试，并重新经过 Gate；
- 不得将本次合成 Executor 的测试资格解释为生产注册资格；
- 在引入更强隔离架构前，产品只对封闭集合中的已验证实现作出两秒取消承诺。

### 4.3 验收条件

1. 三种合格合成 Executor 的完成、失败和取消合同测试均通过；
2. 取消合规测试使用单调时钟，确认取消信号后两秒内完成；
3. 合规资格明确绑定到具体实现和版本；
4. 不合格 Executor 无法进入 Provider 可见定义、解析表或执行表；
5. 不合格 Executor 没有被执行，也没有产生持久化副作用；
6. Registry 仍拒绝重复工具名/版本和 Provider 可见名称冲突；
7. Production Registry 工具数仍为 `0`；
8. 不新增通用动态注册 API、子进程、沙箱服务、后台守护进程或生产 Tool；
9. 技术报告不得宣称可以强制终止任意进程内代码。

---

## 5. NRG3-R03：补齐 Compactor + Tool 单 Turn 组合规模证据

### 5.1 问题

现有测试分别证明了 Memory/Compactor 与 Tool Loop，但没有用同一个 Turn 证明：

`长 History → Compactor → Initial → Tool → Continuation → Conversation/Task Commit`

也没有证明 Compactor 生成并提交的新 Memory 确实被后续 Initial 请求读取。因此“同一 Turn 累计最多三次 Provider 调用”与上下文衔接目前缺少组合运行证据。

### 5.2 设计边界

#### A. 生产与测试共用同一核心编排路径

- 提取或复用唯一的核心 Turn 编排函数；
- 生产包装入口调用该核心函数，并固定传入空 Production Registry；
- 测试调用同一个核心函数，只替换为通过 R02 合规边界的测试 Registry；
- 不允许复制、重写或维护第二套仅供测试的 Agent Loop；
- 测试注入缝不得暴露为 Tauri Command、前端 API、Provider 控制字段或生产公共注册接口。

#### B. 单 Turn 三阶段脚本

Loopback Provider 在同一 Turn 中按确定顺序返回：

1. Compactor 请求返回有效五栏目 Memory；
2. Host 严格校验并提交新的 Memory Revision；
3. Initial 请求读取该新 Memory，并返回一个合法 Tool Call；
4. Tool 成功后，Continuation 返回最终自然语言回复。

使用一个已通过 R02 合规注册的合成 Executor，且只允许执行一次。

#### C. 新 Memory 必须被实际消费

测试必须捕获 Initial 请求体并证明：

- 请求使用的是本 Turn 刚提交的新 Memory Revision，而不是旧 Memory；
- 新 Memory 的五栏目语义进入既有动态系统指令位置；
- 已被压缩替代的早期 History 不再以原始消息重复发送；
- Compactor 的原始回复不作为 Assistant 消息或 Tool Result 混入 Conversation；
- Continuation 保持同一有效 Memory 上下文，并只增加受控 Tool Result。

不得仅根据调用顺序推断“新 Memory 已被使用”。

#### D. 组合断言

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
2. 生产和测试入口经过同一个核心 Turn 编排函数；
3. 同一 Turn 内恰好观察到三次 Provider 请求；
4. 请求顺序、系统指令类别与请求体类型均可由测试捕获并断言；
5. Initial 请求体明确包含新 Memory，并排除已被压缩替代的原始早期 History；
6. 最终 Memory、Conversation 和 Task Store 可重载且通过严格解析；
7. 没有产生第二个 Tool Round、重试或额外 Assistant 占位消息；
8. 测试不连接真实 DeepSeek/OpenAI 服务，不读取真实 Credential。

---

## 6. 实施顺序与停止条件

若本 Packet 后续获批并获得明确实施授权，唯一允许的连续顺序为：

1. `NR-G3-R01`：严格配对、状态矩阵与错误码白名单；
2. `NR-G3-R02`：封闭 Registry 与具体 Executor 取消合规；
3. `NR-G3-R03`：同编排路径的 Compactor + Tool 组合证据；
4. 更新 NR-G3 技术提交和机器 Manifest；
5. 重新提交 `NR-G3`，然后停止。

任一项失败时不得跳过、弱化断言或提前声明 Gate 通过。

---

## 7. 重新提交 NR-G3 所需证据

### 7.1 自动化测试

- R01 的正确配对正例，以及错误 Call ID、错误工具身份、错误终态、非法 Hash、非法状态矩阵、未知错误码和非法恢复反例；
- R02 的三类合格 Executor 证据及不合格注册反例；
- R03 的同 Turn 三次 Provider 调用、新 Memory 实际消费和同核心编排路径测试；
- 现有 NR-0、NR-1、NR-2 回归测试；
- 工作区既有格式、静态检查、单元测试、离线 E2E 和本地构建 Gate。

### 7.2 机器 Manifest

Manifest 应以机器可验证字段证明至少以下事实：

- `strictToolResultCallPairing = true`；
- `strictTaskStateMatrixValidated = true`；
- `taskErrorCodeAllowlistValidated = true`；
- `invalidTaskStoreRecoveryRejected = true`；
- `unqualifiedExecutorRegistrationRejected = true`；
- `productionRegistryTools = 0`；
- `sharedProductionTestTurnOrchestrator = true`；
- `compactorMemoryConsumedByInitial = true`；
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
- 将两秒取消保证限定为封闭 Registry 中已验证的具体 Executor；
- 不宣称生产 Tool 已注册；
- 不宣称可以强制终止任意进程内代码；
- 不宣称执行过真实 Provider Live Smoke；
- 不宣称已进入 NR-3。

---

## 8. Draft 2 批准与实施记录

以下三个设计决策已经产品负责人明确批准并完成实施：

- `NRG3-D2-R01`：接受严格配对、状态矩阵、错误码白名单，以及非法 Store 不得被恢复逻辑自动改写；
- `NRG3-D2-R02`：接受把两秒取消保证限定为封闭 Registry 中经验证的具体 Executor，并明确不承诺强制终止任意进程内代码；
- `NRG3-D2-R03`：接受生产与测试共用核心 Turn 编排函数，并以请求体证据证明新 Memory 被 Initial 实际消费。

`NR-G3-R01～R03` 已按批准顺序完成；当前只允许重新评审 NR-G3 技术提交。未经新的明确授权，不执行真实 Provider Live Smoke，不批准 NR-G3，不进入或形成 NR-3。
