# NovelGainker Agent Protocol V2 与 Context Mark 评审规格

> 状态：Revision 2.1；Revision 2 已于 2026-08-30 获产品批准，2.1 仅补齐 AM-R1 实施契约；不自动授权实现
>
> 日期：2026-08-30
>
> 上游 Plan：`agent-conversation-authoring-rebaseline-plan.zh-CN.md`
>
> 目标：让产品负责人完整审查 Agent 如何对话、选择 Skill、构造 Context、生成长内容、形成 Canon、解决冲突、写入工程并保存历史

Revision 2 已修订：

1. Conversation 从 Control JSON 完全分离为独立文本流；
2. Memory 改为 Host 增量更新，只在阈值触发时调用可选压缩；
3. 预算增加每 Turn 累计 Provider 调用、输入、输出、Context、Segment 和修复上限；
4. Skill 候选过滤、项目内 History、Context Mark 锚点、冲突分级和 V1 删除条件按评审意见收紧。

Revision 2.1 实施澄清：

1. `control` 模式由 Host 为每次调用固定 `skill_selection_v2` 或 `agent_action_v2` 子契约，不进行 Union 猜测；
2. 四个项目读取能力是 Host 内部 Context Executor，Provider 只能提交语义 `request_context`；
3. Budget Profile 按 GAR1/GAR2/GAR3 已激活能力分阶段校准，GAR3 完成全能力规模门。

## 1. 本协议解决什么

现有 V1 把四类职责塞进同一个 JSON：

1. AI 给用户的回复；
2. AI 请求读取项目；
3. AI 生成角色、设定或章节正文；
4. AI 请求 App 把正文写入路径。

这会让长 Markdown 必须在控制 JSON 中转义，Provider 输出稍长就可能截断；格式修复又会要求模型“返回更短内容”，与完整写章目标冲突。V2 将职责拆开：

| 平面         | 负责内容                              | 输出形式            |
| ------------ | ------------------------------------- | ------------------- |
| Conversation | 用户与 AI 实际看到的对话              | 可流式文本          |
| Control      | Skill、Context 请求、创作任务和下一步 | 严格短 JSON         |
| Content      | 角色卡、设定、剧情、章纲、章节 Draft  | 原始 Markdown 流    |
| Canon        | 冲突、Diff、风险、事务和最终写入      | Rust 生成的严格结构 |
| Persistence  | 对话历史、记忆、Mark 和终态 Turn      | 项目内版本化文件    |

协议目标：

- 中栏只显示 Conversation，不显示 Tool/Diff/Context 卡。
- 长内容不再放进 `toolCalls[].arguments.content`。
- 模型不能决定最终磁盘路径、风险或是否提交。
- 所有进入 Canon 的内容都有用户消息或 Canon 来源。
- 每次读取、冲突、写入和撤销都能追溯。
- 项目移动到另一台机器后，对话历史、记忆和 Mark 可以恢复。

协议非目标：

- 不暴露隐藏推理或 Provider 原始响应。
- 不允许模型调用任意文件系统、URL、Shell、Process 或外部 Skill。
- 不让会话记忆、Mark 或检索索引成为小说事实的第二份 Canon。
- 不支持并行 Canon Writer 或自动跨 Provider Fallback。

## 2. Context Mark 到底是什么

Context Mark 是项目内持久化的“上下文书签”。它告诉 Context Planner：

> 后续讨论或写作时，优先检查这个角色、规则、剧情线程、章节或 Markdown 标题区间。

例如作者和 AI 正在讨论第三次转折，可以固定：

- `characters/lin-ye.md#秘密`
- `world/sea-ban.md#规则`
- `plot/main.md#第三次转折`

下一轮作者只说“让他在下一章发现漏洞”，Context Planner 也会先检查上述三处，而不是只靠聊天里“他”和“漏洞”的字面匹配。

### 2.1 Mark、Canon、Memory、History 的区别

| 对象           | 保存什么                           | 是否小说 Canon | 作用               |
| -------------- | ---------------------------------- | -------------- | ------------------ |
| Canon Markdown | 角色、世界、剧情和章节事实         | 是             | 小说真相           |
| Context Mark   | 指向 Canon 位置的引用              | 否             | 提高后续检索优先级 |
| Session Memory | 当前目标、已确认决定、未决问题摘要 | 否             | 压缩长对话         |
| Chat History   | 完整可见用户/AI 消息               | 否             | 恢复对话与审计     |
| Context Index  | 从 Canon 派生的搜索结构            | 否             | 快速检索，可重建   |

Mark 不保存“林夜其实是皇子”这句话的副本，而是保存“读取林夜人物卡的秘密章节”。如果这条事实尚未写入人物卡，AI 必须先创建 Canon Proposal，不能只靠 Mark 把它变成长期事实。

### 2.2 谁可以创建和删除 Mark

- 用户可以固定、取消固定任何合法 Canon 引用。
- AI 只能标记当前 Turn 实际读取过的 Canon Source；可以直接创建可见的 session Mark，右栏立即显示“AI 已标记”，用户可以取消。
- AI 创建的 Mark 默认 priority 为 `medium`；`high` 只能由用户设置。
- AI 可以建议升级为 project Mark，但只有用户明确固定后才生效。
- AI 不能删除用户创建的 Mark，只能建议删除。
- 删除 Mark 不修改被引用的 Markdown。
- 每项目最多 64 个活动 Mark；超过上限必须由用户整理。

### 2.3 Mark 的作用域

- `session`：只服务当前写作会话；创建新 Session 时不自动继承。
- `project`：跨 Session、重启和工程移动持续有效。

AI 默认创建 `session` Mark；Session 关闭时自动归档，不注入新 Session。只有用户明确固定为“工程重点”时才升级为 `project`。

### 2.4 Mark 的失效

Mark 同时绑定稳定字段/标题路径、锚点 Hash 和文档 Hash。文件变化后：

1. 只有文档 Hash 变化、锚点 Hash 未变化：说明改动发生在无关段落，更新文档 Hash，Mark 保持 `active`；
2. 字段路径仍存在且内容变化：更新锚点 Hash，并要求本轮 Context Planner 重新读取；
3. 标题移动但实体 ID/字段路径仍可唯一定位：重新绑定并显示“已重新定位”；
4. 找不到或出现多个候选：状态变为 `stale`，不再静默注入正文；
5. 用户在右栏选择重新绑定或删除。

### 2.5 ContextMarkV1

```json
{
  "schemaVersion": 1,
  "markId": "mrk_01J...",
  "scope": "session",
  "createdBy": "agent",
  "label": "海禁规则",
  "reason": "当前第三次转折依赖这条世界规则",
  "target": {
    "kind": "canon_section",
    "projectRelativePath": "world/sea-ban.md",
    "entityId": "setting_sea_ban",
    "fieldPath": "rules",
    "headingPath": ["规则"],
    "anchorHash": "64-hex",
    "documentHash": "64-hex"
  },
  "priority": "medium",
  "state": "active",
  "createdAtMs": 1788019200000,
  "updatedAtMs": 1788019200000
}
```

结构化实体优先使用由 Schema 定义的 `fieldPath`。自由 Markdown 使用完整 `headingPath`、同名标题 ordinal 和 excerpt Hash 形成稳定锚点。模型只可以提出 `label`、`reason`、当前 Turn 已读取的实体/Source 引用和建议作用域；最终路径、锚点、Hash、状态、priority 与 ID 由 App 解析和填写。

## 3. 信任与权限边界

| 主体               | 可以决定                                                   | 不能决定                         |
| ------------------ | ---------------------------------------------------------- | -------------------------------- |
| 用户               | 写作请求、冲突决议、高风险确认、Mark 固定/取消             | 绕过项目路径和文件校验           |
| Provider           | 语义理解、Skill 建议、Context 查询、创作 Draft、Canon 候选 | 权限、绝对路径、风险、提交、Undo |
| Skill Pack         | 写作规则、模板、参考资料、允许能力声明                     | 扩大 Host 权限、执行外部脚本     |
| Context Engine     | 索引、检索、来源绑定、预算选择                             | 创造没有来源的 Canon 事实        |
| Rust Policy/Writer | 路径、Schema、Hash、风险、冲突门、事务、恢复               | 替作者决定语义冲突               |
| UI                 | 展示对话、来源、冲突、Diff、确认和撤销                     | 直接把 Provider 输出写入文件     |

安全不变量：

1. Project/Session/Turn/Provider/Skill Pack/Policy 绑定全部由 App 注入。
2. 模型输出、项目 Markdown、聊天历史和 Tool Result 都是不可信数据。
3. 所有文件操作必须使用项目相对路径，并通过路径、符号链接、UTF-8、类型和大小校验。
4. 一个 Turn 最多一个 Canon 事务；混合风险按最高风险处理，禁止部分提交。
5. 冲突未解决、来源 Hash 变化、取消或任一前置校验失败时写入数为 0。

## 4. 项目内持久化

RB-Q004 决定把完整可见聊天和会话记忆保存在工程内。建议目录：

```text
.novelgainker/
  project.json
  agent/
    manifest.json
    context-marks.json
    sessions/
      ses_.../
        session.json
        memory.json
        turns/
          000001-trn_....json
          000002-trn_....json
        drafts/
          art_....md
```

### 4.1 文件职责

- `manifest.json`：Agent 数据 Schema、当前 Session、序号和整体 Hash。
- `session.json`：Provider/Skill Pack/Policy 快照、Session 状态和活动 Turn 恢复标记。
- `turns/*.json`：终态 Turn；保存完整可见用户/AI 消息、来源引用、Mark 变化、冲突决议和写入结果。
- `memory.json`：当前目标、作者已确认决定、未决问题和带来源引用的摘要。
- `context-marks.json`：全部活动、stale 和最近删除 Mark。
- `drafts/*.md`：只保存用户已经看到但尚未提交/丢弃的 Draft；提交或丢弃后清理。

### 4.2 允许保存

- 用户输入和最终向用户展示的 AI 文本；
- 用户已经看到的 Draft；
- Skill ID、Source ID、项目相对路径、内容 Hash；
- 冲突卡、用户选择、Diff/事务/Undo 的非正文引用；
- 稳定、脱敏错误码。

### 4.3 禁止保存

- API Key、credential reference 或 Key 特征；
- Provider 原始响应、请求头、隐藏推理、内部 system/developer prompt；
- 工具返回的未展示全文、绝对路径和系统环境信息；
- 未脱敏异常、网络响应体和任意外部用户数据。

项目内 Agent 文件默认可随 Git/同步工具移动，因此首次创建工程时必须明确提示：聊天可能包含私人创作内容。用户可以清空某个 Session 或全部 Agent 历史；清空不修改 Canon Markdown。

`.novelgainker/agent/**` 必须从普通项目文件树写工具、Canon Search/Context Index、出版输入和 Novelist Skill 读写范围中排除。Orchestrator 只能通过专用 History/Memory/Mark Reader 读取，并始终标记为不可信数据。

打开已有工程时必须验证 Agent Manifest、Schema、项目绑定、序号和 Hash Chain；损坏或伪造记录进入隔离状态，不能成为 Prompt instruction。跨设备同步如果产生同一 Session 的 Manifest 分叉，不自动合并，创建新的 Session branch 并让用户选择保留。

未决 Draft 必须有独立的文件数/字节配额和可见清理入口；达到配额时停止创建新 Draft 并请求用户处理，不能静默删除。

### 4.4 写入与恢复

- 终态 Turn、Memory、Mark 和 Manifest 使用临时文件、flush、原子替换。
- 活动 Turn 不边生成边覆盖正式 Turn 文件；进程崩溃后写入一个可见失败终态，并保留用户输入。
- 最后一个文件损坏时只隔离该文件并报告，不删除其他聊天历史或 Canon。
- Memory 和 Mark 可以从 Turn 历史与 Canon 重建；搜索索引始终可从 Canon 重建。

## 5. Session 与 Turn

### 5.1 ProjectAgentSessionV2

必需字段：

| 字段                          | 含义                                                               |
| ----------------------------- | ------------------------------------------------------------------ |
| `schemaVersion`               | 固定为 2                                                           |
| `projectId` / `sessionId`     | App 生成的不透明绑定                                               |
| `status`                      | `ready / turn_active / awaiting_user / recovery_required / closed` |
| `providerSnapshot`            | Provider、Preset、Model，不含凭据                                  |
| `skillPackSnapshot`           | Pack 版本和内容 Hash                                               |
| `policySnapshot`              | `plan_only / review_every_write / auto_apply_safe`                 |
| `conversationRef`             | 项目内 Session 目录                                                |
| `memoryHash` / `marksHash`    | 当前可见记忆和 Mark 版本                                           |
| `activeTurnId`                | 空或唯一活动 Turn                                                  |
| `writerLease`                 | released / held / recovery_required                                |
| `createdAtMs` / `updatedAtMs` | 单调时间字段                                                       |

Provider 或 Policy 只允许在没有活动 Turn、Pending Conflict、Pending Confirmation 时切换。

### 5.2 AgentTurnV2

每个 Turn 冻结：

- 用户消息 ID 和可见正文 Hash；
- Provider、Skill Pack、授权策略；
- 会话 Memory、Context Marks 和 Canon Index 版本；
- Context、Provider、Composition、修改文件和事务预算；
- 唯一 Writer 绑定。

状态机：

```text
accepted
  → selecting_skills
  → planning_context ↔ gathering_context
  → responding ───────────────────────────────→ completed
  → planning_composition → composing → reconciling
      → awaiting_conflict_resolution
      → awaiting_write_confirmation
      → committing → refreshing_context → responding → completed
```

任何非提交阶段都可以进入 `cancelling → cancelled` 或 `failed`。事务真正开始后必须原子完成、回滚或进入 `recovery_required`，不能标记普通 cancelled。

### 5.3 一个 Turn 的硬性不变量

- 同时最多一个活动 Turn、一个 Composition Job 和一个写事务。
- Context Tool Result 在整个 Turn 中累计保存，不能在下一批工具前清空。
- 用户输入、最终回复和失败状态都必须形成项目内终态记录。
- `completed` 之前必须由 Host 应用确定性的 `SessionMemoryDeltaV2`；该步骤不调用 Provider。
- Memory Compaction 不是 Turn 完成前置条件；压缩失败保留上一个有效摘要和完整 History，并显示“记忆压缩待重试”，不回滚 Canon。

## 6. Provider 请求类型与 Prompt 层级

V2 不要求一次 Provider 输出同时完成所有工作。每次调用固定一种模式：

| 模式                | 输出                           | 是否允许一次同 Provider 修复      |
| ------------------- | ------------------------------ | --------------------------------- |
| `control`           | Host 固定的一个 Control 子契约 | 是                                |
| `conversation`      | 用户可见普通文本流             | 否；失败形成可见终态              |
| `composition`       | 原始 Markdown 内容流           | 否；截断走续写，不改写为短 Action |
| `reconciliation`    | `CanonProposalV1` 短 JSON      | 是                                |
| `memory_compaction` | `MemoryCompactionV1` 短 JSON   | 是；仅达到阈值时调用              |

每次 Provider 调用都由 Host 创建不可由模型覆盖的 `ProviderInvocationV2`，至少冻结：

| 字段                 | 规则                                                                                                                                                        |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `requestId`          | App 生成并绑定 Turn                                                                                                                                         |
| `mode`               | 上表五种模式之一                                                                                                                                            |
| `completionContract` | `skill_selection_v2 / agent_action_v2 / conversation_text_v1 / composition_markdown_v1 / canon_proposal_v1 / memory_compaction_v1` 中与当前 mode 匹配的一项 |
| `budgetLeaseId`      | 本次调用的预扣预算绑定                                                                                                                                      |
| `cancellationId`     | 当前 Turn 的统一取消绑定                                                                                                                                    |

`control` 保持单一 Provider 模式，但分为两个互斥子契约：

1. Turn 位于 `selecting_skills` 时，Host 固定 `completionContract=skill_selection_v2`，只加载 `SkillSelectionV2` Schema；
2. Skill 已冻结后，Host 固定 `completionContract=agent_action_v2`，只加载 `AgentControlActionV2` Schema；
3. Provider 返回另一子契约、混合两种字段或未知字段时，不尝试 Union 猜测，按当前子契约执行最多一次同 Provider 修复；
4. 修复、输入和输出全部计入同一 Turn 累计预算。

Prompt 层级固定为：

1. **System**：不可变 App Policy、信任边界、取消与禁止项；
2. **Developer/Host**：已选 Skill 的完整 instruction、允许能力、当前输出 Schema；
3. **Context Data**：Session Memory、Context Plan、Skill 参考、Canon 摘要/片段和 Tool Results；
4. **User**：当前可见用户消息；
5. **Completion Contract**：本次只能返回哪一种协议。

Provider 不支持 developer/tool role 时，Adapter 可以合并到 system 和带边界标记的 untrusted data block，但不得把 App Policy 或 Skill instruction 当成普通 user message。Canon、聊天和 Tool Result 中的任何文字都不能改变上面三层权限。

## 7. AgentControlActionV2

### 7.1 公共字段

所有 Control Action 都必须包含：

| 字段            | 规则                                          |
| --------------- | --------------------------------------------- |
| `schemaVersion` | 固定为 2                                      |
| `requestId`     | App 生成并要求 Provider 原样返回              |
| `action`        | 下列四种之一                                  |
| `skillIds`      | 0～4 个，唯一、必须来自本 Turn Host 候选集    |
| `visibleReason` | 最多 512 UTF-8 字节，可展示，不得包含隐藏推理 |

零 Skill 只允许产品帮助或完全不涉及项目 Canon 的普通交流。一旦请求读取、创作、Canon 化、评审或出版，至少选择一个 Skill。

### 7.2 `request_context`

用于请求 Context Engine 查找资料，不直接指定最终文件操作。

```json
{
  "schemaVersion": 2,
  "requestId": "req_01J...",
  "action": "request_context",
  "skillIds": ["create-setting", "create-plot"],
  "visibleReason": "需要核对海禁规则与第三次转折",
  "queries": [
    {
      "kind": "entity_or_rule",
      "terms": ["海禁", "第三次转折"],
      "knownEntityIds": [],
      "knownSourceIds": [],
      "purpose": "定位相关世界规则和主线节点"
    }
  ]
}
```

`queries` 每个 Action 最多 8 个。允许的 `kind`：

- `entity_or_rule`
- `relationship`
- `plot_thread`
- `chapter_or_scene`
- `timeline`
- `clue_or_material`
- `full_text`
- `marked_context`
- `skill_resource`

模型可以描述语义查询，但不能要求绝对路径、项目外路径、网络或任意文件系统。

`project.list_markdown / project.read_markdown / project.search_markdown / project.validate_markdown` 只作为 Host 内部 Context Executor 存在。它们的工具名、路径参数和调用 Schema 不进入 Provider Completion Contract；Provider 只能返回 `request_context.queries`。Host 把语义查询编译为内部读取计划，执行路径、权限与预算校验，并只以 `SourceRefV2` 和有界片段返回结果。

### 7.3 `start_response`

用于授权 Host 建立独立 Conversation 输出，不携带实际回复正文：

```json
{
  "schemaVersion": 2,
  "requestId": "req_01J...",
  "action": "start_response",
  "skillIds": ["novel-story-telling"],
  "visibleReason": "已根据现有章节和人物卡完成判断",
  "responseBrief": "回答林夜当前知识边界，并解释依据",
  "sourceIds": ["src_01J...", "src_01K..."],
  "markSuggestions": []
}
```

- `responseBrief` 最多 1024 UTF-8 字节，只描述回答目标，不是用户可见回复正文。
- `sourceIds` 必须来自当前 Turn 已读取或已绑定的 Context。
- `markSuggestions` 最多 8 个，只能引用已知 Source/Entity；App 解析成 `ContextMarkV1`。
- Host 接受后生成 `ConversationLeaseV1`，冻结 Response ID、Context Plan Hash、Source IDs、最大输出字节、Provider/Skill 快照与取消绑定。
- Provider 随后以 `conversation` 模式输出原始普通文本流；该流不需要满足 JSON Schema。
- Conversation 完成后 Host 生成 `ConversationMessageV2`，保存可见正文、正文 Hash、Source IDs 和完成状态，并把 Turn 转为终态。

`ConversationLeaseV1` 至少包含 `responseId / turnId / contextPlanHash / sourceIds / maxOutputBytes / providerSnapshot / skillIds / cancellationId / createdAtMs`。`ConversationMessageV2` 至少包含 `schemaVersion / messageId / responseId / turnId / role / body / bodyHash / sourceIds / status / createdAtMs`；其中 `role` 固定为 `assistant`，`status` 只能是 `complete / cancelled / failed`。这些字段均由 Host 创建或校验，Provider 不能在正文流中覆盖。

Conversation 输出禁止包含 Tool/Path/Policy 指令；它只能被展示和持久化，不能触发新的工具或写入。如果还需要读取或写入，必须在 `start_response` 前通过 Control Action 完成。

### 7.4 `start_composition`

用于请求 App 建立内容生成任务：

```json
{
  "schemaVersion": 2,
  "requestId": "req_01J...",
  "action": "start_composition",
  "skillIds": ["novel-story-telling"],
  "visibleReason": "开始生成第十二章 Draft",
  "spec": {
    "artifactKind": "chapter",
    "operation": "create",
    "targetEntity": {
      "kind": "chapter",
      "stableId": "chapter-012",
      "displayName": "潮门之夜"
    },
    "objective": "林夜第一次发现海禁规则存在人为漏洞",
    "desiredLengthChars": 6000,
    "sourceIds": ["src_01J...", "src_01K..."],
    "constraints": ["不得让林夜提前知道幕后主使", "结尾必须推动第三次转折"]
  }
}
```

允许的 `artifactKind`：

- `project_note`
- `character`
- `setting`
- `material`
- `plot`
- `outline`
- `chapter`
- `review_report`

`operation` 只能是 `create / revise / extend / merge_candidate`。模型不能提交 `path`、`risk`、`confirmed`、`content` 或事务字段。

### 7.5 `submit_canon_proposal`

用于把聊天中已经明确的小型事实或已生成 Draft 交给 Host 进行 Canon 化。它不等于写入授权：

```json
{
  "schemaVersion": 2,
  "requestId": "req_01J...",
  "action": "submit_canon_proposal",
  "skillIds": ["create-character"],
  "visibleReason": "把已确认的身份秘密补充到林夜人物卡",
  "proposal": {
    "proposalId": "prp_01J...",
    "operation": "merge",
    "targetEntity": {
      "kind": "character",
      "stableId": "character_lin_ye",
      "displayName": "林夜"
    },
    "artifactIds": [],
    "facts": [
      {
        "subject": "character_lin_ye",
        "predicate": "secret.identity",
        "value": "真实身份与前朝皇族有关",
        "sourceMessageIds": ["msg_01J..."],
        "sourceIds": []
      }
    ]
  }
}
```

Provider 给出的 `proposalId` 只作为提议内关联键；Host 会重新分配受信任 ID。最终文件路径、模板、Patch 形态、风险和提交状态全部由 Host 生成。

## 8. Skill Selection 与 Context 协议

### 8.1 Skill 选择

Skill 选择由 Provider 做语义判断，Host 做边界约束：

1. Host 根据 Skill 是否随发行包启用、项目是否已打开、所需 Host 能力是否可用和产品阶段授权形成完整 eligible Skill 集合；
2. Provider 在候选集中选择 0～4 个并给出可见理由；
3. Host 验证 Skill 的 allowed capabilities、读写范围和题材参考；
4. 后续每个 Control Action 的 Skill 集合必须是已选择集合的子集；
5. 需要新 Skill 时重新进入 selection，不允许 Provider 自行扩大权限。

Host 不得因为用户消息缺少某个关键词而排除本来 eligible 的 Skill。本地关键词只能帮助 Provider 排序展示或 Context Search，不能决定候选资格，也没有默认 storytelling 回退。

`SkillSelectionV2`：

```json
{
  "schemaVersion": 2,
  "requestId": "req_01J...",
  "selectedSkillIds": ["create-setting", "create-plot"],
  "intentClass": "canon_update",
  "visibleReason": "请求同时改变世界规则和主线转折"
}
```

- `requestId` 由 App 注入并要求原样返回；
- `selectedSkillIds` 必须是 Host 随请求提供的 eligible Skill 集合子集；
- `intentClass` 只能是 `product_help / project_discussion / canon_update / composition / revision / review / visual / publish`；
- 未知、重复或超过 4 个 Skill 安全拒绝；
- Provider 不返回路径、工具、权限或风险。

### 8.2 SourceRefV2

Context Engine 返回的每个来源都有稳定引用：

| 字段                  | 含义                                          |
| --------------------- | --------------------------------------------- |
| `sourceId`            | 当前 Turn 内不透明 ID                         |
| `kind`                | canon / skill / session_memory / chat_message |
| `projectRelativePath` | 仅 Canon 来源存在，由 Host 填写               |
| `entityId`            | 可选稳定实体 ID                               |
| `fieldPath`           | 结构化 Canon 的可选 Schema 字段路径           |
| `headingPath`         | 自由 Markdown 的可选完整标题路径与 ordinal    |
| `contentHash`         | 完整来源 Hash                                 |
| `excerptHash`         | 实际发送片段 Hash                             |
| `trust`               | 固定 untrusted_data                           |
| `truncated`           | 是否截断及原因                                |

Source ID 只在当前 Turn 有效；Context Mark 使用路径、实体、锚点和 Hash 持久化，不直接持久化临时 Source ID。

### 8.3 ContextPlanV1

最终 Context Plan 由 Host 生成，模型只能请求：

```json
{
  "schemaVersion": 1,
  "turnId": "trn_01J...",
  "skillIds": ["create-setting", "create-plot"],
  "mustRead": ["src_01J...", "src_01K..."],
  "supporting": ["src_01L..."],
  "marked": ["mrk_01J..."],
  "recentMessages": ["msg_01J..."],
  "skillResources": ["skr_01J..."],
  "inputBytes": 74216,
  "planHash": "64-hex"
}
```

构造顺序：

1. Session Memory 与最近可见对话；
2. 活动 Context Marks 的摘要；
3. 项目索引命中；
4. 当前请求必须读取的 Canon 原文；
5. 已选 Skill instruction、模板和必要参考资料；
6. 当前 Turn 之前所有 Context Result 的累计集合。

Mark 只提高优先级：项目级 Mark 至少注入摘要，只有与当前任务相关时才注入全文。预算不足时先保留必须来源和约束，再压缩支持性材料；不得静默丢弃本轮先前已经依赖的来源。

### 8.4 每次写入前的 Context Preflight

Canon Proposal 进入 Conflict/Write 流程前必须重新读取：

- 所有目标文件当前 Hash；
- 提案引用的角色/世界规则/剧情线程；
- 章节生命周期和 Story Ledger；
- 相关活动 Mark 是否 stale。

任何 Hash 漂移都返回 Context 重新规划，不能继续使用旧 Draft 直接覆盖。

## 9. Content 与 Composition 协议

### 9.1 ArtifactLeaseV1

Host 接受 `start_composition` 后生成：

- `compositionJobId`
- `artifactId`
- 规范化 artifact kind 和 operation
- Host 解析的目标实体，不包含最终路径承诺
- Context Plan Hash
- 期望长度与最大 UTF-8 字节数
- 分段策略
- Provider/Skill/Policy 快照 Hash

### 9.2 Composition Request

Composition 调用只要求原始 Markdown，不要求 JSON、代码围栏或工具参数。Prompt 包含：

- 完整 Skill instruction 和选中的模板；
- Context Plan 中的来源；
- 章纲/目标/连续性约束；
- 本段 ordinal、上一段尾部 Hash 和已经生成的简短 handoff；
- 严格禁止的提前揭示、角色越权和章节状态操作。

### 9.3 长内容分段

- 短实体或设定可单次完成。
- 章节优先按场景或章纲 Beat 分段，而不是按任意 token 边界硬切。
- 每段完成后保存内容 Hash、首尾重叠检查和累计字符数。
- Provider 截断时从最后一个完整段落恢复，并验证重叠 Hash；不得把截断当成无效 Control Action，也不得要求“返回更短的写工具 JSON”。
- 全部段落合成后再进行 Markdown Schema、连续性和 Canon 冲突检查。

### 9.4 DraftArtifactV1

Draft Artifact 保存：

- Artifact/Job/Turn 绑定；
- kind、operation、标题和稳定实体引用；
- 原始 Markdown 内容或项目内 Draft 文件引用；
- Context Plan Hash、来源 IDs 和内容 Hash；
- 完成状态：`streaming / complete / truncated / cancelled / invalid`；
- 校验诊断。

只有 `complete` 且通过结构校验的 Artifact 可以进入 Canon Proposal。模型不能直接把 Artifact 变成文件。

## 10. Canon Proposal 与冲突

### 10.1 CanonProposalV1

提案必须回答：

- 要创建、修订、扩展还是合并哪个逻辑实体；
- 使用哪些 Draft Artifact；
- 新增或改变了哪些可枚举事实；
- 每个事实来自哪条可见消息或哪个 Canon Source；
- 为什么它与当前写作目标有关。

Host 随后执行实体归一和路径解析。例如人物 `character_lin_ye` 由 Host 映射到已存在的 `characters/lin-ye.md`；新人物路径由安全 slug 和冲突检查生成。Provider 永远不拥有最终路径权。

### 10.2 冲突类型

- `identity`：同一稳定 ID 或同名实体身份不一致；
- `field_value`：人物、地点、规则或剧情字段值冲突；
- `relationship`：人物关系方向或状态冲突；
- `timeline`：时间、年龄、先后顺序冲突；
- `world_rule`：新情节违反已定义规则或例外；
- `plot_thread`：承诺、伏笔、揭示或回收状态冲突；
- `chapter_state`：试图自动修改 accepted/final 或 Ledger；
- `source_hash`：读取后文件被人工修改；
- `semantic`：Agent 发现的潜在含义矛盾，必须附来源。

冲突分析先分类：

- `confirmed_conflict`：新旧命题不能同时成立，并且有明确的新候选与 Canon 证据；进入 ConflictSet 并阻断关联事务。
- `clarification_required`：来源不足、措辞含糊或只存在低置信度语义疑点；不伪装成已证实冲突，先通过 Conversation 向作者澄清。本 Turn 在澄清前不写入相关候选。

语义冲突必须引用至少一处旧 Canon 和一处新候选证据，并给出可证伪的矛盾说明；只有“感觉不一致”不得进入 `confirmed_conflict`。

### 10.3 ConflictSetV1

每个 Conflict Item 必须包含：

- Host 生成的 Conflict ID 和绑定 Hash；
- 冲突类型、目标实体和受影响 Proposal；
- 旧 Canon 的路径、锚点、Hash 和可见摘录；
- 新候选的消息/Artifact 来源和可见摘录；
- 简短可见解释；
- 固定 `blocking=true`；只有 `confirmed_conflict` 可以形成 Conflict Item。

### 10.4 用户决议

固定五种操作：

1. `keep_existing`：保留 Canon，丢弃相关候选；
2. `adopt_candidate`：采用新内容，进入新的 Diff；
3. `merge_manual`：用户在右栏编辑合并结果；
4. `save_as_alternative`：保存到素材/备选，不覆盖原事实；
5. `cancel_change`：取消整个相关事务。

决议提交 `conflictSetHash`、`resolution` 和 `confirmed=true`，不从前端回传隐藏正文。Host 重新运行实体归一、冲突检查、来源 Hash 和风险分类；旧确认不可复用。

`adopt_candidate` 和 `merge_manual` 形成的新 WritePlan 始终归 `always_confirm`；`save_as_alternative` 只能写入 Host 固定的素材/备选范围，也必须经过独立 Diff 和风险分类。

## 11. WritePlanV2、授权与 Undo

### 11.1 Host 内部 Commit 能力

V2 不再向模型注册 `project.create_markdown` 和 `project.patch_markdown` 作为携带正文的工具。它们转为 Host 内部能力：

1. 根据实体类型选择固定模板和目标目录；
2. 把已验证 Draft/Fact 合成为完整候选文档；
3. 对源文件与结果运行 Markdown Schema、领域 Validator 和连续性检查；
4. 由本地 diff engine 计算变更，不要求模型提供 exact-match patch；
5. 分类风险并建立事务；
6. 根据项目 Policy 自动提交或等待用户确认。

### 11.2 WritePlanV2

必需字段：

- Project/Session/Turn 绑定；
- Policy、Provider、Skill Pack、Context Plan 和 Conflict Resolution Hash；
- 每个目标的项目相对路径、源 Hash/必须不存在、结果 Hash；
- 变更类型、完整 Diff 引用、Validator 结果；
- 本地分类的 `write_safe / always_confirm / forbidden`；
- Undo 预计大小和 Ready 状态；
- Source Set、Result Set、Plan 和 Transaction Hash。

模型看不到也不能设置 `confirmed`、`policyDisposition`、`risk`、`writerLease` 或最终路径。

### 11.3 风险规则

`auto_apply_safe` 只允许：

- 无冲突的新人物、世界、素材和普通剧情文件；
- Draft 章节的新建；
- 已验证为局部且不改变受保护字段的安全补充；
- Undo 已准备、所有源 Hash 匹配且完整事务均为 write-safe。

始终确认：

- 任意已解决冲突形成的 Canon 变更；
- 整文档/整章替换和跨章批量修订；
- accepted/final 章节的任何正文改动；
- 章节状态、Story Ledger、关键 `project.md` 元数据；
- 规则、出版、资产、项目外导入或其他高风险能力。

禁止：

- 规则/权限/Skill Pack 自修改；
- 任意绝对路径、路径逃逸、符号链接逃逸；
- Shell、Process、URL、删除、移动、重命名；
- 未注册文件类型、`published/` 源编辑和无来源覆盖。

### 11.4 提交与撤销

- 一个 Turn 最多一个、最多 8 文件的原子事务。
- Commit 前再次获取唯一 Writer、比较全部源 Hash、准备 DPAPI Undo。
- 任一目标变化时整个事务零写入并回到 Context/Conflict 阶段。
- Commit 成功后回读全部结果 Hash，再刷新 Context Index、Memory 和 Marks。
- Undo 只有在当前目标 Hash 仍等于事务结果 Hash 时可执行；否则提示冲突，不覆盖人工修改。

## 12. Chat History 与 Session Memory 协议

### 12.1 TurnRecordV2

每个终态 Turn 文件保存：

- Turn ID、sequence、时间和终态；
- 完整可见用户消息；
- 0～多条实际展示的 AI 消息；
- selected Skill IDs；
- Source/Mark/Artifact/Conflict/Write Receipt 的引用与 Hash；
- 用户冲突决议和高风险确认；
- 脱敏稳定错误码；
- Turn Record Hash 和前一 Turn Hash。

工具原始返回、Provider 原始文本和 system/developer prompt 不进入 Turn Record。大篇章 Draft 通过 Artifact/Canon 引用保存，避免在 JSON 中重复整章。

### 12.2 SessionMemoryV2 与 Host 增量更新

```json
{
  "schemaVersion": 2,
  "sessionId": "ses_01J...",
  "version": 17,
  "currentGoal": "完成第三次转折前的两章铺垫",
  "confirmedDecisions": [
    {
      "text": "海禁令存在人为设置的漏洞",
      "sourceRefs": ["world/sea-ban.md#规则", "turn:trn_01J..."]
    }
  ],
  "unresolvedQuestions": ["林夜何时知道幕后主使"],
  "activeEntityIds": ["character_lin_ye", "setting_sea_ban"],
  "recentTurnIds": ["trn_01J..."],
  "compactedThroughTurnId": "trn_01H...",
  "summaryText": "作者正在推进第三次转折……",
  "memoryHash": "64-hex"
}
```

规则：

- Memory 必须引用可见 Turn 或 Canon；不能保存无法追溯的“AI 记得”。
- Canon 与 Memory 冲突时 Canon 永远优先，Memory 标记待重建。
- 每个终态 Turn 由 Host 根据协议事件生成并应用 `SessionMemoryDeltaV2`，不调用 Provider。
- Delta 只能增加/关闭带来源的 confirmed decision、unresolved question、active entity、recent Turn 和写入结果；不能凭模型摘要创造事实。
- `currentGoal` 只在用户明确表达目标或用户确认目标变更时更新；Host 不从模糊对话中猜测。
- 后续 Provider 请求实际注入 Memory，不只把它保存到文件。
- Memory 丢失时可由项目内 Turn 历史和 Canon 重建。
- 聊天历史不自动过期；用户可以按 Session 清空。清空 Memory/History 不删除 Canon。

`SessionMemoryDeltaV2` 至少包含：

- base Memory Hash 和目标 version；
- 新增/关闭的 decision 与 question，各自绑定 Turn/Canon Source；
- 新增/移除的 active entity；
- recent Turn 窗口变更；
- Canon Commit/Undo Receipt 引用；
- Delta Hash。

Base Hash 不匹配时拒绝应用并从终态 Turn 重建，不做模糊合并。

### 12.3 按阈值 Memory Compaction

`memory_compaction` 只在以下任一条件达到配置阈值时调用：

- recent visible Turn 数量过多；
- recent message UTF-8 总字节超过 Context 预算；
- 用户明确结束一个剧情阶段并开始新目标；
- Context Planner 证明当前摘要不足以在预算内继续。

Compactor 只可以：

- 把较老的可见 Turn 压缩为新的 `summaryText`；
- 给摘要中的每个决定或未决问题附 Turn/Canon 引用；
- 推进 `compactedThroughTurnId`；
- 保留最近 Turn 原文窗口。

Compactor 不可以改变 confirmed decision、删除 unresolved question、创建 Canon 事实或修改完整 Chat History。输出 Schema/来源校验失败时保持上一个 Memory，不阻断已完成 Turn；稍后可以重试。

Compaction 只能在下一 Turn 的 Context Preflight 或用户显式发起“整理记忆”时执行；已经完成的 Turn 之后不得在后台继续调用 Provider。这样可以保证项目关闭、Provider 切换和预算记账都有明确边界。

## 13. UI 事件与三栏映射

### 左栏

- 新建/打开/最近项目；
- 项目文件树与搜索；
- Provider 选择、连接和 Key 状态；
- Session 选择或清空历史。

### 中栏

- 只显示用户消息、AI 可见回复和输入框；
- 可以显示纯文本的失败/取消回复，但不显示 Skill 卡、Tool 卡、Context 卡、Diff 或确认卡；
- 长 Draft 以简短对话说明加右栏 Artifact 引用呈现，不把整章强制塞入聊天气泡。
- 等待冲突或高风险确认时，中栏只禁用输入并显示简短状态；App 自动打开右栏对应面板，不把决议控件插入对话流。

### 右栏

- 当前文件编辑器；
- 已选 Skill 和实际加载的 Skill 资源；
- Context Plan、来源和 Context Marks；
- Tool/Composition 进度；
- Draft Artifact；
- Conflict Resolution；
- Diff、风险、确认、提交结果和 Undo。

Host 事件至少包括：

- `turn.state_changed`
- `skill.selected`
- `conversation.started`
- `conversation.chunk`
- `conversation.completed`
- `context.plan_ready`
- `context.source_added`
- `context.mark_changed`
- `composition.progress`
- `artifact.ready`
- `conflict.awaiting_resolution`
- `write.preview_ready`
- `write.committed`
- `turn.failed`

这些事件只携带 UI 所需的脱敏字段；事件日志不保存正文副本。

## 14. TurnBudgetV2 与规模校准

Revision 2 冻结“必须统计和强制哪些预算”，不在没有规模数据时冻结最终数值。预算分为三层：

1. 单 Action/单调用上限；
2. 整个 Turn 累计上限；
3. Provider 能力快照给出的更小实际上限。

最终有效值始终取三者最小值。

### 14.1 分阶段首轮规模测试候选值

以下数值是跨 GAR1～GAR3 的候选上限池，只作为合成工程测试护栏，不是最终产品常量。每个 `BudgetProfileV2` 只启用本阶段已经实现的字段；未激活能力的计数必须为 `0`，不能因为下表存在后续阶段候选值而获得调用权：

| 项目                              | 候选上限      |
| --------------------------------- | ------------- |
| 每 Turn Skill                     | 4             |
| Control Steps                     | 12            |
| 单 Control Action Context Queries | 8             |
| 每 Turn Context Queries           | 32            |
| 唯一 Canon Sources                | 48            |
| 单次 Provider 输入                | 96 KiB UTF-8  |
| 单个来源原文片段                  | 16 KiB UTF-8  |
| 可见 Conversation Message         | 32 KiB UTF-8  |
| 单个 Composition Segment          | 64 KiB UTF-8  |
| 单个 Draft Artifact               | 512 KiB UTF-8 |
| 修改文件                          | 8             |
| 写事务                            | 1             |
| Confirmed Conflict Items          | 16            |
| 活动 Context Marks                | 64            |
| **累计 Provider Calls**           | **16**        |
| **累计 Provider Input**           | **1.5 MiB**   |
| **累计 Provider Output**          | **1 MiB**     |
| **累计 Context Result 保留字节**  | **512 KiB**   |
| **累计 Composition Segments**     | **12**        |
| **累计结构化修复调用**            | **3**         |

`TurnBudgetV2` 还必须登记 `maxTurnElapsedMs` 和 Provider 返回时可用的 input/output token 计数；时间和 token 的最终默认值同样由规模测试决定。重试、结构修复、Conversation、Composition、Reconciliation 和 Memory Compaction 都计入累计 Provider Calls/Input/Output，不能使用独立计数器绕过。

GAR1 候选规模测试 Profile 的机器可读形态至少为：

```json
{
  "schemaVersion": 2,
  "scope": "gar1_read_only",
  "profileStatus": "scale_test_candidate",
  "enabledProviderModes": ["control", "conversation", "memory_compaction"],
  "enabledHostCapabilities": ["context_executor"],
  "maxProviderCalls": 16,
  "maxTotalProviderInputBytes": 1572864,
  "maxTotalProviderOutputBytes": 1048576,
  "maxAccumulatedContextResultBytes": 524288,
  "maxCompositionSegments": 0,
  "maxCanonWrites": 0,
  "maxStructuredRepairCalls": 3,
  "maxTurnElapsedMs": null,
  "maxTotalProviderInputTokens": null,
  "maxTotalProviderOutputTokens": null
}
```

`null` 表示规模测试必须给出最终值，不表示运行时无限制。受锁 Profile 发布前，测试环境使用 Harness 的有限超时与 Provider 能力上限；生产路径不得启用未冻结的 Profile。

Provider 输出不再使用统一 4096-token 上限：Control/Reconciliation 使用较小结构化预算；Conversation 和 Composition 根据 Provider 能力和当前剩余累计预算协商。章节超过单次输出能力时由 Composition Job 分段，但所有 Segment 仍受 Artifact、Segment 和 Turn 累计上限约束。

### 14.2 预算耗尽行为

- 启动任何 Provider 请求、Context Query 或 Segment 前先预扣预算；
- 预算不足时不启动该动作，不依赖请求完成后再拒绝；
- 未开始写事务时保持 Canon 零写入；
- 已生成的完整 Draft 可以作为未决 Artifact 展示，但不能绕过后续 Context/Conflict/Write 门；
- UI 显示耗尽的具体预算项，并允许用户缩小范围或在新 Turn 继续；
- 不能静默截断 must-read Canon、取消 Validator 或降低冲突检查。

### 14.3 分阶段规模校准 Gate

`BudgetProfileV2.scope` 只能是 `gar1_read_only / gar2_canon / gar3_long_form`。每个 Profile 只允许启用已经实现并通过相应 Gate 的 Provider 模式和 Host 能力：

- `gar1_read_only`：control、conversation、memory_compaction 与 Context Executor；composition/reconciliation、Mark、Conflict 和 Canon Write 固定 disabled；
- `gar2_canon`：在 GAR1 Profile 上增加 reconciliation、Canon Proposal、Mark、Conflict 与 Write Preflight/Commit 预算；
- `gar3_long_form`：增加 composition、Artifact/Segment、长篇连续性与完整 Storytelling 预算。

较早 Profile 不能为未实现能力预留“可调用但未测试”的非零预算。生产路径只能加载当前产品 Gate 已批准的受锁 Profile；候选 Profile、较晚 Profile或 Scope 不匹配时启动失败。

GAR1 使用不少于 120 章、50 万中文字符的确定性合成项目，至少包含 48 个带真值来源的查询，平均覆盖跨卷人物/关系、世界规则、剧情线程和最近章节四类。通过条件：

- 每个 must-read 真值来源命中率 100%，四类查询的目标 Canon 命中率 100%；
- `.novelgainker/agent/**`、未授权目录和项目外来源命中数为 0；
- Context Plan 不静默丢弃 must-read，所有截断均有原因；
- 在 Evidence 记录的参考硬件上，冷索引构建不超过 30 秒、峰值 RSS 不超过 512 MiB、热查询 p95 不超过 500 ms；
- 取消、重启、Hash 漂移、History 隔离和 Memory Compaction 场景全部保持来源、决定和未决问题不变量；
- DeepSeek/OpenAI Loopback 使用同一 Profile，并且无累计预算绕过。

GAR2 增加世界规则/剧情线程冲突、多 Skill、多文件 Canon Proposal、Mark 失效和 Commit/Undo 场景。GAR3 增加 6000～12000 字符章节生成、截断续写、连续性和 Ledger，并完成全能力规模门。

每个阶段的 Evidence 都记录 Provider Calls、累计 Input/Output、Context Result、Segment、修复、耗时、Token、相关来源命中和截断原因。受锁值必须覆盖该阶段通过场景的观测 p99 加至少 25% 安全余量，同时不得超过 Provider 能力和协议硬上限；任一客观条件未通过时不得冻结 Profile。任何后续调整都走独立 Gate。

## 15. 修复、失败与取消

### 15.1 结构化输出修复

- Control、Reconciliation、Memory Compaction 每次最多一次同 Provider 修复。
- 修复 Prompt 必须指出安全错误类别和精确允许 Schema，不包含原始敏感响应。
- 第二次失败返回阶段化错误，例如 `agent-control.repair-exhausted`，并保留用户输入、Skill/Context 状态和零写入证明。
- Composition 不进入 JSON 修复；截断、无效 UTF-8、结构缺失分别进入 resume、invalid 或用户可见失败。

### 15.2 稳定错误域

- `agent-session.*`
- `agent-control.*`
- `agent-context.*`
- `agent-composition.*`
- `agent-canon.*`
- `agent-conflict.*`
- `agent-write.*`
- `agent-history.*`

UI 显示用户可理解的原因和稳定代码。底层网络正文、Provider 原始输出和绝对路径不得进入项目历史。

### 15.3 取消

- Provider 响应头、正文流、退避、Context 读取、每个 Composition Segment、Reconciliation 和 Write Preflight 都检查同一取消令牌。
- 取消后不启动新工具、新 Segment、新 Conflict 分析或写事务。
- 已收到的部分 Draft 标为 `cancelled`，不能 Canon 化；用户可以查看后丢弃。
- 原子事务开始后只允许成功、回滚或 recovery_required。

## 16. V1 与旧 AM1/AM2 合并迁移

遵循 RB-Q008，不保留两套产品历史：

1. 冻结 V1 的新功能开发，只允许影响当前可运行基线的阻断修复。
2. 从现有实现复用 ProjectHandle、Provider/Vault、取消、Validator、Hash、Risk、事务、确认和 Undo 代码。
3. 用 V2 Session、Context、Control、Composition、Canon 和项目内 History 替换旧 Router、V1 Action Loop 和旧持久化模型。
4. 新 UI 只挂载一条 V2 产品路径，不提供 V1/V2 切换。
5. 把 V1 的路径逃逸、权限提升、Prompt Injection、取消、Hash、事务、Undo、敏感数据和双 Provider 负向测试逐项映射到 V2；V2 等价测试全部通过前不得删除旧覆盖。
6. V2 Gate 通过后移除旧 AM1/AM2 Story Packet、旧阶段状态、被替代的 V1 Contract/Prompt/E2E 和无调用旧后端。
7. 架构、Sprint Plan、Workspace Rules 和开发文档重写为一套当前设计，不保留 GA1/GA2 历史叙述。

删除动作必须在新的 Story Packet 中列出精确文件和调用图，并证明没有仍被 V2 复用的模块；本评审稿本身不执行删除。

现有 App State 中的 V1 Session/Turn 元数据不复制成新的项目 Chat History；启动 V2 前只允许读取它来安全终止真实活动/恢复状态，随后按迁移 Story 退役。Canon、Provider/Vault 选择和仍被复用的事务安全数据按各自当前契约迁移。

“不保留历史”指最终工作树、运行时入口和权威文档不保留旧 AM1/AM2 产品层；不执行破坏性的 Git 历史重写。

## 17. 端到端示例

用户说：

> 海禁不是皇帝下的命令，而是港务司伪造的。让林夜在下一章发现漏洞，但先不要知道主使。

协议流程：

1. Skill Selection 选择 setting、plot、storytelling。
2. Context Planner 优先读取活动 Mark，再检索海禁、林夜、第三次转折和最近章节。
3. 如果 `world/sea-ban.md` 写着“皇帝亲自颁布”，Host 生成 ConflictSet，右栏要求作者处理。
4. 用户选择“采用新设定”，Host 形成新的世界观候选和受影响剧情候选。
5. Provider 返回 `start_composition`，App 建立章节 Artifact Lease。
6. Composer 以原始 Markdown 生成下一章 Draft；Reconciler 证明林夜只发现漏洞、没有提前知道主使。
7. Host 生成 world/plot/chapter 的单个事务 Diff。因为包含已解决冲突和多文件改动，等待确认。
8. 用户确认后原子写入、生成 Undo、刷新 Context Index。
9. Provider 返回 `start_response`；Host 建立 Conversation Lease，独立 Conversation 流在中栏回复：“已按你的决定更新海禁来源，并完成下一章 Draft。”
10. Host 根据 Conflict Resolution、Commit Receipt 和可见消息确定性应用 SessionMemoryDelta，不额外调用 Provider。
11. 右栏展示三个文件、来源、Diff、Mark 和 Undo；完整可见对话及决定进入项目内 Turn History。

## 18. Revision 2 已批准决策

批准记录：产品负责人于 2026-08-30 明确回复“接受 AP-Q001～AP-Q010”。以下十项按 Revision 2 原文冻结。

### AP-Q001——协议分层

Revision 2：Conversation、Control、Content、Canon、Persistence 五层完全分离；Control 只返回 `start_response`，实际对话使用独立文本流。

### AP-Q002——Provider 调用模式

Revision 2：使用 control/conversation/composition/reconciliation/memory_compaction 五种模式；Host 每 Turn 增量更新 Memory，只有达到阈值才调用压缩。

### AP-Q003——Skill 与路径权

Revision 2：冻结 `SkillSelectionV2`；Provider 在 Host 按能力/授权形成的完整 eligible 集合内语义选择，Host 不得按关键词排除 Skill，并独占路径、风险和提交权。

### AP-Q004——项目内历史

Revision 2：接受 `.novelgainker/agent/`；专用 Reader、打开校验、隔离、同步分叉和 Draft 配额已纳入，普通 Canon Tool/Search/Publish 永远排除该目录。

### AP-Q005——中栏协议

Revision 2：中栏只消费 Conversation；等待决议时自动打开右栏并禁用中栏输入，不插入操作卡。

### AP-Q006——Context Mark 语义

Revision 2：Mark 仍是 Canon 书签；使用 fieldPath/headingPath、anchorHash 和 documentHash，避免无关文件修改导致错误 stale。

### AP-Q007——Mark 权限与作用域

Revision 2：AI 只能标记当前 Turn 实际读取的 Canon，默认创建 medium session Mark；project/high Mark 由用户固定，AI 不能删除用户 Mark。

### AP-Q008——冲突决议

Revision 2：保留五种决议；只有证据充分的 confirmed conflict 进入 ConflictSet，低置信度疑点先对话澄清；采用/合并冲突结果始终确认。

### AP-Q009——首版预算

Revision 2：冻结预算维度、预扣与耗尽行为；表中数值只是 50 万字合成工程的测试护栏，最终 Budget Profile 由规模证据和独立 Gate 冻结。

### AP-Q010——V1 处置

Revision 2：V2 单路径切换；先迁移全部安全回归，再移除旧 V1 协议、Story 和无调用后端。“不保留历史”不包含 Git 历史重写。

状态：上述 Revision 2 条文均 **已接受**。RB-Q006 由 AP-Q001～AP-Q005、AP-Q008～AP-Q010 关闭；RB-Q007 由 AP-Q006～AP-Q007 关闭。

## 19. Review Gate

本规格 Review Gate：**Revision 2 通过**（2026-08-30）；Revision 2.1 只关闭 AM-R1 Draft 1 发现的调用子契约、Host-only Context 和分阶段 Budget 实施歧义，不改变 AP-Q001～AP-Q010 的产品决定。

本次批准冻结 V2 产品与协议边界。AM-R1 已依 Revision 2.1 完成只读 GAR1 Slice；该技术完成不自动批准 GAR1，也不授权 AM-R2、AM-R3、Sprint 6、真实 Provider Live Smoke、Context Mark 写入、冲突决议或 Canon Write。

AM-R1 的完成证据位于 `target/qa/gar1/`。下一步且唯一允许动作是 GAR1 独立产品评审；只有后续显式产品决定与新的 Story Packet 才能授权下一阶段。
