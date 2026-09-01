# NovelGainker NR-G1 Remediation Story Packet Draft 1

> 状态：Draft 1，等待产品评审；未授权实施
>
> 日期：2026-08-31
>
> BMAD 数据包：`review-gate-packet`
>
> 范围：仅 NRG1-R01、NRG1-R02、NRG1-R03，以及修复后的 NR-G1 重新技术提交

## 1. BMAD Routing Brief

- Packet type：`review-gate-packet`
- Project type：Windows-first local-first Tauri desktop app
- Project level：4
- Runtime context：Codex / Rust Host / React UI
- Current phase：Implementation Remediation / NR-G1 Gate
- Main constraint：NR-G1 review blockers
- Recommended next move：评审本 Draft 1；批准后才按 NRG1-R01 → NRG1-R02 → NRG1-R03 连续修复，并重新提交 NR-G1

当前 NR-001～NR-006 技术实现仍停在 `NR-G1-technical-submission`，`nrG1Approved = false`。本 Packet 不重开 NR-0 产品设计，也不进入 NR-1；它只关闭 2026-08-31 NR-G1 评审确认的三项阻断。

## 2. 评审结论与修复目标

NR-G1 当前不通过，原因固定为：

1. `NRG1-R01`：前端终态依赖单次事件通知，监听延迟或终态事件丢失时可能继续显示“正在回复/正在停止”。
2. `NRG1-R02`：已删除的 Workflow 产品仍通过 `workflow_pack_version` 残留在 Project Metadata、创建预览、TypeScript Contract 和 UI。
3. `NRG1-R03`：NR-G1 门禁扫描范围不完整，若干 Manifest 结论由常量写入，且报告声称的 TypeScript Client 全量对齐没有由边界门强制证明。

本轮修复后的唯一目标是：让 NR-0 当前承诺可被运行时和机器证据真实证明，再重新提交 NR-G1。

## 3. 冻结原则

### 3.1 Conversation 终态

- `.novelgainker/conversation/session.json` 是 Turn 终态的最终事实来源。
- `conversation-turn-event` 是低延迟通知，不是唯一终态来源。
- 丢失、延迟或重复 Event 不得使 UI 永久停在活动状态。
- 修复不得新增第二次 Provider 调用、自动重试、断点续传、Provider Cursor 或 Memory。
- 已有“一次用户发送最多一次 Provider 调用”和“非空文本必保存”规则不变。

### 3.2 Clean Slate

- `workflow_pack_version`、`WORKFLOW_PACK_VERSION`、`workflowPackVersion` 及其用户可见字段全部退出当前产品契约。
- 不用新名字包装同一个旧 Workflow 概念；当前产品不需要 Workflow/模板包版本。
- 旧应用元数据视为可重建状态，不建立兼容 Reader、Converter、Migration、Alias 或 Feature Flag。
- 小说 Markdown Canon 与资产不得修改；清理前后 Canon Tree Hash 必须一致。

### 3.3 Gate 证据

- Gate 结论必须由扫描、比较或测试结果计算，不得把待证明结果直接写成常量。
- 负向回归测试可以保留退役名词作为检测字面量，但必须显式列入 Gate-only allowlist；它们不得被统计为生产引用。
- NR-G1 Report 只能陈述机器证据已经实际覆盖的事实。

## 4. 实施顺序

```text
NRG1-R01 终态收敛
    ↓
NRG1-R02 Workflow 残留清理
    ↓
NRG1-R03 门禁与证据修正
    ↓
重新运行 pnpm quality
    ↓
重新提交 NR-G1（nrG1Approved 仍为 false）
```

不得并入 NR-1 工作。每项完成后运行与该项直接相关的回归测试；R03 完成后运行完整 `pnpm quality`。

## 5. Remediation Stories

### NRG1-R01——Event 快路径与 Store 终态收敛

#### 问题

`ConversationView` 在 Event Listener 尚未就绪时可能允许发送；发送和停止后的活动状态主要依赖终态 Event 清除。当终态已经写入 Store、但 Event 未被接收时，当前窗口没有必然的再次对账路径。

#### 实施边界

- 显式维护 Conversation Event Listener 是否就绪；监听完成前不得启动新 Turn。
- Listener 就绪或重新建立后，立即读取当前 Session，收敛可能已经完成的 Turn。
- Turn 启动后继续使用 Event 显示首个 Delta 和流式文本。
- 终态 Event 到达时读取持久化 Session，并以 Store 中的 Turn 状态和文本为准。
- `conversation_turn_cancel` 返回 `cancelRequested = true` 后，不只等待 Event；必须进行有界 Session 对账，直到目标 Turn 不再是 `running`。
- Event 丢失、延迟、重复、顺序竞争或 Listener 重建不得触发第二次 Provider 调用。
- 对账失败显示稳定错误并允许用户再次加载状态；不得无限显示“AI 正在回复”或“正在停止”。

#### 验收

- [ ] Listener Promise 被人为延迟时，发送入口保持不可用；就绪后正常发送。
- [ ] 终态 Event 被人为丢弃，但 Store 已为 `completed` 时，UI 自动显示完整回复并退出活动状态。
- [ ] 终态 Event 被人为丢弃，但 Store 已为 `failed`、`cancelled` 或 `not_generated` 时，UI 显示相应持久化终态。
- [ ] `cancelRequested = true` 且不投递 `cancelled` Event 的确定性 Loopback 中，UI 在两秒内退出“正在停止”。
- [ ] 延迟或重复终态 Event 不产生重复消息、重复终态或重复 Provider 调用。
- [ ] 每个用户 Turn 的 Provider Calls 仍为 1，其他操作仍为 0。
- [ ] Conversation、Provider 和 UI 定向回归测试通过。

### NRG1-R02——移除旧 Workflow 项目契约残留

#### 问题

Workflow Runtime 与资源已经退役，但新建工程和打开校验仍依赖 `workflow_pack_version = 1.0.0`，创建预览仍把它传到 TypeScript 和 UI。这不是当前 NR-0 所需的数据中立能力。

#### 实施边界

- 从 `novelgainker-domain`、`app-core`、Tauri DTO、TypeScript Contract、创建预览、UI 文案和测试中移除旧 Workflow 版本字段及常量。
- 不新增 `projectTemplateVersion`、`runtimePackVersion` 或其他替代包装字段。
- 旧 `.novelgainker` 应用元数据不迁移、不转换、不读取 Workflow 语义；它不属于小说 Canon。
- 只允许重建必要的项目绑定元数据；不得修改 `project.md`、`characters/`、`world/`、`materials/`、`plot/`、`chapters/` 或资产。
- 不恢复任何 Workflow Registry、Reader、资源包、Feature Flag 或隐藏 UI。

#### 验收

- [ ] 当前 Rust/TypeScript 数据契约不存在 `workflow_pack_version`、`WORKFLOW_PACK_VERSION` 或 `workflowPackVersion`。
- [ ] 新建工程预览和用户界面不显示 Workflow/模板包版本。
- [ ] 新建、打开、保存和关闭当前工程路径通过。
- [ ] 合成旧元数据 Fixture 不进入兼容 Reader；小说 Canon 与未知文件保持不变。
- [ ] 清理前后 Canon Tree Hash 与 NR-001 基线一致。
- [ ] 不新增迁移器、旧 Reader、Alias、Feature Flag 或双 Schema 产品路径。
- [ ] Project、Workspace 和前端 Contract 定向回归测试通过。

### NRG1-R03——NR-G1 门禁真实性与完整覆盖

#### 问题

当前 NR-006 只扫描手选文件，并把若干结论直接写入 Evidence。边界脚本未从 TypeScript Client 提取完整命令集合，却在 Report 中声称已经逐项比较。

#### 实施边界

- 建立一个当前产品面清单，覆盖 Rust/TypeScript 源码、Contracts、IPC、UI、Tests、Scripts 和权威 Docs。
- 对生产源码和用户可见 UI 做全量退役引用扫描；Gate-only 负向字面量使用精确文件和用途 allowlist。
- 从 Manifest 提取 24 条 Command，并分别与 Handler、Permission、Capability 和 TypeScript Client 的实际命令集合比较。
- 证明 3 条 Conversation Command 名称精确一致；Event 不计入 Command。
- `providerCallsPerTurn`、`optionalOperationsPerTurn`、`releaseConversationPathCount`、`retiredProductReferences`、`conversationStartsBlank`、`automatedExternalRequests` 必须由实际断言或上游 Evidence 推导。
- Evidence 包含用于得出结论的文件范围和稳定 Hash；报告内容与 Evidence 一致。
- 不以删测试、缩小扫描范围或增加例外通配符的方式让 Gate 通过。

#### 验收

- [ ] Manifest、Handler、Permission、Capability、TypeScript Client 均为同一 24 条 Command 集合。
- [ ] Conversation Command 精确为 `conversation_session_load`、`conversation_turn_start`、`conversation_turn_cancel`。
- [ ] 当前产品面零旧 Agent/Workflow 产品引用；Gate-only 检测字面量均有精确 allowlist 和理由。
- [ ] 门禁能通过合成残留 Fixture 证明：在此前漏扫文件中加入退役引用会可靠失败。
- [ ] NR-G1 Manifest 不直接硬编码待证明结论。
- [ ] NR-G1 Report 不再包含超出机器证据覆盖范围的声明。
- [ ] 完整 `pnpm quality`、Debug/Release 构建和 Windows 离线 Smoke 通过。
- [ ] 真实 Provider Live Smoke 仍未执行，自动化外部请求仍为 0。

## 6. 明确不包含

- Memory、自动压缩、摘要、长上下文和规模预算；
- Novelist Skill Registry、Skill 语义判断或 Context 组装；
- Agent Tool Loop、机器 Tool Request/Result 或 Host Markdown 自动写入；
- Canon 自动维护、冲突处理、Diff、确认、Undo 或章节生产闭环；
- 真实 Provider Live Smoke；
- NR-1～NR-6 的任何实施。

## 7. 重新提交 NR-G1 的条件

只有以下条件全部满足，才能形成新的 NR-G1 技术提交：

1. NRG1-R01～NRG1-R03 全部完成；
2. 三项新增回归证据均为通过；
3. 完整 `pnpm quality` 通过；
4. Canon Tree Hash 保持 NR-001 基线；
5. NR-G1 Manifest 与 Report 由修正后的 Gate 重新生成；
6. `nrG1Approved = false`，重新提交后仍等待产品评审；
7. 没有进入 NR-1，也没有执行真实 Provider Live Smoke。

## 8. Review Gate

当前 Gate：`NR-G1-REMEDIATION-PACKET-REVIEW`。

本 Draft 1 仅供评审。产品负责人明确批准前：

- 不实施 NRG1-R01～NRG1-R03；
- 不修改 `nrG1Approved`；
- 不进入 NR-1；
- 不运行真实 Provider Live Smoke。
