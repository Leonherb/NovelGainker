# NR-G1 Remediation Story Packet Draft 2 Final

> 状态：已批准并实施；已重新提交 NR-G1 技术证据  
> 日期：2026-09-01  
> BMAD Packet：`review-gate-packet`  
> 上游 Gate：`NR-G1-technical-submission`  
> 范围：只关闭 R01～R03；不进入 NR-1，不执行真实 Provider Live Smoke

## 1. Final 对齐结论

本 Final 在 Draft 2 基础上只完成三项收敛：

1. **R01 保持不变**：Event 是快速通知，Conversation Store 是最终事实源；活动 Turn 必须自动对账。
2. **R02 明确一次性切换**：复用现有通用 `schemaVersion`，执行 `1 -> 2` Clean Slate；它不是 Workflow 版本，也不是旧数据迁移。
3. **R03 收缩 Gate**：只检查发布面、IPC 安全面、机器证据和当前 NR-G1 Report；不扫描全部测试或历史文档，不建设通用仓库治理系统。

R01～R03 完成并通过回归后，只重新提交 NR-G1 技术证据；`nrG1Approved` 仍为 `false`。

## 2. 冻结边界

### 2.1 NR-0 保持不变

- Conversation 只输出自然语言，不要求 Provider 输出 Control JSON。
- 每个用户 Turn 最多调用 Provider 一次。
- `Skill Calls = 0`、`Tool Calls = 0`、`Canon Reads/Writes = 0`。
- 不实现 Memory、自动压缩、长上下文预算、Novelist Skill、Tool Loop 或 AI Markdown 写入。
- 不新增 Conversation IPC Command。
- 不执行真实 Provider Live Smoke。

### 2.2 数据原则

- 小说 Markdown Canon、素材与图片必须保持不变。
- 当前旧应用状态按“不重要”处理，不迁移、不转换、不保留。
- 完成 Clean Slate 后产生的新 Project ID 与 Conversation 必须保留，不能在后续打开时再次清理。
- Provider Settings、无关项目状态、UI Preferences 和 Windows Credential Manager 凭据不属于本次重置范围。
- 不增加兼容 Reader、Converter、Alias、Feature Flag 或隐藏 UI。

## 3. Story NR-G1-R01：Event 快路径与 Store 终态收敛

### 3.1 问题

当前 UI 可以在 Conversation Event listener 尚未就绪时发送 Turn。终态 Event 丢失、延迟或重复时，Store 已经持久化终态，UI 仍可能永久显示“正在回复”或“正在停止”。

### 3.2 实施边界

- UI 必须先完成 listener 注册，再允许发送 Turn。
- Event 继续用于首个 Delta、流式文本和低延迟终态通知。
- Conversation Store 是 Turn 状态和可见消息的最终事实源。
- 收到终态 Event 后立即调用现有 `conversation_session_load`，以 Store 覆盖 UI。
- 存在活动 Turn 时，UI 使用单一、非重叠的 Session reconciliation；最大间隔不超过 2 秒。
- 对账覆盖 `completed`、`failed`、`cancelled`、`not_generated`。
- Event 丢失、延迟、重复、乱序或 listener 重建不得产生重复消息、状态倒退或第二次 Provider 调用。

### 3.3 同步失败

listener 建立失败或活动 Turn 对账失败时：

- UI 显示“状态同步失败”和重试入口；
- 不再显示虚假的“正在回复”或“正在停止”；
- 上一 Turn 终态未知时不得发送第二个 Turn；
- 重试只重新建立 listener 或读取 Store，不调用 Provider，不复制用户消息。

### 3.4 验收标准

1. 人为延迟 listener Promise 时，发送入口保持不可用。
2. 丢弃终态 Event 后，UI 在最多 2 秒内从 Store 恢复四种真实终态。
3. 终态 Event 正常到达时立即重载 Store。
4. 重复、延迟或乱序 Event 不产生重复消息或状态倒退。
5. reconciliation 不并发堆积；组件卸载、项目切换或终态到达后停止。
6. 同步失败显示稳定错误，重试成功后恢复 Store 真值。
7. `Provider Calls per Turn = 1`。
8. `Skill Calls = 0`、`Tool Calls = 0`、`Canon Operations = 0`。

### 3.5 非目标

- Provider 自动重试、续写或 Cursor；
- 流式字符级持久化；
- 新 IPC；
- Memory、Skill、Tool 或 Canon。

## 4. Story NR-G1-R02：schemaVersion 1 → 2 一次性 Clean Slate

### 4.1 问题

当前 Project Metadata 使用 `schemaVersion = 1`，并包含已退役的 `workflowPackVersion`。直接从严格 Metadata 合同删除该字段后，旧工程会在 Host 执行重置前就因未知字段而解析失败；另一方面，若每次打开都重置，又会删除切换后产生的新 Conversation。

### 4.2 判定规则

`schemaVersion` 是现有 Project Metadata 的通用数据格式版本，不是 Workflow 版本。

项目打开前，Host 只读取有大小上限的最小信封：

~~~json
{
  "schemaVersion": 1,
  "projectId": "optional-validated-uuid"
}
~~~

判定固定为：

- `schemaVersion = 1`：视为本次可丢弃的旧应用状态；获得 writer lease 并完成安全检查后执行一次 Clean Slate。
- `schemaVersion = 2`：按当前严格 Metadata 合同正常打开，不再清理。
- Metadata 不存在：沿用当前“打开 Markdown 工程并确认创建 Metadata”的路径。
- schema 缺失、损坏或为未知版本：返回稳定错误，不删除任何内容。

Host 不读取、解释或转换旧 Workflow 字段，也不保留旧 Conversation；最小信封只用于安全选择“重置”或“正常打开”。

### 4.3 重置范围

对确认属于 `schemaVersion = 1` 的目标项目：

- 安全重置该项目精确的 `.novelgainker/**`；
- 删除旧 Project Metadata、Conversation、索引、恢复状态和其他项目本地 App State；
- 生成新的 Project ID；
- 写入 `schemaVersion = 2` 的最小 Metadata，不包含 `workflowPackVersion` 或替代字段；
- 创建空白 Conversation；
- 从全局 App State 中删除与旧 Project ID 或该规范化项目根精确关联的 `recent_projects`、`project_files` 记录；
- 随后只记录新的 Project ID。

不得删除或重置：

- 小说 Markdown Canon、素材、图片和其他作者文件；
- Provider Settings；
- UI Preferences；
- 无关项目记录；
- Windows Credential Manager 凭据。

### 4.4 安全与事务边界

- 只在已规范化的目标项目根和已获得的 writer lease 下执行。
- 必须验证 `.novelgainker` 绝对目标仍位于该项目根内，并拒绝 symlink/reparse point。
- Project-local v2 状态必须完整准备后再提交；失败不得留下可被当作合法 v2 打开的 v1/v2 混合状态。
- 全局 App State 的精确关联清理使用单一数据库事务。
- 任一步骤失败时 Canon 不变，返回稳定且脱敏的错误，允许重试。
- 成功后再次打开同一项目不得重新执行 Clean Slate。

本 Story 在这一次 `v1 -> v2` 切换上明确取代当前“打开后只惰性删除 `.novelgainker/agent`”的旧清理规则。实现完成后不保留两条并行旧状态清理路径。

### 4.5 验收标准

1. v1 fixture 首次打开后获得新 Project ID、v2 Metadata 和空白 Conversation。
2. 同一项目第二次打开保留新 Project ID 和新 Conversation，不再次重置。
3. v2 fixture 直接打开，不触发清理。
4. Metadata 缺失沿用当前确认路径。
5. schema 损坏或未知时失败且不删除项目状态或 Canon。
6. 旧 Project ID 不再出现在 `recent_projects` 或 `project_files`；无关项目记录不变。
7. Provider Settings、UI Preferences 和 Credential 不变。
8. Canon、素材与图片逐字节不变。
9. reparse point、路径越界、无 writer lease 或失败注入均不能产生合法混合状态。
10. 当前 Rust、Tauri、TypeScript、UI 与测试不存在 `workflowPackVersion` 及同义替代字段。
11. 不新增 migration、converter、compat reader、alias 或 feature flag。

### 4.6 非目标

- 导出或保留旧聊天；
- 为旧状态提供升级 UI；
- 重置全部全局 App State；
- 清理 Credential Manager；
- 修改小说正文。

## 5. Story NR-G1-R03：最小真实 NR-G1 Gate

### 5.1 目标

Gate 只证明当前 NR-0 技术提交声称的事实。它不是产品能力，也不是全仓库治理平台。

检查范围固定为四部分：

1. 发布面；
2. IPC 安全面；
3. 机器证据；
4. 当前 NR-G1 Manifest 与 Report。

### 5.2 发布面检查

递归扫描会进入桌面应用编译、打包或运行时加载的生产文件：

- `apps/desktop/src/**` 中的非 test/spec/fixture TypeScript 与 TSX；
- `apps/desktop/src-tauri/src/**`；
- 实际参与桌面应用编译的 `crates/*/src/**`；
- `contracts/**` 中当前生产 IPC 合同；
- Tauri 配置实际声明会打包或加载的 production resources；若没有，则该集合为空。

发布面中的旧 Agent/Workflow 产品入口、旧 IPC、旧 Control JSON、旧 Skill 选择或旧 Proposal 路径必须为零。生产发布面不允许 allowlist。

Gate 不扫描以下内容中的旧词：

- 普通单元测试、E2E 源码和测试 fixture；
- 历史 Story、历史评审记录和归档文档；
- 第三方依赖、缓存和构建产物。

Gate 必须有一个自身回归：在临时复制的受管生产根中新增包含退役引用的生产文件，扫描必须失败。该临时 fixture 不写入真实源码树。

### 5.3 IPC 闭包

Gate 从真实文件提取命令集合，并比较：

- Command Manifest；
- Tauri Handler 注册；
- generated permission；
- main-window Capability；
- 所有生产 TypeScript IPC Client。

五个集合必须严格相等为 24 条 Command；Conversation 子集严格为：

1. `conversation_session_load`
2. `conversation_turn_start`
3. `conversation_turn_cancel`

`conversation-turn-event` 是 Event，不计入 Command。

### 5.4 机器证据

以下结论必须从已经通过的自动化测试 Evidence、实际集合比较或实际扫描结果派生：

- `providerCallsPerTurn`
- `skillCallsPerTurn`
- `toolCallsPerTurn`
- `canonOperationsPerTurn`
- `sessionReconciliationMaximumIntervalMs`
- `releaseConversationPathCount`
- `retiredProductReferences`
- `conversationStartsBlank`
- `automatedExternalRequests`

其中 `retiredProductReferences = 0` 的精确定义是：**生产发布面中的退役产品引用为零**。测试和历史文档不计入该指标。

如果上游 Evidence 尚未提供某字段，应增强对应的离线合成测试并输出字段；不得在 NR-G1 Gate 中把期望值直接写成证据。

### 5.5 Report 一致性

- 新 NR-G1 Manifest 只能写入实际派生的结果和范围 Hash。
- 当前 NR-G1 Report 只能陈述 Manifest 已证明的事实。
- BMAD workflow、sprint 和 Report 必须保持同一 Gate 状态。
- Gate 不修改产品运行时，不新增 UI、IPC、遥测或外部请求。

### 5.6 验收标准

1. 新增受管生产文件会自动进入扫描。
2. 临时生产文件包含退役引用时 Gate 必须失败。
3. 测试和历史文档中的说明性旧词不会被误判为生产引用。
4. Manifest、Handler、Permission、Capability 与所有生产 TypeScript Client 的实际集合严格等于 24 条。
5. Conversation Command 实际集合严格等于 3 条。
6. Evidence 字段全部来自测试、扫描或集合比较；删除上游 Evidence 或篡改集合会使 Gate 失败。
7. NR-G1 Report 与 Manifest 不一致时 Gate 失败。
8. 完整 `pnpm quality`、Debug/Release 构建和 Windows 离线 Smoke 通过。
9. 真实 Provider Live Smoke 未执行，自动化外部请求为零。

### 5.7 非目标

- 扫描整个仓库；
- 扫描历史文档中的旧名词；
- 扫描所有测试源码中的旧名词；
- 维护通用文件分类器或复杂 allowlist；
- 建立运行时遥测系统；
- 增加任何产品功能。

## 6. 已执行的实施顺序

本 Final 已获明确批准，并已按以下顺序连续实施：

1. `NR-G1-R01`：Event + Store 终态收敛；
2. `NR-G1-R02`：`schemaVersion 1 -> 2` 一次性 Clean Slate；
3. `NR-G1-R03`：最小真实 NR-G1 Gate；
4. 执行定向回归与完整 `pnpm quality`；
5. 重新生成 NR-G1 Manifest 和技术提交；
6. 在 NR-G1 技术提交停止，等待产品评审。

不得进入 NR-1，不得执行真实 Provider Live Smoke。

## 7. 测试与证据矩阵

| Story | 最小证据 | 必须证明 |
| --- | --- | --- |
| R01 | listener 延迟、Event 丢失/重复/乱序、Store 对账失败 | UI 最终服从 Store；最大间隔不超过 2 秒；不重复调用 Provider |
| R02 | v1/v2/缺失/损坏/未知 schema、二次打开、全局精确记录、路径/reparse/失败注入 | 只切换一次；新数据保留；Canon/Credential/无关状态不变 |
| R03 | 发布面递归扫描、临时违规生产文件、五层 IPC 集合、上游 Evidence 缺失、Report 漂移 | 新生产文件无法绕过；证据非硬编码；范围保持最小 |

共同证据：

- `Provider Calls per Turn = 1`
- `Skill Calls per Turn = 0`
- `Tool Calls per Turn = 0`
- `Canon Operations per Turn = 0`
- `Automated External Requests = 0`
- `Conversation Starts Blank = true`
- `sessionReconciliationMaximumIntervalMs <= 2000`

## 8. Final Review Closure

| 评审项 | Final 结论 |
| --- | --- |
| R01 Event/Store | 通过，无方向变更 |
| R02 切换判定 | 复用通用 `schemaVersion`，固定 `v1 -> v2` 一次性重置 |
| R02 数据范围 | 重置精确项目本地状态并清除旧 Project ID 的全局精确关联；保留其他全局状态与凭据 |
| R03 Gate 范围 | 只检查发布面、IPC、安全证据和当前 Report |
| Tests/历史 Docs | 不做退役词扫描，不维护复杂 allowlist |
| BMAD | Final 待产品明确批准；未批准前不实施 |

## 9. Final Approval Closure

- Gate ID：`NR-G1-REMEDIATION-D2-FINAL-REVIEW`
- 当前状态：`implemented`
- `nrG1Approved = false`
- 实施授权：`true`
- NR-1 授权：`false`
- Live Smoke 授权：`false`

已接受的批准语句：

> 批准 NR-G1 Remediation Story Packet Draft 2 Final，授权按 NR-G1-R01 → NR-G1-R02 → NR-G1-R03 连续实施，并在重新提交 NR-G1 技术提交后停止；不进入 NR-1，不执行真实 Provider Live Smoke。
