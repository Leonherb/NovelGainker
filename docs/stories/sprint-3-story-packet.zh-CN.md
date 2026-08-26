# NovelGainker Sprint 3 Story Packet

> 状态：NG-301～NG-306 已获产品验收；G3 已通过并关闭 Sprint 3；Sprint 4 Story Packet 已创建，NG-401 尚未授权
> 日期：2026-08-24  
> BMAD 数据包：execution-packet  
> Sprint：3——安全提案循环  
> 范围：NG-301～NG-306  
> 团队：1 人，严格顺序实施  
> 上游计划：[sprint-plan.zh-CN.md](../sprint-plan.zh-CN.md)（已批准）  
> 上游架构：[architecture.zh-CN.md](../architecture.zh-CN.md)（已批准）  
> 前置 Gate：[sprint-2-story-packet.zh-CN.md](./sprint-2-story-packet.zh-CN.md)（G2 已通过并关闭）

## 1. Sprint 目标

在 Sprint 2 的安全 Provider 与离线编辑基础上，建立 NovelGainker 原生的单 Manager、Workflow Pack、Context Manifest、结构化 Proposal、Proposal Hash、Diff、独立确认和可恢复原子 Apply，并以 `novelgainker.character-create` 作为第一条真实 AI → Canon 纵向工作流。

Sprint 3 的核心不是“让聊天回答旁边多一个保存按钮”，而是证明 AI 生成与 Canon 写入之间存在不可绕过的确定性安全层：

- App 只运行自己内置、构建时校验的 Workflow Pack，不读取 Novelist、Codex Plugin Cache、`SKILL.md` 或用户自定义 Prompt 包；
- 一个确定性 Manager 持有 Workflow Run、上下文、Provider 快照、Proposal 和确认状态；模型不是 Manager，也不能直接调用写入工具；
- 发送前先展示 Context Manifest，用户能看到接收方、固定模型、文件、片段、来源 Hash 和包含理由；
- 项目文本始终被标记为不可信数据，不能通过正文或项目 `AGENTS.md` 授予权限；
- Provider 只返回严格的 `CharacterProposalV1` 数据，不返回可直接执行的路径、命令或文件操作；
- Rust 根据已批准模板确定性序列化 Markdown，并在独立确认前展示完整文件或 Diff；
- 确认绑定 Proposal Hash、Context Manifest Hash、目标源状态和固定 Provider；任一变化都会使确认失效；
- Apply 只能通过 Rust 持有的 Proposal 执行，前端不能提交任意 Markdown、目标路径或工具名冒充 AI Proposal；
- Canon 写入继续复用 Sprint 1 的 Project Writer、路径沙箱、Hash、恢复日志和原子文件语义；
- Prompt、Provider Response、Proposal 正文和 Context 片段只存在当前进程内存，不进入 SQLite、日志或证据；
- 无 Key、无网络、Vault 不可用或 Workflow 失败时，手动编辑、搜索、校验和保存继续可用。

## 2. Sprint 3 可演示结果

G3 Review 应能按以下顺序复现：

1. 启动 Windows 11 x64 内部测试构建，打开一个只含合成内容的 NovelGainker 项目；
2. 在中栏打开“创建人物”工作流，确认当前只注册已实现的 NovelGainker 原生工作流；
3. 输入一段简体中文人物简报，点击准备，不立即联网；
4. 查看 Context Manifest，确认接收方 Provider、固定模型、`project.md`、项目 `AGENTS.md`、来源 Hash、片段和包含理由清晰可见；
5. 确认未保存编辑器文本、搜索查询、其他人物、隐藏目录、`published/` 和项目外文件没有被自动加入；
6. 明确点击“确认发送并生成”，Manager 才通过当前固定 Provider 发起请求；
7. 在生成过程中看到工作流 ID、Provider、模型、状态和停止入口，但不把未校验的部分 JSON 当作可应用内容；
8. Provider 返回后，Rust 严格解析 `CharacterProposalV1`，并确定性生成人物 Markdown 和安全目标路径；
9. 查看完整新文件预览、目标相对路径、结构诊断、Proposal Hash 与“尚未写入”状态；
10. 在未确认时关闭或放弃 Proposal，确认项目文件写入数为 0；
11. 重新生成 Proposal，明确点击独立的“确认写入”，成功创建 `characters/` 下的新 Markdown；
12. 确认文件满足 Frontmatter、`writingLanguage: zh-CN` 和人物模板，文件树刷新并可手动继续编辑；
13. 制造同名冲突、过期 Context Hash、目标在预览后出现、第二 Writer、恢复阻断和校验失败，确认均不会覆盖或半写 Canon；
14. 在 generating 阶段停止一次，在 applying 阶段模拟中断一次，确认前者不写文件，后者只会完成、回滚或进入 `recovery_required`；
15. 分别用 DeepSeek/OpenAI Loopback Contract 完成同一人物工作流，确认输出进入同一 Schema，且失败不会自动切换 Provider；
16. 重启应用，确认未完成的 Prompt/Response/Proposal 正文不会恢复或泄漏；只显示脱敏的 Run 恢复状态，并允许用户明确终止或重新开始；
17. 完全断网且无 Key 时，确认 Workflow 生成禁用，但右栏手动编辑、保存、搜索和校验继续可用；
18. 运行统一质量入口和 Sprint 3 E2E，满足 G3 Gate。

## 3. 范围边界

### 3.1 本 Sprint 包含

- NovelGainker 原生 Workflow Pack v1 的资源布局、Schema、构建时注册表与内容 Hash；
- 第一条可执行工作流 `novelgainker.character-create`；
- 单一、App 管理的 Workflow Manager 和显式状态机；
- App State v2 → v3 的脱敏 Workflow Run 元数据 Migration（取决于 S3-Q002）；
- Context Manifest、来源 Hash、包含理由、信任标签和发送前确认；
- 固定顺序 Prompt Compiler 与项目文本不可信边界；
- Provider Core 的固定结构化输出请求与严格本地解析；
- 最多一次同 Provider 结构修复（取决于 S3-Q003）；
- `CharacterProposalV1`、确定性 Markdown Serializer 和安全文件名策略；
- 单文件 `create`/`replace` Proposal Operation 基础能力；
- Proposal Hash、目标源状态、完整预览、逐行 Diff 和独立确认；
- 复用 Project Writer、路径沙箱、单 Writer、恢复日志与原子 Apply；
- Workflow 取消、启动恢复、歧义阻断和 Sprint 3 Mock E2E；
- Sprint 1 离线编辑与 Sprint 2 Provider/Key/禁止 Fallback 全量回归。

### 3.2 明确不做

- 不运行或兼容 Novelist Skill、Agent YAML、Codex Plugin、Python Sidecar 或第三方工作流包；
- 不允许用户导入、编辑或热加载 Workflow Pack；
- 不注册尚未实现的 setting/material/plot/storytelling/review/visual/publish 工作流；
- 不实现自由文本自动路由、模型 Router 或低置信度路由；Sprint 3 只从明确 UI 动作进入人物创建；
- 不实现 Multi-agent、Agent Handoff、并行 Workflow Run 或多个 Manager；
- 不实现章节写作、连续性检查、Story Ledger 更新、EPUB 或图片导入；
- 不自动读取当前编辑器文本、未保存草稿、搜索结果、其他人物、world、plot、chapters 或任意项目文件；
- 不允许模型返回可执行工具名、绝对路径、任意相对路径、Shell、URL、Provider 或 Model；
- 不允许前端提交 Proposal 正文、目标路径或新文件内容给 Apply Command；
- 不允许未确认写入、批量 Apply、自动 Apply、删除、移动或重命名 Canon；
- 不持久化 Prompt、Provider Response、Context 片段、Proposal 正文、Diff 正文或未保存编辑器内容；
- 不自动跨 Provider Fallback，不在结构修复时更换 Provider；
- 不新增 Provider、自定义 Endpoint、任意 Model ID、离线 AI、云同步、Android 或 Git 产品能力；
- 不提前实施 Sprint 4～7。

## 4. 关键契约

### 4.1 Workflow Pack v1 与注册表

Sprint 3 建议建立以下 App 自有资源结构：

~~~text
resources/workflows/v1/
  pack.json
  character-create/
    workflow.md
    input.schema.json
    output.schema.json
    character.template.md
~~~

约束：

- 资源名称不能使用 `SKILL.md`、`openai.yaml` 或 Novelist 命名空间；
- `pack.json` 固定 `schemaVersion`、`packVersion`、Workflow ID、版本、模式、输入/输出 Schema、内部工具权限和确认点；
- Sprint 3 只把 `novelgainker.character-create` 注册为可执行工作流；未实现工作流不以占位状态暴露给前端；
- `workflow.md` 是 NovelGainker 自有生成说明，不授予工具权限；真正权限来自 Rust 注册表；
- 构建阶段按排序后的相对资源路径与原始 UTF-8/LF 字节计算 SHA-256，并把校验后的资源编译进二进制；
- 缺文件、重复 ID、未知字段、错误版本、非法工具、未声明确认点、非 UTF-8、CRLF、BOM 或 Hash 不一致必须使构建/测试失败；
- 运行时不遍历用户文件系统、不读取 Plugin Cache、不从网络更新 Pack；
- 前端只获得展示所需的 Workflow ID、中文名称、版本、模式和可用状态，不能获得任意资源路径或工具注册接口；
- Workflow Pack 内容变化必须有版本或构建期内容 Hash 变化，并保留 Golden Test。

建议首个注册项：

~~~json
{
  "id": "novelgainker.character-create",
  "version": 1,
  "mode": "interactive-write",
  "allowedTools": ["source.create_character"],
  "requiredConfirmations": ["send_context", "create_character_file"],
  "inputSchema": "CharacterBriefV1",
  "outputSchema": "CharacterProposalV1"
}
~~~

`source.create_character` 是 Rust Manager 内部能力，不直接注册成可由 WebView 或模型调用的 Tauri Command。

### 4.2 单 Manager 与状态机

第一版 Workflow Manager 是 Rust 内的确定性应用服务，不是常驻 AI Agent。它满足：

- 全 App 同一时间最多一个活动 Workflow Run；
- Workflow 生成与 Sprint 2 普通聊天共享“单活动 Provider Run”边界，不能并行占用两次模型请求；
- 前端只能通过固定 Command 请求合法状态转换，不能提交目标状态；
- Manager 捕获 Workflow ID/版本、Pack Hash、Project ID、当前 Provider、Preset 版本、模型和 Context Manifest Hash；
- Run 启动后不能切换 Provider、Pack、Workflow 或项目；
- 模型只返回数据；工具执行、确认检查与文件写入全部由 Manager 决定；
- 每个 Run 只有一个终态；终态后的事件、Provider Delta 或 Apply 请求全部拒绝。

Sprint 3 状态机：

~~~text
created
  -> routed
  -> awaiting_input
  -> context_ready
  -> generating
  -> proposal_ready
  -> awaiting_confirmation
  -> applying
  -> validating
  -> completed
  -> failed | cancelled | recovery_required
~~~

状态转换规则：

- `workflow_run_prepare` 验证明确 Workflow、项目 Writer 状态和人物简报后，依次进入 `created/routed/awaiting_input/context_ready`，不联网；
- `context_ready` 返回 Context Manifest；只有绑定 Manifest Hash 的明确确认才能进入 `generating`；
- Provider 原始流只在 Rust 内有界累积，未通过 Schema 前不进入 Proposal UI；
- 严格解析与本地语义校验通过后进入 `proposal_ready/awaiting_confirmation`；
- `workflow_proposal_apply` 只接受绑定 Proposal Hash 的确认，之后进入 `applying/validating`；
- `generating` 可取消；`applying` 不接受任意取消，只允许事务完成、回滚或恢复阻断；
- 普通聊天、切换项目、切换 Provider和关闭窗口必须与活动 Workflow Run 协调，不得留下后台请求或失去写入归属。

### 4.3 App State v3 与检查点

若 S3-Q002 接受建议，NG-302 把 App State 显式迁移到 Schema v3，只增加脱敏、可删除的 Workflow 元数据：

~~~text
workflow_runs
  run_id               TEXT PRIMARY KEY
  project_id           TEXT NOT NULL
  workflow_id          TEXT NOT NULL
  workflow_version     INTEGER NOT NULL
  pack_hash            TEXT NOT NULL
  state                TEXT NOT NULL
  provider_id          TEXT NULL
  preset_version       INTEGER NULL
  model                TEXT NULL
  manifest_hash        TEXT NULL
  proposal_hash        TEXT NULL
  target_relative_path TEXT NULL
  target_source_hash   TEXT NULL
  confirmed_at         INTEGER NULL
  updated_at           INTEGER NOT NULL
~~~

约束：

- 不保存人物简报、Prompt、Message、Context 片段、Provider Response、Proposal 字段、Markdown、Diff、诊断正文或 Key；
- 只保留当前活动/恢复 Run 和有界的近期终态元数据；精确数量由实现记录并测试；
- 删除 SQLite 不修改 Canon、Vault 或项目；只丢失可重建的 UI/Run 元数据；
- `generating/proposal_ready/awaiting_confirmation` 在进程重启后没有正文可恢复，必须标记 `recovery_required`，让用户明确放弃并重新生成；
- `applying/validating` 在启动时先与现有恢复日志和目标 Hash 对账，再归类为 `completed`、已回滚失败或 `recovery_required`；
- 启动恢复不自动联网、不自动重新生成、不自动 Apply；
- Migration 失败安全降级并禁用 Workflow，Sprint 1 手动编辑与 Sprint 2 普通对话按各自既有边界继续工作。

### 4.4 窄 Tauri Command 与事件

Sprint 3 建议新增七条窄 Command，总 Command 数从 22 增加到 29；Review Gate 若修改名称，必须同步 Manifest、Handler、Capability、前端类型客户端和契约测试：

| Command | 输入 | 输出 | 允许副作用 |
| --- | --- | --- | --- |
| `workflow_registry_load` | 无 | 可展示的已实现 Workflow 列表 | 只读内置注册表 |
| `workflow_run_status_load` | `sessionId` | 脱敏活动/恢复状态 | 只读 Manager/App State |
| `workflow_run_prepare` | `sessionId`、固定 `workflowId`、人物简报 | `PreparedRun` + Context Manifest | 读取批准项目源；不联网、不写 Canon |
| `workflow_run_generate` | `runId`、`manifestHash`、确认标记 | `RunAccepted` | 对已捕获 Provider 发起一次结构化请求 |
| `workflow_run_cancel` | `runId` | `CancelAccepted` | 取消 generating Provider Run |
| `workflow_proposal_apply` | `runId`、`proposalHash`、确认标记 | `ApplyAccepted/ApplyResult` | 执行已持有 Proposal 的单文件受控 Apply |
| `workflow_run_abandon` | `runId`、确认标记 | 脱敏终态 | 清除内存正文并终止 Run |

Command 约束：

- `workflow_run_prepare` 是唯一允许人物简报进入 Workflow IPC 的命令；简报有固定字符/字节上限，不进入 Debug、日志、AppError 或 SQLite；
- `workflow_run_generate` 不接受 Message、URL、Provider、Model、Key、路径或任意参数；
- `workflow_proposal_apply` 不接受 Markdown、Diff、文件路径、源 Hash、工具名或 File Operation；这些全部从 Rust 持有的 Proposal 读取；
- `workflow_run_cancel` 只能取消当前进程登记且处于 `generating` 的 Run；
- `workflow_run_abandon` 会清除 Prompt/Response/Proposal 内存，不能删除或回滚已完成的 Canon；
- 未登记 Workflow、未知 Run、错配 Session、过期 Hash、错误状态和重复确认在联网/写入前拒绝；
- 不新增通用文件、HTTP、Shell、Process、Opener、Clipboard 或 Plugin Capability。

Rust 使用固定事件名 `workflow-run-event`：

~~~text
WorkflowRunEvent
  runId
  sequence
  kind: state_changed | progress | proposal_ready | completed | failed | cancelled
  workflowId
  providerId?
  model?
  state
  proposalView?
  error?
~~~

事件不包含 Key、Prompt、Context 原文、Provider 原始 SSE、未校验 JSON、服务器 Body、绝对路径或 Rust Debug。`proposalView` 只能在严格 Schema、语义、路径和大小校验后出现。

### 4.5 Context Manifest 与 Prompt Compiler

Sprint 3 的 `character-create` 只允许以下输入源：

1. 用户在 Workflow 输入框明确提交的人物简报；
2. 项目根 `project.md`；
3. 项目根 `AGENTS.md`。

不得自动加入：

- 当前文件或未保存编辑器文本；
- 搜索查询与结果；
- `characters/` 中的既有人物；
- `world/`、`plot/`、`outlines/`、`materials/`、`chapters/`；
- `.novelgainker/`、`published/`、`.git/`、隐藏目录或项目外文件；
- Sprint 2 普通聊天历史。

`ContextManifestV1` 至少包含：

~~~text
schemaVersion
runId
workflowId / workflowVersion / packHash
providerId / presetVersion / model
briefHash / briefCharacterCount
sources[]:
  relativePath
  contentHash
  byteCount
  characterCount
  inclusionReason
  trustClass: project_instruction | project_canon
  boundedPreview
manifestHash
~~~

约束：

- Manifest UI 显示接收方、固定模型、每个来源和实际有界片段；
- `boundedPreview` 只存在当前前端/后端内存，不持久化；
- `AGENTS.md` 作为不可信的项目写作约束，不能授予工具或覆盖 App Policy；
- Context 文件在确认后发生 Hash 变化时，`manifestHash` 失效，必须重新准备和确认；
- Prompt 固定顺序遵循已批准架构：App 安全策略 → 项目 `AGENTS.md` → Workflow 说明 → 固定参考资料与模板约束 → `project.md` → 标准化对话状态（Sprint 3 固定为空）→ 用户简报 → `CharacterProposalV1` Schema 与完成契约；
- 每段项目内容使用不可伪造的边界标签和来源元数据；项目内出现“忽略系统规则”“调用工具”等文本只能当作小说数据；
- Prompt Compiler 返回 Provider Core 的内部请求对象，不把完整 Prompt 返回前端；
- 生成前 UI 明确提示这是一次 Provider 请求，可能产生 API 用量。

### 4.6 CharacterProposalV1 与确定性序列化

Provider 只允许返回严格结构化人物数据，不允许返回 Markdown 或路径。建议最小 Schema：

~~~json
{
  "schemaVersion": 1,
  "workflowId": "novelgainker.character-create",
  "character": {
    "displayName": "林遥",
    "role": "主角",
    "summary": "一句话人物定位",
    "traits": ["克制", "敏锐"],
    "goal": "当前核心目标",
    "conflict": "内外部主要冲突"
  },
  "rationale": "为什么该人物符合简报"
}
~~~

边界建议：

- 拒绝未知字段、空字符串、控制字符、过长文本、过多列表项、HTML/脚本和无法表示的 Unicode；
- `displayName`、role、summary、trait、goal、conflict、rationale 均有独立字符上限；
- Provider 不能指定 `id`、`relativePath`、Frontmatter、文件名、工具或操作类型；
- Rust 根据 `displayName` 和 S3-Q004 的文件名策略生成相对路径；
- Rust Serializer 使用固定字段顺序、UTF-8 无 BOM、LF 和末尾换行；
- Frontmatter 至少包含 `title`、`writingLanguage: zh-CN`、`kind: character`、`schemaVersion: 1`；
- 正文固定包含“角色定位”“核心特征”“目标”“冲突”四个章节；
- YAML 和 Markdown 特殊字符必须通过受控 Serializer 转义，不能字符串拼接注入 Frontmatter；
- 序列化后必须运行 `novelgainker-validation`；阻断诊断不会进入可确认 Proposal。

确定性目标示例：

~~~markdown
---
title: 林遥
writingLanguage: zh-CN
kind: character
schemaVersion: 1
---

# 林遥

## 角色定位

主角。一句话人物定位。

## 核心特征

- 克制
- 敏锐

## 目标

当前核心目标。

## 冲突

内外部主要冲突。
~~~

### 4.7 Proposal、Hash、Diff 与确认

Sprint 3 的通用 `CanonProposalV1` 只允许一个文件操作：

~~~text
proposalId
workflowId / workflowVersion / packHash
runId
providerId / presetVersion / model
manifestHash
operation:
  kind: create | replace
  relativePath
  expectedSourceHash: absent | sha256
  proposedContentHash
  proposedByteCount
validationSummary
warningCodes[]
proposalHash
~~~

约束：

- G3 引擎支持单文件 `create` 与 `replace`，不支持 delete/move/rename/multi-file；
- `character-create` 产品路径只生成 `create`；`replace` 由 NG-304 合成 Fixture 验证，为后续 Sprint 提供同一确认基础；
- 新文件显示完整内容；替换显示目标相对路径、源 Hash、完整新内容和逐行 Diff；
- Proposal Hash 使用版本化 Canonical JSON，覆盖 Workflow/Pack、Provider/Model、Manifest Hash、目标状态、拟写入字节 Hash、诊断和警告代码；
- 确认 UI 必须显示目标、操作类型、Provider/模型、是否新建/替换、结构校验、警告和 Proposal Hash 短标识；
- 确认只对当前 Proposal 生效；重新生成、改变 Context、目标变化、Provider 切换或 App 重启都会使其失效；
- 未确认、取消、关闭窗口、切换项目或解析失败时 Canon 写入数为 0；
- 确认记录可保存脱敏元数据与 Hash，但不得保存正文或 Diff。

### 4.8 Apply、路径与恢复

Apply 必须复用并扩展 Sprint 1 安全边界：

- 要求有效 ProjectSession、Writer Lease、无 `recovery_required` 和与 Run 绑定的 Project ID；
- 在 Apply 前重新读取所有 Context Source Hash 和目标状态；任一变化立即使 Proposal 过期；
- `create` 只允许 `characters/<安全文件名>.md`，使用 `create_new`、同目录临时文件、Flush/Sync 和可恢复提交；目标已存在时绝不覆盖或自动编号；
- `replace` 复用源 Hash、同目录临时文件、Flush/Sync、Windows 原子替换和恢复日志；
- Apply Command 从 Rust 内存取得序列化字节，不能相信前端回传内容；
- 写入后重新读取文件、核对 Hash 并执行严格 Validation；
- 失败只能归类为未写入、已回滚、已完整提交或 `recovery_required`；不存在半写后继续运行；
- Apply 成功后刷新文件树并可打开目标文件；不能自动保存或丢弃用户正在编辑的其他 dirty Buffer；
- 当前文件、目标文件或项目在预览后发生外部修改时必须阻断；
- 第二 Writer 只读，不能确认或执行 Apply；
- 恢复日志和 App State 不保存人物正文、Prompt、Response 或 Proposal 正文。

### 4.9 错误、隐私与 Fallback

稳定错误至少区分：

- `workflow.not_available`；
- `workflow.run_conflict`；
- `workflow.invalid_state`；
- `workflow.context_changed`；
- `workflow.context_too_large`；
- `workflow.output_invalid`；
- `workflow.repair_failed`；
- `workflow.proposal_expired`；
- `workflow.target_conflict`；
- `workflow.confirmation_required`；
- `workflow.writer_required`；
- `workflow.recovery_required`；
- `workflow.validation_failed`；
- `workflow.cancelled`；
- `workflow.internal_unavailable`。

所有错误使用稳定简体中文，不包含简报、Prompt、项目片段、模型原文、Markdown、Diff、Key、Authorization Header、绝对路径、用户名或 Rust/JavaScript Stack。

Provider 规则继续继承 Sprint 2：

- 当前 Provider、Preset 和模型在 Run 创建时捕获；
- 认证、模型、协议或结构失败不得自动调用另一 Provider；
- 用户取消后迟到事件丢弃；
- 若 S3-Q003 接受，一次结构修复仍使用同一 Provider 和同一 Run 接收方，并在 UI 标明额外请求；
- 未配置 Key、Vault 不可用或无网络时只禁用 AI Workflow，不影响离线编辑；
- G3 自动化使用双 Provider Loopback Contract，不把真实 Key 放进 CI、命令行、环境变量或证据。

## 5. 依赖与执行顺序

~~~mermaid
flowchart LR
    G2[G2 已通过] --> NG301[NG-301 Workflow Pack v1]
    NG301 --> NG302[NG-302 单 Manager 与检查点]
    NG301 --> NG303[NG-303 Prompt 与 Context Manifest]
    NG302 --> NG304[NG-304 Proposal、Diff 与 Apply]
    NG303 --> NG304
    NG304 --> NG305[NG-305 character-create]
    NG302 --> NG306[NG-306 取消、恢复与 G3 E2E]
    NG303 --> NG306
    NG304 --> NG306
    NG305 --> NG306
    NG306 --> G3[G3 Safe Apply]
~~~

单人团队按以下顺序执行：

1. NG-301；
2. NG-302；
3. NG-303；
4. NG-304；
5. NG-305；
6. NG-306；
7. G3 综合验收。

NG-302 与 NG-303 在依赖图中都只依赖 NG-301，但团队为 1 人，仍严格顺序实施，避免并行修改 Manager、App State、Provider 请求和 IPC 契约。每个 Story 单独授权、实施、验证和验收；产品负责人可另行一次性授权后续 Story，但不得并行写同一边界。

## 6. NG-301——实现 Workflow Pack v1 注册表

### Story

作为产品维护者，我需要一个构建时验证、运行时只读的 NovelGainker 原生工作流注册表，以便后续 AI 能力只从已批准流程进入，而不是加载插件或任意 Prompt。

### 用户价值

用户能看到明确、版本化且能力真实的“创建人物”入口；应用不会因为外部插件更新而改变行为。

### 实施范围

- 建立 `resources/workflows/v1/` 的 Pack Manifest、人物 Workflow、输入/输出 Schema 和模板；
- 新增 `novelgainker-workflows` Rust Crate 或等价清晰模块；
- 构建时验证 UTF-8/LF、Schema、ID、版本、模式、工具、确认点和资源引用；
- 计算稳定 Pack/Workflow 内容 Hash，并编译进二进制；
- 运行时提供只读 Registry 查询；
- 只注册 `novelgainker.character-create`；
- 登记 `workflow_registry_load` 窄 Command、Capability 和前端类型客户端；
- 中栏增加明确的人物工作流入口，但保持不可生成直到后续 Story；
- 静态禁止 Plugin Cache、`SKILL.md`、Agent YAML、Python 和运行时资源遍历。

### 验收条件

- [x] Pack 与 Workflow Schema 有显式版本。
- [x] 构建时能拒绝重复 ID、未知字段、非法工具和缺失确认点。
- [x] 内容 Hash 对资源字节变化敏感且排序确定。
- [x] 只有已实现人物工作流出现在注册表和 UI。
- [x] 模型和 WebView 都不能新增工具或 Workflow。
- [x] 运行时不读取 Plugin Cache、Novelist、`SKILL.md` 或网络资源。
- [x] Workflow 资源默认简体中文且无韩语专用分支。
- [x] `source.create_character` 只作为内部权限标识，不是通用 Tauri Command。
- [x] Command Manifest、Handler、Capability、前端客户端和契约测试同步。
- [x] Sprint 1/2 全量回归通过。

### 测试与证据

- Pack Manifest/Schema Golden Test；
- 资源 Hash 稳定性和改动敏感性测试；
- 重复 ID、未知字段、非法工具、路径越界和编码负面 Fixture；
- Registry 精确字段与前端 DTO 测试；
- 运行时无目录遍历、Plugin Cache、Python 和 `SKILL.md` 静态检查；
- 真实 Tauri 人物 Workflow 入口视觉证据。

### 完成定义

NG-301 只有在注册表完全由 App 构建产物决定、只暴露真实实现范围，并且任何外部资源都不能改变工具权限时才算完成。

## 7. NG-302——实现单 Manager 状态机和检查点

### Story

作为作者，我希望每次 AI 工作流都有明确、可取消、可恢复判断的状态，以便关闭窗口、请求失败或异常退出时不会留下未知写入。

### 用户价值

用户始终知道当前工作流在准备、生成、待确认、写入还是恢复状态，并且同一时间不会出现两个 AI 流程争夺项目。

### 实施范围

- 建立 Rust `WorkflowManager` 与固定状态转换；
- 全 App 最多一个活动 Workflow Run；
- 捕获 Workflow/Pack/Project/Provider/Preset/Model 快照；
- 建立 RunId、sequence、唯一终态和迟到事件丢弃；
- 与 Sprint 2 普通聊天共享单活动 Provider Run 约束；
- 按 S3-Q002 实现 App State v3 脱敏检查点或批准的替代方案；
- 登记 `workflow_run_status_load`、`workflow_run_cancel`、`workflow_run_abandon`；
- 启动时归类非终态 Run，不自动联网或写入；
- UI 显示稳定中文状态、停止、放弃和恢复说明；
- Provider、Manager 或 App State 故障不得影响手动编辑。

### 验收条件

- [x] 状态只能沿批准转换移动，前端不能提交目标状态。
- [x] 同一时间最多一个 Workflow Run，且不与普通聊天并行发请求。
- [x] Provider/项目/Workflow 快照在 Run 内不可更换。
- [x] 每个 Run 只有一个终态，迟到事件不复活 Run。
- [x] generating 可取消，非 generating 状态拒绝错误取消。
- [x] 放弃 Run 清除内存正文但不修改 Canon。
- [x] 检查点不包含 Prompt、Response、Context、Proposal 或 Markdown 正文。
- [x] 重启不自动重放请求或 Apply。
- [x] App State Migration/损坏/删除不阻塞离线项目。
- [x] Rust/IPC/UI 状态机契约一致。

### 测试与证据

- 状态转换 Property/表驱动测试；
- 单活动 Run 与普通聊天冲突测试；
- Provider/项目快照不可变测试；
- 取消、重复终态和迟到事件测试；
- App State v2 → v3 Migration、回滚、删除、损坏和敏感字段扫描；
- 进程重启恢复分类测试；
- 离线编辑故障隔离回归。

### 完成定义

NG-302 只有在 Manager 能确定性解释每个 Run 的状态，而且丢失内存正文时选择安全终止而不是猜测恢复时才算完成。

## 8. NG-303——实现 Prompt Compiler 与 Context Manifest

### Story

作为重视隐私的作者，我希望在发送前看到 AI 将接收的文件和片段，以便明确控制哪些项目内容离开电脑。

### 用户价值

用户不再依赖“应用大概只会读这些”的承诺，而能在每次人物生成前检查实际 Context。

### 实施范围

- 实现 `CharacterBriefV1` 输入验证和固定大小边界；
- 实现 Sprint 3 最小 Context Builder，只读 `project.md` 与项目 `AGENTS.md`；
- 生成 Context Manifest、来源 Hash、信任标签、包含理由和有界预览；
- 计算 Manifest Hash 并绑定 Project/Provider/Workflow/Brief；
- 实现固定顺序 Prompt Compiler；
- 项目文本使用不可信边界标签，禁止授予工具或改变输出 Schema；
- 登记 `workflow_run_prepare` 与 `workflow_run_generate`；
- 发送前展示 Context Manifest 与极少 API 用量说明；
- 文件在确认后变化时阻断并要求重新准备；
- 不自动读取编辑器、搜索、其他 Canon 或普通聊天历史。

### 验收条件

- [x] Prepare 不联网、不写项目。
- [x] Manifest 精确列出 Provider、模型、文件、Hash、片段和包含理由。
- [x] 只允许人物简报、`project.md` 与项目 `AGENTS.md`。
- [x] 未保存文本、搜索、其他目录和聊天历史不会进入 Context。
- [x] 项目 `AGENTS.md` 不能授予权限或覆盖 App Policy。
- [x] Prompt 顺序与架构一致并有 Golden Test。
- [x] 用户未确认 Manifest 时 Provider 请求数为 0。
- [x] Context Hash 变化会使确认失效。
- [x] 完整 Prompt、Context 片段和简报不进入日志、App State 或证据。
- [x] 无 Key/网络/Vault 时生成禁用，离线编辑继续可用。

### 测试与证据

- Context 选择白名单与排除矩阵；
- Prompt Injection 项目 Fixture；
- Manifest Hash 稳定性与源变化测试；
- Prompt Golden Test 与边界标签测试；
- 未确认零请求断言；
- Context/Brief/Prompt 泄漏扫描；
- 宽屏/窄屏 Context Manifest UI 测试。

### 完成定义

NG-303 只有在用户能在联网前检查准确 Context，且任何项目文字都无法扩大权限或隐藏附带来源时才算完成。

## 9. NG-304——实现 Proposal、Hash、Diff、确认和原子 Apply

### Story

作为作者，我希望 AI 的拟写入内容先形成可审查、不可被替换的 Proposal，以便只有我确认过的版本才能进入 Canon。

### 用户价值

用户可以查看完整新文件或修改 Diff，并确信确认后不会写入另一个目标或另一版内容。

### 实施范围

- 建立严格 `CanonProposalV1` 与单文件 create/replace Operation；
- 建立版本化 Canonical JSON 与 Proposal Hash；
- 绑定 Manifest、Provider、模型、Workflow、目标状态、内容 Hash、诊断和警告；
- 新文件展示完整预览，替换展示完整新内容和逐行 Diff；
- 独立确认 UI 显示目标、操作、来源和安全状态；
- 登记 `workflow_proposal_apply`；
- Apply 只读取 Rust 持有的 Proposal；
- create 使用不覆盖新建与恢复日志；replace 复用源 Hash 与原子替换；
- Apply 前重新检查 Context 和目标 Hash；
- 写后核对内容 Hash并严格校验；
- 成功后刷新文件树；失败保留编辑器文本和旧 Canon；
- 合成 Fixture 覆盖 create 与 replace，用户可见人物路径仍只 create。

### 验收条件

- [x] 未确认 Proposal 的 Canon 写入数为 0。
- [x] 前端不能向 Apply 提交正文、路径或工具名。
- [x] Proposal Hash 覆盖全部安全相关字段和拟写入字节 Hash。
- [x] 新文件完整预览、替换 Diff 与目标路径清晰可见。
- [x] 重新生成、Provider/Context/目标变化使旧确认失效。
- [x] create 目标已存在时不覆盖、不自动编号。
- [x] replace 源 Hash 过期时旧文件保持不变。
- [x] 第二 Writer、恢复阻断和 dirty 目标不能 Apply。
- [x] applying 只能完整提交、回滚或进入 recovery_required。
- [x] 确认/恢复/错误证据不含正文、Diff、Prompt 或 Response。
- [x] 写后 Hash 与 Validation 通过才进入 completed。
- [x] Sprint 1 手动保存路径继续独立可用。

### 测试与证据

- Canonical JSON/Proposal Hash Golden Test；
- 完整预览和逐行 Diff 组件测试；
- 未确认、重复确认和过期确认零写入断言；
- create collision、replace source-changed 与路径攻击测试；
- Writer Lock、恢复日志、故障注入和写后校验测试；
- 前端伪造正文/路径/工具 DTO 负面契约测试；
- 确认元数据敏感字段扫描。

### 完成定义

NG-304 只有在“用户看到的 Proposal”与“Rust 最终写入的字节”通过 Hash 和状态机严格绑定，任何中间变化都必须重新确认时才算完成。

## 10. NG-305——交付 character-create 纵向工作流

### Story

作为中文小说作者，我希望描述一个人物并获得结构化人物文件提案，以便审查后安全加入自己的小说项目。

### 用户价值

用户第一次能通过 AI 完成一条真实 NovelGainker 原生工作流，同时保留对发送内容、人物设定和 Canon 写入的控制。

### 实施范围

- 实现 `CharacterProposalV1` 严格 Schema 与语义上限；
- 扩展两家 Provider Adapter 的固定结构化输出路径；
- 按 S3-Q003 实现或禁用一次同 Provider 修复；
- 实现人物 Markdown 确定性 Serializer；
- 按 S3-Q004 生成安全中文文件名并处理冲突；
- 中栏交付人物简报、Context 确认、生成、停止、Proposal、完整预览和确认写入；
- 显示当前 Provider、固定模型、Workflow/Pack 版本和结构校验；
- Provider 输出不直接渲染 HTML、不执行链接或工具；
- 创建成功后刷新并打开人物文件，允许用户继续手动编辑；
- 两家 Loopback Provider 使用同一输入/输出 Schema 和本地 Validator；
- 确认资源与输出中不存在韩语命名规则或韩国语境默认分支。

### 验收条件

- [x] DeepSeek/OpenAI Mock 均能完成相同 `CharacterProposalV1` 契约。
- [x] Provider 不能指定文件路径、Frontmatter、工具或操作。
- [x] 人物字段有严格类型、数量、字符和大小上限。
- [x] Rust Serializer 输出固定 UTF-8/LF Markdown 与安全 Frontmatter。
- [x] 文件名符合批准策略，冲突不会覆盖。
- [x] Proposal 预览与最终磁盘字节 Hash 一致。
- [x] 用户取消或不确认时项目不变。
- [x] 创建成功后文件树刷新，右栏可继续手动编辑。
- [x] 当前 Provider 失败不会调用另一 Provider。
- [x] UI、模板、Schema 和测试默认简体中文且无韩语专用逻辑。
- [x] 普通聊天与离线编辑回归通过。
- [x] 真实 API 运行只由用户主动触发，不进入默认自动化。

### 测试与证据

- CharacterBrief/Proposal Schema Golden Test；
- 两家 Provider 结构化输出 Contract；
- 畸形/超限/未知字段/恶意 Markdown/HTML 输出测试；
- Serializer 转义、UTF-8/LF 和 Validation Golden Test；
- 中文文件名、Windows 保留名、尾随点/空格和冲突矩阵；
- 完整人物工作流 React/Tauri Mock E2E；
- 无 Fallback、无未确认写入、无韩语专用资源扫描；
- 真实 Tauri Proposal/确认/完成视觉证据。

### 完成定义

NG-305 只有在模型输出先成为受限数据、再由 Rust 生成可审查文件，并且独立确认后才创建 Canon 时才算完成。

## 11. NG-306——建立取消、恢复与 G3 E2E

### Story

作为产品负责人，我需要验证完整安全提案循环在取消、崩溃、冲突和两家 Provider 下都不会绕过确认或损坏 Canon，以便批准 G3 Safe Apply。

### 用户价值

用户获得可重复证明的 AI → Proposal → 确认 → Canon 路径，并能在故障后继续信任项目完整性。

### 实施范围

- 建立隔离临时项目与高熵合成隐私 Canary；
- 覆盖 Workflow 注册、Context、生成、Proposal、确认、Apply、校验和完成；
- 覆盖 generating 取消、迟到事件、关闭窗口和普通聊天冲突；
- 覆盖未确认关闭、重复确认、过期 Manifest/Proposal/Source Hash；
- 覆盖 create collision、第二 Writer、外部修改、恢复阻断和校验失败；
- 通过子进程故障注入覆盖 applying 中断与启动对账；
- 覆盖 App State 删除、损坏与 v2 → v3 Migration；
- 覆盖 DeepSeek/OpenAI Loopback 结构化输出和禁止 Fallback；
- 扫描 SQLite、项目外测试产物、日志、错误、DOM、截图元数据和 G3 证据；
- 断言 Prompt/Response/Proposal/Context Canary 不进入禁止位置；
- 将 Sprint 3 检查加入 `pnpm quality`；
- 生成真实 Tauri G3 窗口证据和脱敏安全报告。

### 安全约束

- E2E 只操作已验证系统临时根内的合成项目；
- 不使用真实用户项目或真实 Key；
- 默认质量门不访问外部网络；
- 失败证据不保留人物正文、简报、Prompt、Response、Diff、Key 或机器路径；
- 不通过缩小泄漏扫描、跳过恢复或降低 Capability 让测试通过；
- G3 E2E 不启动 Sprint 4 工作流。

### 验收条件

- [x] 两家 Mock Provider 完成相同人物工作流 Schema。
- [x] 未确认、取消、过期和冲突路径 Canon 写入数均为 0。
- [x] 成功路径只创建一个已确认人物文件。
- [x] applying 中断后只会完成、回滚或 recovery_required。
- [x] AI 绕过 Manager、未注册工具和伪造 Apply DTO 全部被拒绝。
- [x] Prompt/Response/Context/Proposal Canary 在禁止扫描目标中出现次数为 0。
- [x] 跨 Provider 自动请求数为 0。
- [x] App State 删除或损坏不影响已保存 Canon 和离线编辑。
- [x] Sprint 1 离线 E2E、Sprint 2 Provider/Key/聊天回归全部通过。
- [x] Debug/Release 构建与真实 Tauri G3 演示通过。
- [x] G3 证据不包含正文、Key、用户路径或真实手稿。
- [x] 完整统一质量入口通过。

### 测试与证据

- G3 Workflow/Provider/Apply 请求与写入计数矩阵；
- 取消、迟到事件和单 Run 报告；
- Manifest/Proposal/Source Hash 过期矩阵；
- Writer、外部修改、恢复与 Crash Fixture；
- App State Migration/删除/损坏报告；
- Canary 泄漏扫描报告；
- `target/g3-evidence/security-report.json`；
- `target/g3-evidence/tauri-g3-window.png`；
- 完整 `pnpm quality` 日志。

### 完成定义

NG-306 只有在自动化证据证明未确认写入为 0、跨 Provider 自动请求为 0、敏感正文泄漏为 0，并且成功路径的磁盘字节与已确认 Proposal 完全一致时才算完成。

## 12. 跨 Story 验收矩阵

| 能力 | NG-301 | NG-302 | NG-303 | NG-304 | NG-305 | NG-306 |
| --- | --- | --- | --- | --- | --- | --- |
| Workflow Pack/Registry | 主责 | 使用 | 使用 | 绑定 | 人物实现 | 回归 |
| 单 Manager/状态机 | 注册入口 | 主责 | 使用 | 写入状态 | 使用 | 汇总 |
| App State/恢复 | 无正文约束 | 主责 | Manifest 元数据 | Apply 对账 | 使用 | 汇总 |
| Context/Prompt | 工作流说明 | 状态 | 主责 | Hash 绑定 | 使用 | E2E |
| Proposal/Hash/Diff | Schema 声明 | 持有 | 输入绑定 | 主责 | 人物提案 | E2E |
| Canon Apply | 权限声明 | 状态 | 源 Hash | 主责 | create | 恢复主责 |
| Provider | 无请求 | 单 Run | 请求编译 | 身份绑定 | 双 Adapter | 禁止 Fallback |
| 隐私 | 资源边界 | 元数据 | Context 主责 | 确认脱敏 | 输出限制 | Canary 主责 |
| Local-first | 不依赖插件 | 故障隔离 | 无 Key 禁用 | 手动保存独立 | 使用 | 回归 |
| G3 演示 | 必需 | 必需 | 必需 | 必需 | 必需 | 汇总 |

## 13. Sprint 3 Gate G3

只有全部技术条件和产品批准满足，Sprint 3 才能结束：

- [x] NG-301～NG-306 均满足 Definition of Done。
- [x] Workflow Pack/Registry 构建时校验，运行时不读取 Plugin Cache、`SKILL.md` 或网络资源。
- [x] 第一版只有单 Manager 和单活动 Workflow Run。
- [x] Context Manifest 在联网前展示准确 Provider、模型、文件、片段和 Hash。
- [x] 未保存编辑器文本、搜索结果和未批准 Canon 不会自动进入 Prompt。
- [x] 项目文本和项目 `AGENTS.md` 无法授予工具或覆盖 App Policy。
- [x] 两家 Provider 的人物输出进入同一严格 `CharacterProposalV1`。
- [x] 模型不能指定路径、工具、Frontmatter 或文件操作。
- [x] 新文件完整预览、替换 Diff 和独立确认可用。
- [x] Proposal Hash、Manifest Hash 和目标源状态共同绑定确认。
- [x] 前端不能向 Apply 提交正文、路径或工具名。
- [x] 未确认、取消、过期、冲突和失败路径 Canon 写入数为 0。
- [x] Apply 只会完整提交、回滚或进入 `recovery_required`。
- [x] 成功人物文件通过严格 Validation，且磁盘 Hash 与确认 Proposal 一致。
- [x] Prompt、Response、Context、Proposal 和 Diff 正文不进入 App State、日志、错误或证据。
- [x] 当前 Provider 失败不会触发另一 Provider 请求。
- [x] 无 Key、无网络、Vault/Workflow 故障时离线编辑保持可用。
- [x] Sprint 1/2 全量回归、统一质量入口、Debug/Release Build 和真实 Tauri G3 演示通过。
- [x] 产品负责人批准进入 Sprint 4 Story Packet。

## 14. 待确认决策

### S3-Q001——Workflow Pack v1 首批注册与版本策略

发现的歧义：架构列出九类原生工作流，但 Sprint 3 只实现 character-create；若提前注册未实现 Workflow，UI 会展示无法完成的能力；若 Pack 内容在后续 Sprint 增长而项目版本不变，又需要明确版本语义。

建议：Sprint 3 只注册 `novelgainker.character-create`。`workflowPackVersion: 1.0.0` 表示项目兼容系列；每个 Workflow 自有整数版本，App 构建另带完整 Pack 内容 Hash。Sprint 4～6 只在对应实现与测试完成时增加注册项；兼容字段不变、已有 Workflow 内容变化时提升该 Workflow 版本并更新 Pack Hash，破坏项目格式时才提升项目的 Pack 兼容版本。

影响：接受后不会出现假入口，开发期仍能用 Pack Hash 精确追踪构建内容；拒绝并要求一次注册九类时，需要为未实现项定义禁用状态、迁移和 UI 解释，扩大 NG-301。

状态：已接受建议（2026-08-23）。

### S3-Q002——Workflow 检查点是否进入 App State v3

发现的歧义：Sprint Plan 要求“检查点”和重启恢复，但项目规则禁止 App State 保存 Prompt、Response 或手稿正文。只用内存最私密，但进程异常后无法解释 Apply 是否完成。

建议：采用 App State v3 的脱敏元数据检查点，只保存 Run/Workflow/Provider/状态/Hash/相对目标和时间，不保存任何正文。生成或待确认阶段重启后标记 `recovery_required`，用户放弃并重新生成；Apply 阶段通过现有恢复日志和磁盘 Hash 对账。删除 App State 不影响 Canon。

影响：接受后能在不保存内容的前提下解释中断状态，但需要 v2 → v3 Migration；拒绝则所有非 Apply Run 在进程退出后直接消失，只能依靠文件恢复日志判断写入事务。

状态：已接受建议（2026-08-23）。

### S3-Q003——畸形结构化输出是否允许一次自动修复

发现的歧义：架构的结构化输出降级允许一次同 Provider 修复，但 Sprint 2 普通对话没有结构化 Schema；Sprint 3 是第一次真实落地，会增加一次潜在 API 请求与少量费用。

建议：允许最多一次同 Provider 修复，只在本地严格解析失败时触发；使用同一 Run 捕获的 Provider/模型，固定修复说明只包含 Schema、稳定错误代码和有界的畸形输出，不重新加载额外项目文件，不跨 Provider。UI 显示“正在进行一次格式修复，可能产生额外用量”；第二次失败即停止，不生成 Proposal。

影响：接受可提高两家 Provider 的结构化输出成功率并验证后续 Chapter Proposal 所需机制；拒绝则第一次格式错误立即失败，用户需要手动重新生成。

状态：已接受建议（2026-08-23）。

### S3-Q004——中文人物文件名与冲突策略

发现的歧义：当前项目允许中文 Markdown 文件名，但人物 Schema 尚未定义稳定 ID；使用 UUID 安全但可读性差，自动编号又可能掩盖同名人物冲突。

建议：使用用户最终确认的人物名生成 `characters/<人物名>.md`，只允许安全 Unicode，去除控制字符并拒绝路径分隔符、Windows 保留名、尾随点/空格、隐藏名和超长名称；不做拼音转换。目标存在时阻断 Proposal，不覆盖、不自动加序号，用户修改人物名后重新生成/确认。Sprint 4 的 NG-405 再增加重复 ID 与同名关系处理。

影响：接受后文件树可读、行为可预测，但同名人物必须人工解决；改用 UUID 可避免文件名冲突，却需要在 UI 和 Frontmatter 引入独立 ID 与显示名映射。

状态：已接受建议（2026-08-23）。

### S3-Q005——G3 是否要求双 Provider 真实运行

发现的歧义：G3 要求 character-create 可分别用两家 Provider 完成，但 G2 已签署 Live Waiver，当前并不具备双 Provider 真实凭据条件。把双 Live 设为 G3 阻断项会重复卡在同一外部条件。

建议：G3 技术 Gate 使用 DeepSeek/OpenAI Loopback Contract 与真实 Tauri Mock E2E 证明同一 Schema、Prompt、取消和禁止 Fallback；若产品负责人本机已有可用 Key，可自愿执行真实人物生成，但不作为 G3 阻断条件。双 Provider Live Conformance 仍由 NG-703 在 Windows 内部测试候选前阻断。

影响：接受后 Sprint 3 可在无真实 Key/网络的可重复环境完成，同时保留发布前真实验证责任；拒绝则必须在 G3 前补齐两家 Key、额度和网络，并定义真实 Prompt/Response 不进入证据的人工验收流程。

状态：已接受建议（2026-08-23）。

## 15. 实施决策权限

以下属于 Story 内部低风险选择，可由实施者选择并记录：

- 新 Crate/模块内部命名，只要依赖方向清晰；
- Workflow 资源的 JSON Schema Validator 与 Hash Library；
- RunId/ProposalId 的具体 UUID 实现；
- Canonical JSON Library 与内部字段类型；
- Diff Library 和 UI 呈现样式，只要完整内容与目标可审查；
- 各人物字段、Context 预览和 Run 元数据保留数量的精确上限，只要有严格边界和测试；
- Provider 结构化输出映射的内部实现，只要保持固定 Schema 与同 Provider；
- G3 脱敏证据文件的内部排序与时间格式。

以下改变范围，必须停止并请求新决定：

- 注册或实现 character-create 之外的 Workflow；
- 读取 Novelist/Codex Plugin、`SKILL.md`、Agent YAML、Python 或用户导入工作流；
- 实现自由文本自动路由、模型 Router、Multi-agent 或并行 Run；
- 把 Prompt、Response、Context、Proposal、Diff 或人物正文持久化到 App State；
- 让模型或前端指定文件路径、工具、任意 Markdown 或 File Operation；
- 允许 Auto-apply、批量写入、删除、移动、重命名或未确认写入；
- 允许过期 Hash、第二 Writer、恢复阻断或校验失败时继续 Apply；
- 自动跨 Provider Fallback、切换模型或增加 Provider；
- 自动读取当前编辑器、搜索结果、其他人物、World、Plot、Chapter 或普通聊天；
- 新增云同步、账户、Git 管理、图片生成、Android、安装器或 Sprint 4+ 能力；
- 在 Review Gate 前开始 NG-301。

## 16. Story Packet Review Gate

开始 NG-301 前，需要产品负责人确认：

1. Sprint 3 的可演示结果和 G3 Exit Gate 符合预期；
2. NG-301～NG-306 的范围、顺序和验收条件可接受；
3. 接受或修改 S3-Q001～S3-Q005；
4. Workflow Pack 从第一天起是 NovelGainker 原生资源，不兼容或运行 Novelist Skill/Agent；
5. 第一版保持单 Manager、单 Workflow Run 和禁止自动 Fallback；
6. Context Manifest 在每次人物生成联网前明确展示并确认；
7. Prompt、Response、Proposal 和 Context 正文保持内存态，不进入 App State；
8. AI Apply 只能写 Rust 持有且经 Hash 绑定的 Proposal；
9. G3 只交付 character-create，其他 Workflow 留在 Sprint 4～6；
10. 团队继续按 1 人严格顺序执行；
11. 只授权 NG-301，完成并验收前不启动 NG-302 或 Sprint 4。

批准结论：通过（2026-08-23）。产品负责人接受 S3-Q001～S3-Q005，并只授权开始 NG-301；NG-302 尚未授权。

## 17. G2 交接记录

G2 于 2026-08-22 由产品负责人明确批准并关闭：

- NG-201～NG-207 已全部完成技术实现、统一质量门和产品验收；
- 产品负责人按 S2-Q004 明确记录 G2 Live Waiver，未执行 DeepSeek/OpenAI 真实 Live Smoke；
- G2 本地证据显示 Key Canary 泄漏、自动跨 Provider 请求和 AI Canon 写入均为 0；
- Sprint 1 离线编辑、Debug/Release 构建和真实 Tauri G2 演示通过；
- 该批准只授权创建本 Sprint 3 Story Packet，不等于批准 Packet 内容或授权 NG-301；
- Sprint 3 在当时的 S3-Q001～S3-Q005 与 Review Gate 完成前保持未启动。

## 18. Sprint 3 Review Gate 与 NG-301 技术验收记录

产品负责人于 2026-08-23 接受 S3-Q001～S3-Q005、批准 Story Packet Review Gate，并只授权实施 NG-301。实现结果：

- `resources/workflows/v1/` 只包含固定 Pack Manifest、`character-create` 说明、输入/输出 Schema 和人物模板五个资源文件；
- 新增 `novelgainker-workflows` Rust Crate，在构建期校验精确资源集、UTF-8/LF、路径、Schema、版本、模式、内部权限、确认点及禁止外部运行时依赖，并把稳定 Pack/Workflow Hash 和资源编译进二进制；
- 运行时注册表只返回 `novelgainker.character-create` 的展示 DTO；内部 `source.create_character` 不暴露为 Tauri Command，前端不能提供路径、工具或新 Workflow；
- 新增只读 `workflow_registry_load`，Command Manifest、Handler、Capability、Rust DTO、TypeScript 客户端和共享 IPC 契约已同步；总 Command 数由 22 增至 23；
- 中栏显示简体中文 Workflow Pack 卡片和唯一“创建人物”入口，入口明确禁用；NG-301 不实现 Manager、项目 Context、Provider 请求、Proposal、Diff、确认或 Canon 写入；
- Workflow 资源与运行时均不读取 Novelist、Codex Plugin Cache、`SKILL.md`、Python、用户目录或网络，也不包含韩语专用内容；
- Rust 校验器覆盖正常嵌入、Golden 内容、Hash 确定性/敏感性，以及重复 ID、未知字段、非法工具、缺失确认、路径越界、额外文件、非法 UTF-8 和 CRLF 负面用例；
- `pnpm quality` 已通过格式、Lint、TypeScript、前端测试、边界检查、Rust 测试、Sprint 1/2 E2E、Debug/Release 构建和真实 Tauri Smoke；
- `pnpm smoke:ng301` 已生成 `target/ng301-evidence/tauri-ng301-window.png`，并验证截图不是未渲染的空白 WebView；
- NG-301 已满足技术完成定义，并于 2026-08-23 获得产品负责人验收。

产品负责人随后回复“OK，下一步吧”，登记为 NG-301 产品验收通过并授权开始 NG-302；该授权不包含 NG-303 或后续 Story。

## 19. NG-302 技术验收记录

NG-302 已按单 Story 范围完成，未进入 Prompt Compiler、项目 Context、真实 Provider 生成、Proposal、Diff、确认 Apply 或 Canon 写入：

- `novelgainker-workflows::manager` 实现固定 13 态状态机、单活动 Run、RunId/sequence、快照不可变、唯一终态、迟到/重复事件拒绝、生成态取消、确认放弃、敏感内存清理和 20 条终态记录上限；生产实现不读取文件、不访问网络；
- App State 已从 Schema v2 事务迁移到 v3，保留原数据库文件名与四张既有表，只新增 `workflow_runs` 脱敏元数据表和单活动 Run 唯一索引；v1/v2 Migration、失败回滚、删除、损坏、快照不可变、保留上限和敏感列扫描已覆盖；
- 检查点只保存 Run/Project/Workflow/Pack/Provider/Preset/Model 标识、状态、sequence、相对目标路径、时间和 Hash；不保存简报、Prompt、消息、Context 正文、Provider Response、Proposal Markdown、Diff、API Key 或 Canon；
- 启动发现任何非终态 Run 时统一转为 `recovery_required` 并清空进程内敏感正文；不会自动联网、重放生成、恢复提案、Apply 或写入 Markdown；
- 与 Sprint 2 普通对话共用单活动 Provider 请求锁；工作流 `generating` 时普通聊天被拒绝，取消会先触发同 Run 的 CancellationToken，终态事件不能复活 Run；
- 只新增 `workflow_run_status_load`、`workflow_run_cancel`、`workflow_run_abandon` 三条 session-scoped 窄 IPC，总 Command 数由 23 增至 26；未登记 `prepare`、`generate`、`confirm` 或 `apply`；
- WebView 只接收不透明 `sessionId` 和严格状态 DTO；项目路径、文件正文、简报、上下文、提案和 Diff 均不进入该 IPC；
- 中栏新增简体中文运行状态卡：未打开项目、Manager 不可用、空闲、运行中、恢复、取消和终态均有稳定说明；放弃必须经过界面内二次确认；
- Manager/App State 故障只停用工作流；普通 AI 对话和 Markdown 手动编辑边界保持独立；确认放弃和恢复测试验证 `characters/` Canon 未变化；
- `pnpm quality` 已通过 Prettier、ESLint、TypeScript、20 个前端测试文件/99 项测试、26 命令边界、Rust Clippy/Workspace 测试、Sprint 1/2 E2E、Debug/Release 构建和真实 Tauri Smoke；G2 安全回归仍为 Key 泄漏 0、跨 Provider 自动请求 0、Canon 自动写入 0；
- `pnpm smoke:ng302` 已生成 `target/ng302-evidence/tauri-ng302-window.png`，SHA-256 为 `c4fc0e3b5e2f759aa92ab3f24b0c126f7b31ed1d2e6c0029cb61f86521a685fe`，截图已人工检查为完整渲染的三栏桌面界面和工作流状态提示。

技术结论：NG-302 满足完成定义。产品负责人随后回复“OK，下一步吧”，登记为 NG-302 产品验收通过并授权开始 NG-303。

该授权只覆盖 NG-303 的 Context Builder、Context Manifest、Prompt Compiler 与显式确认生成；不包含 NG-304 的 Proposal、Hash、Diff、确认写入或原子 Apply。

## 20. NG-303 技术验收记录

NG-303 已按单 Story 范围完成，未进入 Proposal、Diff、确认写入、Apply 或 Canon 自动修改：

- 新增严格 `CharacterBriefV1`：Schema v1、拒绝未知字段、换行标准化、控制字符检查、4000 字符/16 KiB 上限；前端与 Rust 都在 Provider 请求前验证；
- 纯 Rust Context Builder 只接受 `project.md` 与根目录 `AGENTS.md`，每个来源最多纳入 1600 字符/4 KiB；未保存编辑器、搜索结果、其他 Canon、隐藏目录、普通聊天和项目外文件均被排除；
- Context Manifest v1 显示固定 Provider/模型、来源相对路径、原始与纳入计数、来源 Hash、包含理由、信任标签、有界明文预览和截断状态；Manifest Hash 绑定 Run/Project/Workflow/Pack/Provider/Preset/Model/Brief/全部来源；
- Prompt Compiler 固定八段顺序，并为项目 `AGENTS.md`、`project.md` 和人物简报添加带内容 SHA-256 的不可信边界；Prompt Injection Fixture 与 Golden Hash `9c0ddc3e0f20f2b0f33986b1716de7c48fe8bf17155b05eb7cd157cfe95c2425` 已锁定顺序；
- `workflow_run_prepare` 完全离线、不写项目；`workflow_run_generate` 只接受 Run ID、Manifest Hash 与 `confirmed=true`，发送前重新检查来源 Hash、捕获 Provider、Vault 与连接状态；用户未确认时请求数为 0；
- Context-ready 期间禁止切换 Provider/Key/项目或并行普通聊天，但右栏 Markdown 手动编辑、保存、搜索和校验保持独立；被纳入来源变化会返回 `workflow.context_changed` 并阻止发送；
- 中栏“创建人物”入口已开放：先填写简报并离线准备，再逐项审查 Manifest，界面明确提示接收方、少量 API 用量和“当前不写 Markdown”，只有点击“确认发送并生成”才发起请求；预览使用 React 纯文本 `<pre>`，恶意标记不会变成脚本或链接；
- 完整简报、Prompt、Context 和 Provider Response 只存在可清除 Rust 内存，不进入 WebView 事件、App State、日志或证据；NG-304/NG-305 未授权，因此成功响应会立即清除并以脱敏 `workflow.output_pending` 终止，不形成 Proposal 或 Canon 写入；
- Command Manifest/Handler/Capability 已从 26 条同步到 28 条，只新增 `workflow_run_prepare` 与 `workflow_run_generate`；`proposal/diff/confirm/apply` 命令仍不存在；
- `pnpm quality` 已通过 Prettier、ESLint、TypeScript、20 个前端测试文件/102 项测试、28 条边界、Rust Clippy/Workspace 全测、Sprint 1/2 E2E、Debug/Release 构建和真实 Tauri Smoke；Canary Key 泄漏、自动跨 Provider 请求和 AI Canon 写入仍均为 0；
- `pnpm smoke:ng303` 已生成 `target/ng303-evidence/tauri-ng303-window.png`，SHA-256 为 `5b7242dc12b13bdd0cbab9dd7acd666ab9dd481090ff4be33c4da300d7dede62`；截图已人工检查为完整渲染的三栏 Release 桌面界面，Manifest 的宽屏/窄屏与恶意预览由组件测试和 CSS 单列规则覆盖。

技术结论：NG-303 满足完成定义。产品负责人于 2026-08-24 回复“批准 NG-303 验收，开始 NG-304”，登记为 NG-303 产品验收通过并只授权开始 NG-304。

## 21. NG-304 技术验收记录

NG-304 已按单 Story 范围完成，没有进入 NG-305 的人物专用输出解析、命名、Markdown 序列化或真实 Provider → Character Proposal 接线：

- 新增严格单文件 `CanonProposalV1`、create/replace Operation、1 MiB 上限、UTF-8/LF/无 BOM/尾换行及共享 Markdown 严格校验；版本化 Canonical JSON 和 Golden Proposal Hash 覆盖 Workflow/Pack/Run、Provider/Preset/Model、Manifest、目标状态、拟写入内容 Hash/字节数、校验与警告；
- Proposal 正文和逐行 Diff 只存在带脱敏 `Debug` 的 Rust 内存；Manager 以 Run/Project/Proposal Hash/状态精确绑定完整预览、独立确认、`applying → validating → completed`，旧 Hash、错误状态、重复确认或重启后缺失正文都会拒绝；
- Workflow 状态 IPC 升级到 Schema v3；中栏显示目标路径、create/replace、Provider/模型、来源状态、严格校验、警告、完整新内容、replace 逐行 Diff 和两步确认；成功 Apply 只刷新文件树，不覆盖右栏未保存编辑器文本；
- 只新增 `workflow_proposal_apply`，总 Command 数从 28 增至 29；前端请求只含 `runId`、`proposalHash` 和 `confirmed`，不能提交正文、路径、源 Hash、工具或 File Operation；
- Apply 在写入前重新检查 Context 与目标状态，并拒绝当前编辑目标、无 Writer 租约、恢复阻断、create collision 和过期 replace；create 使用原子不覆盖提交，replace 使用同目录临时文件和 Windows 原子替换，二者共用脱敏恢复日志、写后 Hash 和严格 Validation；
- 合成测试覆盖稳定 Golden Hash、全部安全字段敏感性、完整预览/逐行 Diff、路径攻击、非法源状态、未确认/伪造/重复确认零写入、Context 变化、当前编辑目标、只读/恢复会话、collision、replace source-changed、提交前回滚、提交后恢复与写后验证；前端伪造正文/路径/工具的额外 DTO 被严格键检查拒绝；
- `pnpm quality` 已通过 Prettier、ESLint、TypeScript、20 个前端测试文件/104 项测试、29 条边界、Rust Clippy/Workspace 全测、Sprint 1/2 E2E、Debug/Release 构建和真实 Tauri Smoke；默认流程未读取 API Key、未访问外网、未发送真实 Provider 请求；
- NG-304 通用 Proposal UI 在 NG-305 接入真实人物输出前没有生产 Proposal 入口，因此本 Story 以组件测试、共享 IPC Fixture、Rust 合成 create/replace 与故障注入作为可复现证据，不伪造真实 Provider 截图。

技术结论：NG-304 满足完成定义。产品负责人随后回复“批准 NG-304 验收，开始 NG-305”，登记为 NG-304 产品验收通过并只授权开始 NG-305。

## 22. NG-305 技术验收记录

NG-305 已按单 Story 范围完成，没有进入 NG-306 的取消/恢复 E2E 汇总或 G3：

- 新增严格 `CharacterProposalV1` 与嵌套人物数据 Schema：拒绝未知字段、固定版本/Workflow、约束字段类型、字符/字节/数组数量与唯一性，并拒绝控制字符、Bidi、HTML、路径、Frontmatter、工具或文件操作；
- DeepSeek 人物请求固定使用 JSON Output，OpenAI 人物请求固定使用 strict JSON Schema Structured Outputs；普通对话请求不携带结构化输出设置。两家 Loopback Provider 验证相同 Schema，OpenAI 发送前仅删除其结构化输出子集不支持的 `uniqueItems`，本地严格 Validator 仍执行唯一性校验；
- 首次输出不合格时只允许向同一捕获 Provider 发起一次有界修复请求；修复状态和额外用量在 UI 明示，第二次仍失败即以稳定脱敏错误终止，不切换 Provider、不保留原始响应；
- Rust 确定性 Serializer 生成 UTF-8、LF、尾换行与安全 Frontmatter/Markdown；中文显示名通过 Windows 保留名、分隔符、隐藏名、尾点/空格和长度矩阵后才映射为 `characters/<人物名>.md`，目标已存在时只报冲突且保持原字节；
- 有效人物数据只形成 create Proposal，不直接写项目。Proposal 继续沿用 NG-304 的完整预览、Hash 绑定、两步确认与可恢复原子 Apply；成功后刷新文件树并请求右栏打开新文件，若右栏有未保存编辑则仍先要求显式丢弃确认；
- 前端严格解析 `repairing`/`proposalReady` 事件，Provider 原始输出、Prompt、Context、Proposal/Diff 正文不进入事件、App State、日志、错误或证据；Manager 中旧的原始响应保留字段与废弃终止路径已删除；
- 测试覆盖未知/超限/恶意输出、Windows 文件名、Markdown 转义、单次有界修复、两家 Adapter 请求体、普通对话回归、同 Provider 无 Fallback、真实临时项目 Proposal 零写入/collision、确认 Apply 后刷新打开及脏编辑保护；
- `pnpm quality` 已通过 Prettier、ESLint、TypeScript、20 个前端测试文件/105 项测试、29 条边界、Rust Clippy/Workspace 全测、Sprint 1/2 E2E、Debug/Release 构建和桌面 Smoke；默认流程未读取 API Key、未访问外网、未发送真实 Provider 请求；
- 按 S3-Q005，真实 Provider 请求只允许产品负责人显式触发。本次技术验收未执行 Live Smoke，也未伪造真实 AI Proposal 截图；这不改变 Mock/本地 Validator/可恢复写入的自动化结论。

技术结论：NG-305 满足完成定义。产品负责人随后回复“批准 NG-305 验收，开始 NG-306”，登记为 NG-305 产品验收通过并只授权开始 NG-306。

## 23. NG-306 技术验收记录

NG-306 已按单 Story 范围完成，只汇总 Sprint 3 既有安全链路、故障矩阵与 G3 技术证据；没有批准 G3，也没有启动 Sprint 4：

- DeepSeek/OpenAI Loopback 均使用生产结构化输出 Schema、本地 Validator、确定性人物 Markdown Serializer、`CanonProposalV1`、Proposal Hash、显式确认与可恢复 Apply；每个成功 Run 只创建一个已确认文件，磁盘字节与 Proposal 完全一致，重复 create collision 不覆盖原文件；
- 既有 Manager/Tauri/前端矩阵覆盖 generating 取消、迟到终态、窗口关闭恢复、普通聊天锁、重复确认、过期 Manifest/Proposal/Source Hash、当前编辑目标、第二 Writer、外部修改、恢复阻断、校验失败与 App State v2 → v3/删除/损坏；未确认、取消、过期、冲突和失败路径 Canon 写入数均为 0；
- 新增真实子进程 Crash Fixture：在 `Commit` 前强制终止时重启对账回滚，在写后强制终止时重启对账完成；两条路径只产生完整完成或完整回滚，二次对账回到 `Ready`，歧义路径仍由既有测试锁定为 `recovery_required`；
- `pnpm e2e:sprint3` 在验证过的系统临时根中建立合成项目和五类高熵 Canary，扫描 SQLite 导出、项目外产物、日志、错误、DOM、截图元数据及 G3 证据；临时根在安全前缀检查后清理，不触碰真实用户项目；
- G3 安全报告记录双 Provider 工作流 2 次、确认 create 2 次且每 Run 最多 1 次，未确认/取消/过期/冲突写入 0、半写 0、跨 Provider 自动请求 0、Canary 泄漏 0；Applying 中断 2 次，覆盖 1 次提交恢复与 1 次回滚恢复；
- `pnpm quality` 已通过 Prettier、ESLint、TypeScript、20 个前端测试文件/105 项测试、29 条命令/权限边界、Rust Clippy/Workspace 全测（`app-core` 72 通过、4 个显式 Fixture/Live 测试忽略）、Sprint 1/2/3 E2E、Debug/Release 构建及真实 Tauri G3 窗口 Smoke；
- `target/g3-evidence/tauri-g3-window.png` 为 173038 字节，SHA-256 为 `64ea22107132faabee5a655ea6195494707c6763c2eb14c43e3905931b690f32`；Smoke 已验证窗口创建、PNG 签名、有效尺寸和非空渲染；
- 默认质量门未读取真实 API Key、未访问外网、未发送真实 Provider 请求，也没有在报告中保存 Prompt、Response、Context、Proposal、Diff、正文、用户路径或 Secret。

技术结论：NG-306 满足完成定义。产品负责人随后回复“批准 NG-306 验收，进入 G3 Review Gate”，登记为 NG-306 产品验收通过并正式打开 G3 Review Gate。

范围门已转移：G3 已关闭 Sprint 3，并只授权创建 Sprint 4 Story Packet；在 Sprint 4 Story Packet Review Gate 通过前，不得启动 NG-401。

## 24. G3 Review Gate 评审记录

产品负责人于 2026-08-24 批准 NG-306 验收并要求进入 G3 Review Gate。本次评审只决定 Sprint 3 是否满足 G3 Safe Apply，不自动扩大产品范围：

- NG-301～NG-306 均已完成技术实现、统一质量门与产品验收；
- G3 清单的 18 项技术条件全部通过；评审时唯一待决的产品批准现已完成并勾选；
- 双 Provider Loopback、确认写入、取消/过期/冲突零写入、Applying 中断恢复、App State 回归、离线编辑回归、Canary 扫描、Debug/Release Build 与真实 Tauri 窗口证据均已复核；
- `security-report.json` 与截图 SHA-256 一致；未确认/取消/过期/冲突写入、半写、跨 Provider 自动请求和 Canary 泄漏均为 0；
- S3-Q005 已批准真实双 Provider Live 不作为 G3 阻断项；真实 Live Conformance 保留给 NG-703，在 Windows 内部测试候选前阻断；
- 没有遗留的 G3 技术阻断项，也没有引入 Sprint 4、第三家 Provider、Android、安装器、Multi-agent 或 Novelist 运行时依赖。

评审结论：**通过**。产品负责人回复“批准 G3，关闭 Sprint 3，创建 Sprint 4 Story Packet”，因此 Sprint 3 已关闭，并只授权创建 Sprint 4 Story Packet；在新 Packet Review Gate 通过并获得单 Story 授权前，不得开始 NG-401 或其他 Sprint 4 实现。

当前决定状态：G3 已获产品批准，清单全部闭环；[Sprint 4 Story Packet](./sprint-4-story-packet.zh-CN.md) 已创建并停在 Review Gate，NG-401 尚未授权。
