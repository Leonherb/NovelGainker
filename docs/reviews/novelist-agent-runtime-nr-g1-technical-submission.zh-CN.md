# NovelGainker NR-G1 技术重新提交

提交日期：2026-09-01  
实施范围：NR-001 → NR-006；NR-G1-R01 → NR-G1-R03  
技术提交时状态：`NR-G1-technical-submission`  
当前 Gate 状态：`passed`  
产品批准：`nrG1Approved = true`（2026-09-01）

## 1. 重新提交结论

NR-G1 的三项阻断已按批准顺序完成修正：Conversation Event 只承担低延迟通知，Store 负责终态真值与自动对账；工程 Metadata 已执行 `schemaVersion 1 -> 2` 一次性 Clean Slate；Gate 已收缩为递归发布面、五层 IPC 闭包、上游机器证据和当前 Report 一致性。

当前已实现版本仍只是 NR-0：用户可以在三栏界面的中栏与 AI 连续自然交流，但不加载 Novelist Skill、不读取 Canon 作为模型上下文、不调用工具，也不由 AI 写入 Markdown。产品负责人已批准本次 NR-G1 技术提交；该批准只允许形成 NR-1 Story Packet，不构成 NR-1 实施或真实 Provider Live Smoke 授权。

## 2. Remediation 交付

| Story     | 状态      | 技术结果                                                                                                                               |
| --------- | --------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| NR-G1-R01 | completed | listener 就绪前禁止发送；活动 Turn 每 1000ms 非重叠读取 Store；四种终态和同步失败可恢复                                                |
| NR-G1-R02 | completed | Project Metadata v1 首次打开安全重置为 v2；新 Project ID/Conversation 后续保留；Canon、凭据和无关状态不变                              |
| NR-G1-R03 | completed | 递归扫描 43 个发布文件；Manifest/Handler/Permission/Capability/全部生产 TS Client 严格等于 24 条；负向生产文件和 Report 漂移自测均拒绝 |

原 NR-001～NR-006 的自然对话、Conversation Store、Provider Loopback、关闭生命周期与离线编辑能力保持不变。

## 3. 冻结事实

| 项目                          | 结果                      |
| ----------------------------- | ------------------------- |
| Production Commands           | 24                        |
| Conversation Commands         | 3                         |
| Conversation Event            | `conversation-turn-event` |
| Provider Calls / Turn         | 1                         |
| Skill Calls / Turn            | 0                         |
| Tool Calls / Turn             | 0                         |
| Canon Operations / Turn       | 0                         |
| Store reconciliation 最大间隔 | 1000ms                    |
| Release Conversation 路径     | 1                         |
| 发布面退役产品引用            | 0                         |
| 新 Conversation 为空          | true                      |
| 自动化外部请求                | 0                         |
| 真实 Provider Live Smoke      | 未执行                    |

三条 Conversation Command：

1. `conversation_session_load`
2. `conversation_turn_start`
3. `conversation_turn_cancel`

<!-- NR-G1-MANIFEST-CLAIMS:START -->

```json
{
  "status": "NR-G1-technical-submission",
  "nrG1Approved": false,
  "productionCommandCount": 24,
  "conversationCommandCount": 3,
  "providerCallsPerTurn": 1,
  "skillCallsPerTurn": 0,
  "toolCallsPerTurn": 0,
  "canonOperationsPerTurn": 0,
  "sessionReconciliationMaximumIntervalMs": 1000,
  "releaseConversationPathCount": 1,
  "retiredProductReferences": 0,
  "conversationStartsBlank": true,
  "automatedExternalRequests": 0,
  "realProviderLiveSmokeExecuted": false
}
```

<!-- NR-G1-MANIFEST-CLAIMS:END -->

该区块冻结技术提交生成时的机器可核对声明，因此保留 `nrG1Approved = false`；它会逐项与当时的 Manifest、实际扫描、IPC 集合和上游 Evidence 比较。2026-09-01 的产品批准属于提交后的 Gate Decision，记录在本文页首与 BMAD 当前状态中，不重写历史 Manifest。删除上游 Evidence、篡改任一 IPC 集合或改变该冻结区块都会使 Gate 失败。

## 4. 用户可见行为

- 启动只要求新建或打开工程，不要求人物、世界、剧情或章节预设。
- 中栏只显示工程绑定的自然对话；普通闲聊、小说讨论与直接创作走同一路径。
- listener 建立完成前不能发送；活动 Turn 即使丢失终态 Event，也会在最多 1000ms 后服从 Store 真值。
- 用户消息在 Provider 调用前落盘。非空 Provider 文本不会因异常终态丢失；零文本失败显示 `本次回复未生成。`。
- 窗口关闭仍只受未保存手动 Markdown 阻断，提供 `保存 / 放弃 / 取消`；确认关闭后直接退出并取消活动 Turn。

## 5. 数据与安全边界

- 小说 Markdown、素材和图片是唯一 Canon。
- Project Metadata 当前通用格式为 `schemaVersion = 2`，不含 Workflow 版本字段。
- v1 工程只在取得 writer lease、通过规范化根路径与 reparse 检查后重置精确 `.novelgainker/**`；首次成功后生成新 Project ID，后续打开不再重置。
- 同根旧 Project ID 的 `recent_projects` 与 `project_files` 在一个数据库事务中精确删除；Provider Settings、UI Preferences、无关项目状态和 Windows Credential Manager 凭据不在重置范围。
- Conversation Store 位于 `.novelgainker/conversation/session.json`，不进入小说索引或出版输入。
- 本阶段不保存流式字符，不实现 Memory、自动压缩、Skill、Tool Loop 或 AI Markdown 写入。

## 6. Gate 与证据

2026-09-01 完整 `pnpm quality` 已通过：前端 18 个 Test File / 102 项测试，Rust format、Clippy 与工作区测试，Sprint 1 离线 E2E，Tauri Debug/Release 构建，两次 Windows 窗口 Smoke，以及 Canon/Recovery Kernel 全部通过。Release 截图 SHA-256 为 `602bca1b893a8e071d234f5d5fdabf4b363290075504e67877cd35899ee6535b`。

- `target/qa/nr-002/nr-002-evidence.json`：v1→v2 一次性切换、Canon 与全局精确状态边界。
- `target/qa/nr-004/nr-004-evidence.json`：Provider/Skill/Tool/Canon 每 Turn 调用数和离线 Loopback。
- `target/qa/nr-005/nr-005-evidence.json`：R01 Store 对账、四终态、同步失败与关闭生命周期。
- `target/qa/nr-006/nr-g1-manifest.json`：递归发布面、五层 IPC 集合、范围 Hash、负向生产文件与 Report 漂移自测。
- `target/nr-g1-evidence/tauri-nr-g1-window.png`：Release 桌面离线 Smoke 截图。

`pnpm quality` 是最终技术验收入口，覆盖格式、lint、TypeScript、React、R01/R02/R03 Evidence、Rust format/Clippy/tests、离线编辑 E2E、Tauri Debug/Release 构建与 Windows 离线 Smoke。自动化环境不读取真实 Provider Key，不访问真实 Provider。

## 7. 明确未包含

- Memory、自动压缩、长对话预算和规模冻结；
- Novelist Skill 加载与语义选择；
- AI 请求文件操作及 Host 执行 Markdown 更新；
- 小说 Canon 上下文读取、持续设定维护、冲突解决；
- 长篇章节生产与出版闭环；
- 真实 Provider Live Smoke。

## 8. Gate Decision

产品负责人于 2026-09-01 明确批准 NR-G1 技术提交。当前授权边界为：

```yaml
status: passed
nrG1Approved: true
nr1PacketAuthorized: true
nr1ImplementationAuthorized: false
realProviderLiveSmokeAuthorized: false
```

下一步只形成并评审 NR-1 Story Packet；实施仍停止，不进入 NR-1 代码或真实 Provider Live Smoke。
