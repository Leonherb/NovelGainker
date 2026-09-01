# NR-G1 Remediation Story Packet Draft 2

> 状态：待最终评审；尚未授权实施  
> 日期：2026-09-01  
> 上游 Gate：NR-G1 技术提交  
> 范围：仅关闭 NR-G1 评审发现的 3 项问题；不进入 NR-1，不增加小说能力

## 1. 本版结论

Draft 1 的三项评审结论均已按建议接受：

1. `D1-R01`：以 Conversation Store 为最终事实源，Event 只负责快速通知；活动 Turn 期间由 UI 定期对账，避免永远停在“正在回复/正在停止”。
2. `D1-R02`：Clean Slate 采用一次性、完整的应用状态重置；旧应用数据不迁移、不兼容、不保留，小说 Markdown 与素材不受影响。
3. `D1-R03`：NR-G1 检查改为递归、闭集、失败即阻断的质量 Gate；它不是产品功能，不引入运行时耦合。

本版将上述决策固化成可实施、可验收的 Story Packet。任何代码实施仍须在 Draft 2 获得明确批准之后开始。

## 2. 冻结边界

### 2.1 保持不变

- NR-0 仍然只建立自然对话运行时，不实现 NR-1 的 Memory 增量维护、自动压缩或长上下文策略。
- Conversation 回复只输出自然语言，不要求模型输出 Control JSON。
- `Skill Calls = 0`、`Tool Calls = 0`、`Canon Reads/Writes = 0`。
- 本包不实现 Novelist Skill 调用、任务自主执行、角色/世界观/章节 Markdown 更新或冲突处理。
- 本包不执行真实 Provider Live Smoke；完成后只重新提交 NR-G1 技术证据。
- Windows Credential Manager 中的 Provider 凭据不属于项目状态，不随 Clean Slate 删除。

### 2.2 明确删除的复杂度

- 不做旧数据迁移器、格式转换器、兼容读取器、旧字段别名或功能开关。
- 不保留旧 Project ID、旧 Conversation 或旧 Host 运行状态。
- 不增加新的 Conversation IPC 命令。
- 不增加 Provider 自动重试，也不允许一次用户消息触发第二次 Provider 调用。
- 不为质量 Gate 增加运行时模块或产品 UI。

## 3. Story NR-G1-R01：终态对账与卡死恢复

### 3.1 问题

当前 UI 主要依赖运行时 Event 获知 Turn 已完成、失败或取消。若监听建立得太晚，或者终态 Event 丢失、延迟、重复，Conversation Store 已经完成落盘，UI 仍可能永久显示“正在回复”或“正在停止”。

### 3.2 决策

- Event 是快速路径，不是最终事实源。
- Conversation Store 是 Turn 状态及已保存消息的最终事实源。
- UI 必须先完成 Event listener 注册，再发送新的 Turn。
- 存在活动 Turn 时，UI 以不超过 2 秒的间隔调用现有 `conversation_session_load` 对账。
- 收到终态 Event 时立即重载 Store，不等待下一次定时对账。
- 对账覆盖 `completed`、`failed`、`cancelled`、`not_generated` 全部终态；不能只覆盖手动停止。
- 重复或乱序 Event 必须幂等，不得重复追加消息或把终态恢复为活动态。

### 3.3 失败边界

如果 listener 建立失败，或活动 Turn 的 Store 对账失败：

- UI 显示明确的“状态同步失败”和可重试入口；
- UI 不再伪装为“正在回复”或“正在停止”；
- 在无法确认上一 Turn 的终态前，不允许启动第二个 Turn；
- 不重新调用 Provider，不复制用户消息，不猜测终态。

### 3.4 验收标准

1. listener 一定在 Turn 发送之前建立。
2. Event 正常时，终态触发一次即时 Store 重载并正确渲染。
3. 人工丢弃终态 Event 时，UI 在下一次对账后恢复真实终态，最大对账间隔 `<= 2000 ms`。
4. 延迟或重复终态 Event 不会产生重复消息、重复完成或状态倒退。
5. `completed`、`failed`、`cancelled`、`not_generated` 均通过相同的对账规则。
6. listener 或对账读取失败时显示“状态同步失败”，重试成功后恢复 Store 真值。
7. 整个 Turn 的 `Provider Calls = 1`，不存在恢复性第二次 Provider 调用。
8. 保持 `Skill Calls = 0`、`Tool Calls = 0`、`Canon Reads/Writes = 0`。
9. Session reconciliation 是 Host 状态同步读取；证据不得把它误写成“其他操作 = 0”。

### 3.5 非目标

- Provider 内容重试或续写；
- 流式字符级持久化；
- Memory、Skill、Tool 或 Canon；
- 新 IPC 命令。

## 4. Story NR-G1-R02：一次性 Clean Slate 状态切换

### 4.1 问题

当前工作区可能残留旧 Agent/Workflow 时代的项目状态。既然现有用户数据已明确按“不重要”处理，保留兼容逻辑只会让新运行时继续背负旧语义和分支。

### 4.2 决策

首次以新 NR-0 版本打开一个尚未完成 Clean Slate 的项目时，Host 执行一次完整的应用状态重置：

- 在确认项目根目录、目标路径和 reparse point 安全后，重置项目内 `.novelgainker/**`；
- 旧 Project Metadata、旧 Conversation 和旧运行状态全部移除；
- 生成新的 Project ID；
- 创建空白 Conversation；
- 小说 Markdown Canon、素材、图片及其他作者内容原样保留；
- Windows Credential Manager 中的 Provider 凭据原样保留。

重置完成后写入当前最小 Metadata。Metadata 不增加 workflow version 字段，也不引入替代性的兼容包装层。是否已经完成切换，应由当前合法状态本身判断，而不是长期维护旧版本分支。

### 4.3 原子性与安全边界

- Clean Slate 只执行一次，不得每次打开项目都清空 Conversation。
- 第二次及后续打开必须读取现有新 Project ID 和 Conversation。
- 删除前必须验证绝对目标仍位于已确认的项目 `.novelgainker` 范围内，并拒绝不安全的 reparse point。
- 任一步骤失败时不得留下“旧状态删了一半、新状态建了一半”的混合状态。
- 失败时 Canon 不变，返回稳定且脱敏的错误，允许用户重试。

### 4.4 验收标准

1. 带旧 `.novelgainker` 状态的 fixture 首次打开后得到新 Project ID 与空白 Conversation。
2. 旧 Project ID、旧 Conversation、旧 Agent/Workflow 状态均不可再读取。
3. 小说 Markdown、素材与图片逐字节保持不变。
4. Provider Credential 不被读取、删除或重写。
5. 同一项目第二次打开不会再次重置，并保留第一次打开后产生的新 Conversation。
6. 路径越界、reparse point 不安全或重置写入失败时，操作失败且不触碰 Canon。
7. 没有 migration、converter、compat reader、alias 或 feature flag。
8. 当前 Metadata 不含 workflow version 字段或其他旧系统替代包装。

### 4.5 非目标

- 保留或导出旧聊天；
- 为现有用户提供升级向导；
- 清理 Credential Manager；
- 改写小说正文。

## 5. Story NR-G1-R03：闭集式 NR-G1 质量 Gate

### 5.1 问题

如果检查脚本只扫描手工列出的文件，新文件可以绕过检查；如果 allowlist 使用目录或通配符，旧 Agent/Workflow 语义也可能借“历史例外”重新进入生产代码。另一方面，Gate 内出现用于负向断言的旧词并不代表产品仍依赖旧系统，因此需要精确分类。

### 5.2 决策

Gate 必须递归发现并分类以下冻结根目录中的所有文件：

- `apps/desktop/src/**`
- `apps/desktop/src-tauri/src/**`
- `crates/*/src/**`
- `contracts/**`
- 当前生产使用的 `resources/**`
- `scripts/**`
- tests
- authoritative docs

未分类的新文件直接使 Gate 失败；新增文件必须自动进入扫描，不能靠人工补到文件清单。

### 5.3 分类规则

1. **生产源码、UI、contracts**：旧 Agent/Workflow 产品引用为零，不允许例外。
2. **Gate tests/scripts**：仅可为了精确负向断言保留旧 token；allowlist 必须记录精确文件、精确 token、用途和期望数量。
3. **历史/评审文档**：仅在确属历史说明时允许旧术语，并采用精确 allowlist。
4. **当前权威文档**：`CLAUDE.md`、architecture、development、sprint、BMAD 状态文档不得以“历史文档”为由豁免。

禁止目录级、glob 或开放式 allowlist。allowlist 数量漂移即失败。

### 5.4 IPC 闭包核验

- TypeScript IPC client 的命令集合必须从真实生产 client 文件中提取，不能在 Gate 中硬编码一份“期望实际值”。
- 提取结果必须与 Manifest、Handler、Permission、Capability 的 24 条命令严格相等。
- Conversation 命令严格为 3 条。
- 任一层多一条、少一条、拼写不一致或存在未分类 client 文件，Gate 失败。

### 5.5 派生证据

以下证据必须由测试或扫描结果计算产生，不能把期望值直接写成通过结果：

- `providerCallsPerTurn`
- `skillCallsPerTurn`
- `toolCallsPerTurn`
- `canonOperationsPerTurn`
- `sessionReconciliationMaximumIntervalMs`
- `releaseConversationPathCount`
- `retiredProductReferences`
- `conversationStartsBlank`
- `automatedExternalRequests`

### 5.6 验收标准

1. 在任一受管根目录新增包含旧产品引用的文件，Gate 必须失败。
2. 新增未分类文件，Gate 必须失败。
3. 精确 allowlist 的 token 数量发生漂移，Gate 必须失败。
4. 生产源码、UI 和 contracts 不存在旧 Agent/Workflow 产品引用。
5. TypeScript client、Manifest、Handler、Permission、Capability 的 IPC 命令集合严格闭合为 24 条，Conversation 子集严格为 3 条。
6. 所有指标来自实际扫描或测试结果，修改硬编码期望值不能伪造通过。
7. Gate 只作为构建/评审质量检查运行，不进入应用运行时。

### 5.7 非目标

- 新产品功能；
- 运行时遥测系统；
- 通用仓库治理平台；
- 扫描第三方依赖、构建产物或明确不属于源码与权威文档的缓存目录。

## 6. 连续实施顺序（待授权）

Draft 2 获得明确批准后，实施顺序固定为：

1. `NR-G1-R01`：终态 Event + Store 对账；
2. `NR-G1-R02`：一次性 Clean Slate 状态切换；
3. `NR-G1-R03`：闭集式质量 Gate；
4. 执行既有 `pnpm quality` 与本包新增的定向回归；
5. 重新形成 NR-G1 技术提交并停止，等待产品评审。

不得跳过 R01 先扩大功能范围，不得进入 NR-1，不得执行真实 Provider Live Smoke。

## 7. 测试与证据矩阵

| Story | 最小自动化证据 | 必须证明 |
| --- | --- | --- |
| R01 | UI/runtime 集成测试、丢 Event/重复 Event/对账失败 fixture | UI 最终服从 Store；不重复调用 Provider；不永久活动 |
| R02 | 旧状态 fixture、二次打开、失败注入、路径/reparse 安全测试 | 一次性完整重置；Canon/Credential 不受影响；无半完成状态 |
| R03 | 递归扫描、合成违规文件、未分类文件、allowlist 漂移、IPC 集合比对 | 新文件无法绕过；例外精确；证据为实际派生 |

共同证据必须继续满足：

- `Provider Calls per Turn = 1`
- `Skill Calls per Turn = 0`
- `Tool Calls per Turn = 0`
- `Canon Operations per Turn = 0`
- `Automated External Requests = 0`
- `Conversation Starts Blank = true`

其中 R01 增加：

- `sessionReconciliationMaximumIntervalMs <= 2000`

## 8. Draft 1 评审关闭记录

| 决策项 | 结论 | Draft 2 落点 |
| --- | --- | --- |
| D1-R01 | 按建议接受 | Event 快速路径、Store 最终事实、`<= 2s` 对账、同步失败显式降级 |
| D1-R02 | 按建议接受 | 一次性完整应用状态重置；保留 Canon 与 Credential；不做迁移兼容 |
| D1-R03 | 按建议接受 | 递归闭集扫描、精确 allowlist、IPC 实际提取、证据派生与合成失败测试 |

Draft 1 的三个阻断在“设计决策”层面已经关闭；只有 Draft 2 获批并完成实现与回归后，NR-G1 技术 Gate 才能重新提交。

## 9. Review Gate

- Gate ID：`NR-G1-REMEDIATION-DRAFT-2-REVIEW`
- 当前状态：`pending_product_review`
- `nrG1Approved = false`
- 实施授权：`false`
- NR-1 授权：`false`
- Live Smoke 授权：`false`

本轮只需确认：

1. R01 的 Event/Store 双路径是否接受；
2. R02 的一次性完整状态重置边界是否接受；
3. R03 的闭集式质量 Gate 是否接受；
4. 是否授权按 `R01 -> R02 -> R03` 连续实施，并在重新提交 NR-G1 后停止。
