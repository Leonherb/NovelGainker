# NovelGainker Sprint 4 Story Packet

> 状态：NG-401～NG-407 已获产品验收；G4 已由产品负责人批准，Sprint 4 已关闭
> 日期：2026-08-24
> BMAD 数据包：execution-packet
> Sprint：4——实体与剧情工作流
> 范围：NG-401～NG-407
> 团队：1 人，严格顺序实施
> 上游计划：[sprint-plan.zh-CN.md](../sprint-plan.zh-CN.md)（已批准）
> 上游架构：[architecture.zh-CN.md](../architecture.zh-CN.md)（已批准）
> 前置 Packet：[sprint-3-story-packet.zh-CN.md](./sprint-3-story-packet.zh-CN.md)（G3 已通过并关闭）

## 1. Sprint 目标

在 G3 已验证的单 Manager、Context Manifest、结构化 Proposal、Hash、独立确认和可恢复原子 Apply 之上，补齐章节写作前所需的项目、设定、人物、素材和主剧情基础，使用户能从空目录建立结构完整、默认简体中文、与 Provider 无关的小说 Canon。

Sprint 4 不重新发明写入链路，而是把 G3 的安全循环扩展成一致的实体工作流基础：

- `project-create` 继续是确定性、离线的项目 Scaffold 流程，不为创建空项目调用 AI；
- `setting-create`、`material-create`、`plot-create` 和增强后的 `character-create` 使用各自严格 Schema，但共用同一 Manager、Manifest、Proposal、确认和 Apply；
- 模型只返回领域数据，不能决定路径、ID、Frontmatter、文件操作或是否写入；
- Rust 分配实体 ID、确定目标路径并序列化 Markdown，Provider 切换不能改变磁盘格式；
- 每次联网前都显示精确 Context Manifest；默认不读取整个项目、未保存编辑器、搜索结果、聊天历史或章节；
- 所有新文件与替换都必须单文件预览、Hash 绑定和独立确认；不支持自动覆盖、自动编号、批量写入、删除、移动或重命名；
- Workflow Pack 只登记已经实现并通过测试的 App 自有工作流，不加载 Novelist、Plugin Cache、`SKILL.md`、Agent YAML 或网络资源；
- 明确按钮和确定性命令直接路由，只有含糊自由对话才允许当前 Provider 做无工具意图分类；
- 无 Key、无网络或 Workflow 故障时，项目创建、打开、Markdown 手动编辑、保存、搜索和校验仍可用。

## 2. Sprint 4 可演示结果

G4 Review 应能按以下顺序在合成项目中复现：

1. 启动 Windows 11 x64 内部测试构建，在没有 API Key、没有网络时打开“创建项目”；
2. 选择一个受控测试目录，填写项目标题及可选作者、类型和一句话前提；
3. 查看将创建的全部目录、`project.md`、`AGENTS.md`、故事账本和 `.novelgainker/project.json` 摘要；
4. 在确认前验证磁盘写入数为 0；确认后一次性创建完整 Scaffold，并打开项目；
5. 对已有计划条目、错误类型、符号链接、预览后目录变化和重复创建执行，确认均不覆盖且可解释；
6. 打开工作流区，确认只展示已经实现的项目、设定、素材、剧情和人物入口；
7. 点击“创建设定”，输入简体中文设定简报并显式选择允许发送的已有 `world/` 来源；
8. 审查 Provider、固定模型、来源路径、片段、Hash、包含理由和不可信标签后再确认发送；
9. 生成 `SettingProposalV1`，查看 `world/<设定名>.md` 的完整预览，确认后创建；
10. 制造同名、过期来源、目标后来出现、第二 Writer 和恢复阻断，确认均为零写入；
11. 点击“创建素材”，选择主题、线索、物件或场景种子，生成 `MaterialProposalV1`；
12. 在“是否成为 Canon”处拒绝一次，确认素材正文不进入项目、App State、日志或证据；
13. 重新生成并确认后创建 `materials/<素材名>.md`，且永不覆盖保留的 `materials/000.story-ledger.md`；
14. 点击“创建主剧情”，显式选择允许读取的人物、设定和素材来源；
15. 生成包含 Promise、Pressure、Turns、Reveals 和 Payoffs 的 `PlotProposalV1`；
16. 完整预览固定目标 `plot/main.md`，确认后写入；再次创建时必须明确选择替换并查看 Diff；
17. 打开“创建/完善人物”，创建带本地实体 ID 的新人，或选择一个已有 v1 人物进行显式升级；
18. 为人物补充关系、动机、秘密和剧情功能；关系只能引用用户已选择且存在的实体 ID；
19. 查看 v1 → v2 人物替换 Diff，确认旧 ID、同名、缺失关系目标和源 Hash 变化都会阻止 Apply；
20. 从每个明确按钮进入时验证 Provider Router 请求数为 0；
21. 输入明确中文命令，验证确定性规则只打开对应表单，不生成、不写入；
22. 输入含糊自由对话，确认只在当前 Provider 可用时发送本条文本进行 `RouterDecisionV1` 分类；
23. 对中低置信度、多意图、无 Key、取消和畸形 Router 输出，展示固定工作流选择器且零 Canon 写入；
24. 显式切换 DeepSeek/OpenAI Loopback，确认所有实体输出进入相同本地 Validator 和确定性 Serializer；
25. 验证失败不会自动切换 Provider，所有 Proposal/Response/Context Canary 在禁止位置出现次数为 0；
26. 运行统一质量入口与 Sprint 4 E2E，生成脱敏 G4 报告和真实 Tauri 窗口证据。

## 3. 范围边界

### 3.1 本 Sprint 包含

- `novelgainker.project-create` 的元数据向导、全量 Scaffold 预览、确认与冲突处理；
- `novelgainker.setting-create` 及 `SettingBriefV1`、`SettingProposalV1`、确定性设定 Markdown；
- `novelgainker.material-create` 及 `MaterialBriefV1`、`MaterialProposalV1`、成为 Canon 的显式决定；
- `novelgainker.plot-create` 及 `PlotBriefV1`、`PlotProposalV1`、固定主剧情文件策略；
- `novelgainker.character-create` v2：本地 ID、关系、动机、秘密、剧情功能和显式 v1 升级；
- 新实体的公共本地 ID、Frontmatter、文件名、链接与冲突规则；
- 每个 AI 工作流的显式 Context Source Picker、Manifest 和来源 Hash 重检；
- 兼容 `workflowPackVersion: 1.0.0` 的逐工作流版本提升和 Pack 内容 Hash 更新；
- 明确入口、确定性命令、受限 Provider Router 与低置信度选择器；
- 全部实体 Schema、Serializer、Golden、双 Provider Loopback、Canon Apply/Recovery 与 G4 E2E；
- Sprint 1 离线编辑、Sprint 2 Provider/Vault/聊天、Sprint 3 Safe Apply 全量回归。

### 3.2 明确不做

- 不实现章节生成、修订、连续性检查、Story Ledger 自动更新或 Storytelling Context Builder；
- 不实现大纲工作流、分析评审、视觉素材、图片生成 API、EPUB、发布或外部分发；
- 不实现任意自由文本自动写入、后台自动路由、自动执行多个工作流或批量实体创建；
- 不实现多文件事务 Proposal、关系反向同步、自动重命名、移动、删除、合并或 ID 重写；
- 不自动迁移所有旧人物；只允许用户选择单个 v1 人物并审查一个 replace Proposal；
- 不读取 `chapters/`、`outlines/`、`style/`、`published/`、隐藏目录、搜索结果、聊天历史或未保存编辑器作为默认 Context；
- 不允许模型、Router 或前端指定路径、实体 ID、Frontmatter、工具、Provider、模型或 File Operation；
- 不注册尚未实现的 storytelling、analytical-review、visual-asset 或 publish；
- 不运行或兼容 Novelist Skill/Agent、Codex Plugin、Python Sidecar 或用户导入工作流；
- 不增加第三家 Provider、任意 Model ID、自动 Fallback、Multi-agent、云同步、产品内 Git、Android、安装器、签名或自动更新；
- 不执行真实 Provider 自动化；若产品负责人自愿运行，只能走已批准的显式 Live 入口，且不作为默认 G4 证据。

## 4. 继承的安全与产品基线

Sprint 4 无条件继承 G0～G3 已批准边界：

- Markdown 与项目资源是唯一 Canon；SQLite、Provider 历史和内存 Proposal 都不是隐藏权威来源；
- 第一版保持单 Manager、单活动 Workflow Run、单 Provider 请求通道；
- Provider/Preset/模型在 Run 创建时捕获，Run 内不可切换；失败不得自动调用另一 Provider；
- Prepare 离线且零项目写入；Generate 必须在用户确认 Manifest 后才能联网；
- Prompt 固定顺序，项目 `AGENTS.md` 和所有项目文本只是不可信写作数据，不能授予权限；
- 结构化输出最多允许一次同 Provider 有界修复；第二次失败即终止且不形成 Proposal；
- Proposal 正文、Diff、Prompt、Context、Response 和实体秘密只存在当前进程内存，不进入 App State、日志、错误或证据；
- Apply IPC 只接受 Run ID、Proposal Hash 和显式确认，前端不能提交正文、路径、ID、工具或文件操作；
- Apply 前重检 Manifest 与目标源状态，写后重检磁盘 Hash 和严格 Validation；
- create collision 不覆盖、不自动编号；replace 只允许已预览且源 Hash 未变化的单一目标；
- Applying 中断只允许完整提交、完整回滚或 `recovery_required`；
- 无 Key、无网络、Vault/Manager/Workflow 故障不能阻塞项目打开和离线手动编辑；
- UI、默认写作内容、模板、错误和测试使用简体中文，不存在韩语命名或韩国语境专用分支。

## 5. Sprint 4 权威契约

### 5.1 Workflow Pack 与版本

Sprint 4 继续使用项目兼容系列 `workflowPackVersion: 1.0.0`，不把新增内置工作流误报为项目格式破坏：

- 每个 Story 只能在实现、Schema、模板和测试同时完成后把对应 Workflow 加入 Pack；
- `project-create`、`setting-create`、`material-create`、`plot-create` 首次登记为 Workflow version 1；
- `character-create` 从 version 1 提升到 version 2，v1 资源不被热加载或远程替换；
- Pack Manifest、每个资源字节和排序共同形成新的内容 Hash；
- 构建期拒绝额外文件、缺失资源、重复 ID、未知字段、非法模式、非法工具和缺失确认点；
- 运行时 Registry 只返回展示 DTO，不返回 Prompt、Schema 正文、工具实现或任意资源路径；
- 未完成的工作流不出现在可用入口，也不能被 Router 返回。

建议的最终 Sprint 4 注册集：

| Workflow | Version | Mode | 内部能力 | 确认点 |
| --- | ---: | --- | --- | --- |
| `novelgainker.project-create` | 1 | `deterministic-write` | `project.scan`、`project.create` | `create_project_scaffold` |
| `novelgainker.setting-create` | 1 | `interactive-write` | `source.create_setting` | `send_context`、`create_setting_file` |
| `novelgainker.material-create` | 1 | `interactive-write` | `source.create_material` | `send_context`、`promote_material_to_canon` |
| `novelgainker.plot-create` | 1 | `interactive-write` | `source.create_plot` | `send_context`、`create_or_replace_main_plot` |
| `novelgainker.character-create` | 2 | `interactive-write` | `source.create_character` | `send_context`、`create_or_replace_character_file` |

这些内部能力不是通用 Tauri Command。模型和前端都不能按字符串调用能力；Command Manifest 只登记窄职责、带类型且经审查的应用命令。

### 5.2 确定性 project-create 契约

`project-create` 不调用 Provider，也不占用 AI 生成通道。它复用 Sprint 1 已验证的目标预检、目录快照、事务创建与回滚，并增加可审查的元数据向导：

- `ProjectCreateInputV1` 只接受标题、可选作者显示名、可选类型和可选一句话前提；写作语言固定 `zh-CN`；
- 标题进入 `project.md` 的 Frontmatter 与一级标题；作者、类型和前提只在用户填写时进入 `project.md`；
- `.novelgainker/project.json` 继续只保存 UUID、Schema、Pack、默认 Provider 和语言等非敏感技术元数据；
- Project ID 由 Rust 在预览时生成并绑定确认 Hash，模型、WebView 和目录名不能提供 ID；
- 预览列出全部目录、全部文件、每个新文件的完整文本或安全摘要、元数据版本和冲突；
- 预览后目录快照变化会使确认失效；
- 允许目标目录含与计划无关的现有条目，但必须醒目标示；任一计划路径已存在、类型错误或为链接时整次创建阻断；
- 创建使用只新增事务，任一步失败只回滚本次创建条目，不删除用户已有内容；
- 重复执行必须报冲突，不覆盖、不合并、不自动导入。

### 5.3 公共实体 ID、Frontmatter 与路径

实体身份由 Rust Core 管理，Provider 只提出领域内容：

| 实体 | 本地 ID 前缀 | Schema | 新建目标 | 替换规则 |
| --- | --- | ---: | --- | --- |
| 设定 | `setting_` + UUID | 1 | `world/<安全名称>.md` | 只替换用户明确选择的同一文件，不改路径 |
| 素材 | `material_` + UUID | 1 | `materials/<安全名称>.md` | 只替换用户明确选择的同一文件，不触碰故事账本 |
| 主剧情 | `plot_` + UUID | 1 | `plot/main.md` | 已存在时必须走 replace Diff 并复用原 ID |
| 人物 | `character_` + UUID | 2 | `characters/<安全人物名>.md` | v2 复用原 ID；v1 显式升级时由 Rust 分配新 ID |

公共 Frontmatter 至少包含：

~~~yaml
id: <Rust 生成或复用的实体 ID>
title: <显示名称>
writingLanguage: zh-CN
kind: character | setting | material | plot
schemaVersion: <实体 Schema 版本>
~~~

规则：

- ID 使用小写前缀加标准 UUID，由 Rust 生成，进入 Proposal Hash 和最终磁盘 Hash；模型不能建议、复用或改写 ID；
- 文件名使用用户最终确认的显示名称，经现有安全 Unicode/Windows 保留名策略验证，不转拼音、不自动编号；
- `plot/main.md` 是 Sprint 4 唯一主剧情目标；支线、多 Plot 文件和大纲留给后续工作流；
- `materials/000.story-ledger.md` 是保留文件，material-create 永远不能创建、替换或引用它作为普通素材；
- replace 不执行重命名；即使用户修改显示名称，目标路径仍是用户选择的原文件，完整差异必须可见；
- 关系引用保存目标实体 ID；Rust 只接受当前项目索引中存在、类型匹配且用户在 Context Source Picker 明确选中的 ID；
- Rust 根据 ID 解析当前相对路径和显示名，模型不能返回 Markdown 链接或路径；
- v1 人物仍可读取和手动编辑；升级必须一次一个文件形成完整 replace Proposal，绝不后台批量迁移；
- 缺失、重复或冲突 ID 阻止 AI Apply，但不阻止右栏手动编辑和原子保存。

### 5.4 Context Source Picker 与 Manifest

所有 AI 实体工作流继续自动包含有界人物/实体简报、`project.md` 和根目录 `AGENTS.md`；除此以外只允许用户逐项选择：

| Workflow | 必需本地来源 | 可显式选择的 Canon | 默认排除 |
| --- | --- | --- | --- |
| setting-create | Brief、`project.md`、根 `AGENTS.md` | 最多 6 个现有 `world/` 文件 | 人物、素材、剧情、章节和编辑器 |
| material-create | Brief、`project.md`、根 `AGENTS.md` | 最多 6 个 `world/`、`characters/`、`plot/` 文件 | 故事账本、章节、搜索和聊天 |
| plot-create | Brief、`project.md`、根 `AGENTS.md`；replace 时本地绑定 `plot/main.md` | 最多 8 个 `characters/`、`world/`、普通 `materials/` 文件 | 故事账本、章节、大纲和未选择实体 |
| character-create v2 | Brief、`project.md`、根 `AGENTS.md`；增强时绑定目标人物 | 最多 6 个 `characters/`、`world/`、`plot/`、普通 `materials/` 文件 | 章节、搜索、聊天和未保存编辑器 |

公共限制：

- 每个来源最多纳入 1600 个 Unicode 字符和 4 KiB，超出时在 Manifest 中显示原始/纳入计数与截断状态；
- 单次 Manifest 的项目来源正文总计不超过 16 KiB；超出时阻止 Prepare 并要求减少选择，不静默扩大预算；
- Source Picker 只列受控 Markdown 索引中的相对路径、标题、类型、ID 和校验状态，不暴露绝对路径；
- 无效、重复 ID、严格校验失败、链接、隐藏目录、`published/` 和项目外路径不能被选择；
- Manifest Hash 绑定 Workflow/Pack/Run、Provider/Preset/Model、Brief、全部来源路径/ID/Hash/片段与截断状态；
- Generate 前重新读取并核对全部来源 Hash；任一变化返回稳定 `workflow.context_changed` 且请求数为 0；
- 替换目标的完整字节用于本地 Source Hash 与 Diff，不因被本地读取就自动全部发送给 Provider；实际发送范围必须在 Manifest 中逐项可见。

### 5.5 结构化输出与确定性 Serializer

模型输出只能满足对应领域 Schema，不能包含 ID、路径、Frontmatter、Markdown、HTML、工具或 File Operation：

- `SettingProposalV1`：显示名称、类型（世界/地点/规则）、摘要、氛围、规则、限制和剧情钩子；
- `MaterialProposalV1`：显示名称、类型（主题/线索/物件/场景种子）、摘要、内容、预期用途和成为 Canon 的理由；
- `PlotProposalV1`：标题、Promise、Pressure、有序 Turns、有序 Reveals、有序 Payoffs 和未决问题；
- `CharacterProposalV2`：保留 v1 的人物名、角色、摘要、特征、目标和冲突，并增加动机、秘密、剧情功能与关系；
- `RouterDecisionV1`：只能包含一个已注册 Workflow ID 和置信度，不包含工具、参数、项目内容或下一动作。

每个 Schema 都必须拒绝未知字段，并为字符串、数组、嵌套层级、总字节数、控制字符、Bidi、HTML、路径样式文本和重复项设置严格上限。DeepSeek/OpenAI 请求使用各自固定结构化输出能力，最终均进入同一 Rust Validator；Provider 特定 Schema 子集不能削弱本地约束。

Serializer 必须：

- 输出 UTF-8、LF、无 BOM、单个尾换行；
- 生成固定字段顺序、固定标题层级和安全 YAML；
- 转义 Markdown 元字符和 Frontmatter 特殊值；
- 把 Rust 分配的 ID、经验证的关系引用和本地目标状态加入 `CanonProposalV1`；
- 在相同输入、ID 和版本下生成逐字节相同结果；
- 先执行领域 Validation，再形成 Proposal；校验失败时磁盘写入数为 0。

### 5.6 路由契约

路由只决定“打开哪个工作流输入界面”，不自动 Prepare、Generate、确认或 Apply：

1. 用户点击明确按钮时直接打开对应表单，Provider 请求数为 0；
2. 明确中文命令通过固定本地规则路由，Provider 请求数为 0；
3. 含糊自由对话由 UI 明示“将仅发送本条文本进行意图识别”，再调用当前选定 Provider；
4. Router Prompt 不读取项目、App State、聊天历史、搜索、编辑器或 Canon，只含固定允许的 Workflow ID、固定规则和本条用户文本；
5. `RouterDecisionV1` 高置信度结果只打开建议表单；中低置信度、多意图、未知或畸形结果展示固定选择器；
6. Router 无工具、无文件、无 Manager 写权限，不得触发另一个 Provider、结构修复之外的重试或任何 Canon 写入；
7. 无 Key、无网络或 Vault 不可用时跳过 Provider 分类并直接展示离线工作流选择器。

### 5.7 隐私、错误和证据

新增稳定错误码至少覆盖：

- `workflow.source_not_allowed`；
- `workflow.source_limit_exceeded`；
- `workflow.entity_id_missing`；
- `workflow.entity_id_duplicate`；
- `workflow.entity_reference_invalid`；
- `workflow.reserved_target`；
- `workflow.legacy_upgrade_required`；
- `workflow.router_unavailable`；
- `workflow.router_ambiguous`；
- `project.metadata_invalid`；
- `project.scaffold_changed`。

错误、日志、SQLite、DOM 测试输出和 G4 证据不得包含 API Key、Authorization、Brief、Prompt、Response、Context 正文、Proposal/Diff、人物秘密、完整素材/剧情正文、绝对路径、用户名或 Stack。G4 使用高熵 Canary 扫描 SQLite、日志、错误、DOM、截图元数据、临时项目外产物和证据报告；真实合成 Canon 仅存在验证过的临时项目中，并在安全前缀核对后清理。

## 6. 依赖与执行顺序

~~~mermaid
flowchart LR
    G3[G3 已通过] --> NG401[NG-401 project-create]
    G3 --> NG402[NG-402 setting-create]
    G3 --> NG403[NG-403 material-create]
    G3 --> NG404[NG-404 plot-create]
    G3 --> NG405[NG-405 character-create v2]
    NG401 --> NG406[NG-406 确定性路由]
    NG402 --> NG406
    NG403 --> NG406
    NG404 --> NG406
    NG405 --> NG406
    NG406 --> NG407[NG-407 Entity Golden/E2E]
    NG407 --> G4[G4 Workflow Foundation]
~~~

Sprint Plan 中 NG-401～NG-404 直接依赖 G3，NG-405 继承 NG-305；但团队为 1 人，按以下顺序严格实施：

1. NG-401；
2. NG-402；
3. NG-403；
4. NG-404；
5. NG-405；
6. NG-406；
7. NG-407；
8. G4 综合验收。

每个 Story 默认单独授权、实施、验证和产品验收。产品负责人可以一次性授权后续 Story，但不得并行修改 Workflow Pack、Manager、Context、Proposal、实体 Schema 或 Command/Capability 同一边界。

## 7. NG-401——完善 project-create 工作流

### Story

作为新项目作者，我需要在不配置 AI 的情况下填写少量项目元数据、预览完整 Scaffold 并确认创建，以便从一个可审查且不会覆盖现有内容的本地项目开始写作。

### 用户价值

用户第一次打开应用就能创建真实可编辑的中文小说项目；目录冲突、计划文件和失败回滚都清楚可见，不会因重复点击或目录变化损坏已有文件。

### 实施范围

- 在 Workflow Pack 登记 `novelgainker.project-create` version 1、`deterministic-write` 模式和内部 `project.scan`/`project.create` 能力；
- 把现有创建项目 UI 整合成分步元数据向导：目标目录、标题、可选作者、类型、前提、Scaffold 预览、最终确认；
- 新增严格 `ProjectCreateInputV1`、标准化和本地字节上限；
- 让 `project.md` 模板确定性接收已确认元数据，同时保持默认简体中文；
- 在预览中展示全部 11 个目录、4 个初始文件、冲突、现有无关条目提醒和技术元数据摘要；
- 生成本地 Project ID 和 Preview Hash，绑定根目录快照、输入、模板版本与全部计划字节；
- 确认时重新扫描根目录和 Hash，变化即阻断；
- 复用现有只新增事务、回滚和项目打开流程；
- 创建成功后打开项目并保持现有最近项目、三栏和离线编辑行为；
- 更新 Pack 构建校验、Registry 展示、UI 卡片和类型化 Command 契约。

### 安全约束

- 全流程不访问 Provider、Vault 或网络，不创建 Workflow AI Run；
- WebView 不获得目标绝对路径以外的任意文件系统能力，仍通过不透明扫描候选与确认句柄工作；
- 用户输入只能影响 `project.md` 的受控字段，不能插入额外 Frontmatter、路径、模板或目录；
- 任一计划目标存在、类型错误、链接或根目录快照变化时整次阻断；
- 回滚只能删除本事务创建且身份仍匹配的条目，不能清理用户预先存在内容；
- `.novelgainker/project.json` 不保存作者、前提、API Key、机器绝对路径或 Provider Response；
- 不增加导入、合并、迁移旧项目或删除目录能力。

### 验收条件

- [x] 无 Key、无网络时可完成创建并立即手动编辑。
- [x] 预览前和最终确认前磁盘写入数均为 0。
- [x] 预览列出全部目录、文件、元数据版本和冲突。
- [x] 标题、可选作者、类型和前提只进入确定性 `project.md`。
- [x] Project ID 只由 Rust 生成并绑定 Preview Hash。
- [x] 预览后目录或输入变化会使旧确认失效。
- [x] 计划目标存在、类型错误和链接路径均阻断创建。
- [x] 中途失败回滚本次条目且保留用户原内容。
- [x] 重复执行不覆盖、不合并、不自动编号。
- [x] 成功 Scaffold 与项目 Schema、UTF-8/LF 和严格校验一致。
- [x] `project-create` 明确入口不会产生 Provider Router 请求。
- [x] Sprint 1 项目打开、导入、保存和恢复回归通过。

### 测试与证据

- `ProjectCreateInputV1` 正负 Schema/字符/字节矩阵；
- 模板 Golden 与 Preview Hash 敏感性测试；
- 空目录、含无关条目、计划冲突、错误类型、符号链接和候选变化矩阵；
- 每个事务步骤的故障注入与原字节保留测试；
- 重复确认、伪造输入、旧 Preview Hash 和额外 DTO 字段拒绝测试；
- 无 Key/无网络真实 Tauri 组件路径与离线编辑回归；
- 脱敏截图和不含机器路径的项目创建报告。

### 完成定义

NG-401 只有在真实 UI 可离线完成“输入 → 全量预览 → 独立确认 → 一次性 Scaffold → 打开编辑”，并且全部冲突/重复/故障路径都证明不覆盖用户原内容时才算完成。

### 技术验收记录（2026-08-24）

- `pnpm quality` 全量通过：格式、Lint、TypeScript、110 个 React 测试、边界检查、Clippy、Rust Workspace 测试、debug/release Smoke、Sprint 1～3 回归与 NG-401 E2E 均通过；
- NG-401 E2E 证明 Provider、Router、Vault、预览前写入和确认前写入均为 0；完整 Scaffold 为 11 个目录、4 个文件；15 个事务故障注入点全部回滚且保留用户原内容；
- 真实 release Tauri 窗口证据位于 `target/ng401-evidence/tauri-ng401-window.png`，本次质量门重跑后的 SHA-256 为 `afc5f69f2ac65accf52436d21b71ea1d6170ee09af394e8df026ee4ffdfcff99`；
- 脱敏安全报告位于 `target/ng401-evidence/security-report.json`，不含 API Key、Provider Response 或机器绝对路径字段。

产品负责人于 2026-08-25 明确回复“批准 NG-401 验收，开始 NG-402”。

当前状态：**NG-401 已获产品验收；NG-402 已获授权。**

## 8. NG-402——实现 setting-create

### Story

作为小说作者，我需要从设定简报和自己选择的有限世界资料生成结构化设定提案，以便把世界、地点或规则安全地写入 `world/`。

### 用户价值

用户能建立可读、可链接且格式一致的世界基础，同时准确知道哪些已有设定被发送给当前 Provider。

### 实施范围

- 登记 `novelgainker.setting-create` version 1、`source.create_setting` 与两个确认点；
- 新增 `SettingBriefV1`：名称、设定类型、目标、必含要点和可选限制；
- Source Picker 只允许最多 6 个有效 `world/` 文件；
- 扩展 Context Builder、Manifest 与 Prompt Compiler 到设定工作流；
- 新增 DeepSeek/OpenAI 固定结构化输出映射和 `SettingProposalV1`；
- 本地严格验证世界/地点/规则、摘要、氛围、规则、限制和剧情钩子；
- Rust 分配 `setting_` ID，生成固定 Frontmatter 和 Markdown；
- 安全名称映射到 `world/<设定名>.md`，目标存在时阻断 create；
- 完整预览、Proposal Hash、独立确认、可恢复 Apply 和写后打开；
- 支持在用户明确选择一个已有 v1 设定后形成单文件 replace Diff；若本 Sprint 不存在旧格式，只需保留通用 replace 能力和测试入口。

### 安全约束

- 未确认 Manifest 请求数为 0，未确认 Proposal 写入数为 0；
- 不自动读取人物、素材、剧情、章节、编辑器、搜索或整个 `world/`；
- 模型不能返回 ID、路径、Frontmatter、Markdown、链接或工具；
- 设定关系/引用只使用 Rust 验证的本地 ID；首版无需自动反向更新其他文件；
- 同名 collision 不覆盖、不编号，用户必须修改名称或明确选择现有目标走 replace；
- replace 不能改目标路径；来源或目标 Hash 变化会使确认失效；
- 结构失败最多一次同 Provider 修复，不跨 Provider；
- 设定正文和项目片段不进入 App State、日志、错误或证据。

### 验收条件

- [x] DeepSeek/OpenAI Mock 均进入相同 `SettingProposalV1` Validator。
- [x] Source Picker 只列有效 `world/` 来源并执行数量/总预算上限。
- [x] Manifest 显示全部来源、Hash、片段、理由和截断状态。
- [x] 模型无法指定 ID、路径、Frontmatter、Markdown 或工具。
- [x] Rust 生成稳定 `setting_` ID、Frontmatter 和确定性 Markdown。
- [x] 世界、地点和规则三类 Golden 均通过。
- [x] create collision、replace source-changed 和过期 Manifest 均为零写入。
- [x] 未确认、取消、失败和放弃路径项目不变。
- [x] 成功 Apply 只创建或替换一个 `world/` 文件。
- [x] 磁盘字节与已确认 Proposal Hash 一致。
- [x] 当前 Provider 失败不会请求另一 Provider。
- [x] 普通聊天和离线编辑回归通过。

### 测试与证据

- Setting Brief/Proposal 正负 Schema、语义上限与恶意字段矩阵；
- 世界/地点/规则 Serializer Golden 与跨 Provider 等价测试；
- Source Picker 路径、链接、隐藏、无效文件、预算和 Hash 变化测试；
- create/replace、collision、当前编辑目标、第二 Writer、恢复和 Validation 失败测试；
- React Manifest、完整预览、确认、成功刷新打开和脏编辑保护测试；
- 高熵 Setting Canary 扫描与脱敏 Run 报告。

### 完成定义

NG-402 只有在用户能审查有限来源、通过任一 Mock Provider 获得同一 Schema 的设定提案，并经 G3 安全循环只写入一个已确认 `world/` 文件时才算完成。

### 技术验收记录（2026-08-25）

- Workflow Pack 已登记 `novelgainker.setting-create` v1；Pack 共 3 个固定工作流、13 个构建期校验资源，App State 升级为 v4 并保留 v3 历史；
- Source Picker 只暴露有效 `world/` 元数据，正文仅在用户显式选择后进入受限 Manifest；最多 6 个来源、合计最多 16 KiB；
- DeepSeek/OpenAI Loopback 均进入同一 `SettingProposalV1` 本地 Validator；结构失败只允许当前 Provider 修复一次，禁止自动 Fallback；
- create 使用 Rust 生成 `setting_` ID、目标路径、Frontmatter 与 Markdown；collision 不覆盖；显式 replace 复用原路径和原 ID，提供完整 Diff，并在 Apply 前重新校验来源 Hash；
- `pnpm quality` 全量通过：格式、Lint、TypeScript、20 个文件/110 个 React 测试、边界检查、Clippy、Rust Workspace 测试、debug/release 构建与窗口 Smoke、Sprint 1～3 回归、NG-401 回归及 NG-402 E2E；
- NG-402 E2E 证明 Manifest 确认前 Provider 请求为 0、Proposal 确认前写入为 0、跨 Provider 自动请求为 0、collision/source-changed 写入为 0；成功路径恰好完成 1 次 create 和 1 次同路径同 ID replace；
- 脱敏证据位于 `target/ng402-evidence/security-report.json`，SHA-256 为 `33f777e13ed51a9348d78c0f38786c9e96884c9b16d2907cfdaacb1dc65f6a41`，不含 API Key、Provider Response、设定正文或机器绝对路径。

产品负责人于 2026-08-25 明确回复“批准 NG-402 验收，开始 NG-403”。

当前状态：**NG-402 已获产品验收；NG-403 已获授权。**

## 9. NG-403——实现 material-create

### Story

作为小说作者，我需要把主题、线索、物件或场景种子整理成结构化素材，并自主决定它是否成为 Canon，以便保留有价值的创作材料而不把每次 AI 输出都写进项目。

### 用户价值

用户可以先思考、审查和丢弃素材；只有明确接受的内容才会成为 `materials/` 下的权威 Markdown。

### 实施范围

- 登记 `novelgainker.material-create` version 1、`source.create_material` 与发送/成为 Canon 两个确认点；
- 新增 `MaterialBriefV1`：显示名称、类型、种子、目标用途和可选约束；
- Source Picker 允许最多 6 个已验证 `world/`、`characters/`、`plot/` 来源；
- 明确排除 `materials/000.story-ledger.md`、章节、搜索、编辑器和聊天；
- 新增双 Provider 结构化输出和 `MaterialProposalV1`；
- 本地验证主题、线索、物件、场景种子四类数据；
- Rust 分配 `material_` ID，确定性生成 `materials/<素材名>.md`；
- Proposal UI 明确区分“继续审查”“确认成为 Canon”和“放弃”；
- 放弃后清除内存 Brief/Context/Response/Proposal，不持久化工作草稿；
- 确认后沿用单文件 create/replace、Hash、恢复、校验和写后打开。

### 安全约束

- AI 输出默认不是 Canon；只有 `promote_material_to_canon` 独立确认才能 Apply；
- 不创建隐藏草稿目录，不把未接受素材存入 SQLite、日志、聊天记录或项目元数据；
- `000.story-ledger.md` 和任何 `000.*` 保留目标都不可由模型、前端或普通素材 Workflow 写入；
- 模型不能指定 ID、文件名、路径、Frontmatter、链接、状态或写入决定；
- 名称冲突不覆盖、不编号；replace 必须由用户先选中具体普通素材文件；
- Context 只包含 Manifest 中明确选择且 Hash 未变化的来源；
- 一次同 Provider 修复以外不重试，不自动换 Provider；
- 未确认、取消、放弃、错误和窗口关闭均为零 Canon 写入。

### 验收条件

- [x] 四类素材均通过严格 `MaterialProposalV1` 和 Serializer Golden。
- [x] DeepSeek/OpenAI Mock 输出进入同一 Validator 和磁盘格式。
- [x] Source Picker 排除故事账本、章节、隐藏和无效来源。
- [x] 未确认成为 Canon 时项目写入数为 0。
- [x] 放弃后 App State、日志、DOM 和证据中无素材正文。
- [x] Rust 生成 `material_` ID 和安全目标，模型不能影响身份或路径。
- [x] `materials/000.story-ledger.md` 在所有正负路径中字节不变。
- [x] collision、过期 Hash、第二 Writer、恢复和 Validation 失败不覆盖。
- [x] 成功路径只创建或替换一个已确认普通素材文件。
- [x] 磁盘字节与确认 Proposal 完全一致。
- [x] 当前 Provider 失败不跨 Provider Fallback。
- [x] 离线手动创建/编辑素材仍可用。

### 测试与证据

- Material Brief/Proposal Schema、枚举、大小、重复与注入矩阵；
- 四类 Serializer Golden 与两家 Adapter 请求体测试；
- 故事账本/保留路径不可写 Property Test；
- 放弃、关闭、取消、未确认与内存清理测试；
- Context Source Picker/Manifest 与来源变化测试；
- Canon create/replace/collision/recovery/validation 测试；
- React “成为 Canon”独立确认、完整预览、刷新打开和脏编辑保护测试；
- Material Canary 泄漏报告。

### 完成定义

NG-403 只有在自动化证明素材可被完整审查和丢弃、未接受内容不留隐藏持久化、确认后只写一个普通 `materials/` 文件且故事账本始终不变时才算完成。

### 技术验收记录（2026-08-25）

- Workflow Pack 已登记 `novelgainker.material-create` v1；Pack 共 4 个固定工作流、17 个构建期校验资源；Tauri 启动和边界门均固定校验 4 个工作流；
- `MaterialBriefV1` 与严格 `MaterialProposalV1` 支持主题、线索、物件、场景种子四类；DeepSeek/OpenAI Loopback 返回字节一致的结构化领域数据，失败只允许当前 Provider 修复一次，禁止自动 Fallback；
- Source Picker 只暴露已验证 `world/`、`characters/`、`plot/` 元数据，最多 6 个来源、合计最多 16 KiB；所有 `materials/`（含故事账本）、章节、聊天、编辑器和搜索结果均不进入 Context；
- Rust 独占 `material_` ID、目标路径、Frontmatter、Markdown 和写入决定；create 冲突不覆盖，显式 replace 保留原路径与原 ID，并在 Apply 前重新校验目标 Hash；
- 未确认、放弃、取消、关闭和重启均不产生 Canon 写入或隐藏素材持久化；App State v5 只保存有界元数据并保留 v3/v4 历史；
- `materials/000.story-ledger.md` 与全部 `000.*` 目标由 Core、Tauri 和前端三层禁止；新建、替换、冲突和放弃路径均证明故事账本字节不变；
- 中文 UI 明确区分“继续审查”“确认成为 Canon”和“放弃素材”；确认成功后打开对应 Markdown，离线手动编辑、保存和恢复继续可用；
- `pnpm quality` 全量通过：格式、Lint、TypeScript、20 个文件/113 个 React 测试、边界检查、Clippy、Rust Workspace、debug/release 构建与真实窗口 Smoke、Sprint 1～3、NG-401、NG-402 及 NG-403 E2E；
- NG-403 E2E 证明跨 Provider 自动请求、Manifest 确认前请求、Proposal 确认前写入、目录正文暴露、隐藏草稿持久化、Story Ledger 写入、collision/source-target-changed 写入和放弃写入均为 0；成功路径恰好完成 1 次 create 和 1 次同路径同 ID replace；
- 脱敏证据位于 `target/ng403-evidence/security-report.json`，SHA-256 为 `fd766ca8bc9683bdbb85b3e9e16781cf684ba9c9069d7b650866ea9f37ded685`，不含 API Key、Provider Response、Prompt、素材正文、隐藏草稿或机器绝对路径。

产品负责人于 2026-08-25 明确回复“批准 NG-403 验收，开始 NG-404”。

当前状态：**NG-403 已获产品验收；NG-404 已获授权。**

## 10. NG-404——实现 plot-create

### Story

作为小说作者，我需要把故事承诺、压力、转折、揭示和回收整理成结构化主剧情，以便在写章节前拥有一份可审查、可修改且 Provider 无关的剧情 Canon。

### 用户价值

用户能看到完整剧情方向和重要秘密，并在真正写入 `plot/main.md` 前独立确认；重新规划时也能审查清晰 Diff，而不是被 AI 静默改写。

### 实施范围

- 登记 `novelgainker.plot-create` version 1、`source.create_plot` 与发送/主剧情写入确认；
- 新增 `PlotBriefV1`：标题、类型、核心问题、期望结局、必含节点和禁止方向；
- Source Picker 允许最多 8 个已验证人物、设定和普通素材；
- 新增双 Provider 固定结构化输出和 `PlotProposalV1`；
- 本地严格验证 Promise、Pressure、有序 Turns、Reveals、Payoffs 和未决问题；
- 首次创建由 Rust 分配 `plot_` ID，目标固定 `plot/main.md`；
- 已有主剧情时，用户必须先选择“修订主剧情”，复用原 ID 并形成完整 replace Diff；
- UI 对 Reveal、Payoff 和未决问题使用清晰分区，但仍以纯文本安全渲染；
- 复用 Proposal Hash、独立确认、源 Hash 重检、可恢复 Apply 和写后验证；
- 成功后刷新文件树并在不丢失脏编辑的前提下请求打开 `plot/main.md`。

### 安全约束

- 模型不能新增剧情文件、支线路径、ID、Frontmatter、Markdown 或操作；
- `plot/main.md` 之外的目标一律拒绝，目标存在时 create 不得退化为 replace；
- replace 必须绑定原 `plot_` ID、当前文件 Hash、Manifest Hash 和完整 Proposal；
- 未被选择的人物、设定、素材、章节、大纲、故事账本和聊天不会进入 Prompt；
- Reveals、Payoffs 和未决问题视为敏感 Canon，不进入日志、App State、错误、DOM 快照或证据；
- 一次同 Provider 修复不能读取新来源或切换 Provider；
- 取消、未确认、旧 Hash、第二 Writer、恢复阻断和校验失败均为零写入；
- 不在本 Story 更新 Story Ledger、Outline 或人物关系反向引用。

### 验收条件

- [x] 两家 Mock Provider 使用同一 `PlotProposalV1` 与本地 Validator。
- [x] Promise/Pressure/Turns/Reveals/Payoffs 顺序和上限稳定。
- [x] Source Picker 只允许已选择且有效的人物、设定和普通素材。
- [x] 首次成功只创建 `plot/main.md` 并获得 Rust 本地 ID。
- [x] 已存在主剧情时必须走显式 replace Diff，create 不覆盖。
- [x] replace 复用原 ID，源 Hash 或 Manifest 变化会阻断。
- [x] 模型和前端不能提供路径、ID、Markdown、工具或 File Operation。
- [x] 未确认、取消、冲突和错误路径写入数为 0。
- [x] 成功磁盘字节与确认 Proposal 完全一致。
- [x] Reveal/Payoff Canary 在禁止位置泄漏数为 0。
- [x] 当前 Provider 失败不触发另一 Provider。
- [x] 离线手动编辑剧情文件保持可用。

### 测试与证据

- Plot Brief/Proposal Schema、数组顺序、上限、重复和恶意字段矩阵；
- create/replace Serializer Golden、ID 复用与 Proposal Hash 测试；
- 固定目标、伪造路径、旧源 Hash、collision、第二 Writer、恢复和 Validation 测试；
- Context Picker/Manifest 来源选择、预算与变化测试；
- React 分区预览、完整 Diff、独立确认、刷新打开与脏编辑保护测试；
- 双 Provider Loopback、禁止 Fallback 和 Plot Canary 报告。

### 完成定义

NG-404 只有在主剧情能以固定 Schema 和固定目标完成 create/replace，全程可预览、可取消、可恢复，并证明任何旧确认或未确认路径都不会改变 `plot/main.md` 时才算完成。

### 技术完成记录（2026-08-25）

- Workflow Pack 已登记 `novelgainker.plot-create` v1；Pack 共 5 个固定工作流、21 个构建期校验资源；Provider 仅返回严格 `PlotProposalV1`，Rust 独占 `plot_` ID、固定路径、Frontmatter、Markdown 和写入决定；
- Plot Context 只接受用户显式选择的已验证 `characters/`、`world/` 与普通 `materials/` 来源，最多 8 个；`plot/`、`chapters/`、大纲、Story Ledger、聊天和未选来源均不会进入 Prompt；
- 首次 create 只允许固定目标 `plot/main.md`；已有主剧情必须显式 replace，复用原 ID/路径并显示完整 Diff。Manifest、Proposal 和目标源 Hash 任一变化都会使确认失效；
- App State v6 只保存有界工作流元数据并保留 v3/v4/v5 历史；Brief、Context、Provider Response、Proposal、Reveal、Payoff、Diff 和 Canon 均不持久化；
- `pnpm quality` 全量以退出码 0 通过：格式、Lint、TypeScript、20 个文件/115 个 React 测试、边界检查、Clippy、Rust Workspace、Debug/Release 桌面构建与窗口 Smoke、Sprint 1～3、NG-401～NG-404 E2E；
- NG-404 E2E 证明双 Mock Provider 输出契约一致；跨 Provider 自动请求、Manifest 确认前请求、Proposal 确认前写入、隐藏草稿、Story Ledger 写入、ID/路径变更、collision/过期/放弃写入以及 Reveal/Payoff 证据泄漏均为 0；成功路径恰好完成 1 次 create 和 1 次同路径同 ID replace；
- 脱敏证据位于 `target/ng404-evidence/security-report.json`，SHA-256 为 `479d7d79ccb1ba8758b88e0cbfc4e7e45a04d6646a7d928a974a3d41a9f5086e`；本 Story 的自动化未调用真实 Provider API。

产品负责人于 2026-08-25 明确回复“批准 NG-404 验收，开始 NG-405”。

当前状态：**NG-404 已获产品验收；NG-405 技术完成、待产品验收；NG-406～NG-407 未授权。**

## 11. NG-405——完善 character-create v2

### Story

作为小说作者，我需要创建或完善带稳定身份、关系、动机、秘密和剧情功能的人物，以便人物可以安全地与设定和主剧情建立可验证联系。

### 用户价值

用户能继续使用 Sprint 3 的人物创建体验，同时获得稳定 ID 和更完整的人物表；旧人物不会被后台迁移，所有升级差异都由作者逐个确认。

### 实施范围

- 将 `novelgainker.character-create` 提升到 version 2，并保留 `source.create_character` 内部能力；
- 新增 `CharacterBriefV2` 的 create/enhance 模式；
- create 接受新人简报；enhance 要求用户从文件树/选择器选中一个已有人物目标；
- Source Picker 允许最多 6 个已验证人物、设定、主剧情和普通素材；
- 新增 `CharacterProposalV2`：v1 字段加动机、秘密、剧情功能和关系；
- Rust 为新人分配 `character_` ID；v2 replace 复用 ID；v1 replace 在预览中显式增加本地 ID 和 Schema v2；
- 关系 DTO 只接受用户已选择实体的 ID、关系类型和有界说明；
- Rust 根据项目索引解析关系目标显示名和相对链接，模型不返回路径；
- create 继续使用安全人物名映射；enhance 永远替换选中原路径，不做重命名；
- 完整预览/Diff、独立确认、Hash、恢复、严格验证和写后打开继续复用 G3。

### 安全约束

- 不扫描或自动升级全部 v1 人物；一次 Run 只创建或替换一个人物文件；
- 模型不能生成实体 ID、文件路径、Frontmatter、Markdown 链接、关系目标路径或操作；
- 关系 ID 必须存在、类型允许、来源有效并在本次 Manifest 中明确选择；
- 重复 ID、同名 create、缺失关系、关系目标变化和目标源 Hash 变化均阻断；
- enhance 修改显示名不重命名文件，也不修改其他人物的反向关系；
- 人物秘密不进入 App State、日志、错误、Router、DOM 测试快照或证据；
- 不读取章节、搜索、聊天、未保存编辑器或未选择人物；
- 结构修复、取消、Fallback、确认和 Apply 继续遵守 G3 契约。

### 验收条件

- [x] v1 人物继续可读、可手动编辑，且不会自动迁移。
- [x] 新建 v2 人物获得 Rust `character_` ID 和确定性 Markdown。
- [x] v1 → v2 只通过单文件 replace Proposal，并完整显示新增字段和 ID。
- [x] v2 replace 复用原 ID，不能改写身份或路径。
- [x] 关系只能引用 Manifest 中明确选择且当前存在的实体 ID。
- [x] 重复 ID、同名、缺失/过期关系和源变化均为零写入。
- [x] 两家 Mock Provider 进入同一 `CharacterProposalV2` Validator。
- [x] 模型不能指定 ID、路径、Frontmatter、链接、Markdown 或工具。
- [x] 未确认、取消、失败和放弃路径项目不变。
- [x] 成功路径只写一个已确认人物文件，其他关系文件不变。
- [x] 人物秘密 Canary 泄漏数为 0。
- [x] Sprint 3 `CharacterProposalV1`、普通聊天与离线编辑回归通过。

### 测试与证据

- Character v1 读取兼容、v2 Schema 和 v1 → v2 Golden Diff；
- ID 生成/复用、重复 ID、关系存在性/类型/选择/过期 Property Test；
- create collision、enhance 原路径、名称变化不重命名和单文件写入测试；
- 两家 Adapter v2 Structured Output、单次修复和禁止 Fallback 测试；
- Context Picker、Manifest、人物秘密内存清理和 Canary 扫描；
- React create/enhance 模式、关系选择、升级提示、完整 Diff 与确认测试；
- G3 v1 人物全量回归。

### 完成定义

NG-405 只有在新人、v2 完善和显式 v1 升级都保持单文件、稳定 ID、可验证关系和独立确认，并证明不会后台迁移或修改其他 Canon 时才算完成。

### 技术完成记录（2026-08-25）

- `novelgainker.character-create` 已升级为 version 2，支持 `CharacterBriefV2` create/enhance 与严格 `CharacterProposalV2`；人物动机、秘密、剧情功能和关系均进入固定 Schema，而不是自由 Markdown；
- Source Picker 只暴露用户显式选择的至多 6 个人物、设定、主剧情和普通素材元数据；v1 人物显示升级警告，enhance 目标自动计入选择；章节、聊天、搜索、未保存编辑器和未选择实体不进入 Context；
- Rust 独占人物 ID、路径、Frontmatter、Markdown 和关系链接：create 新建一个文件；v2 enhance 复用同一 ID 与原路径；v1 仅在显式升级 Proposal 后获得新 v2 ID；不会创建反向关系或改写其他 Canon；
- App State v7 从 v6 事务迁移，历史终态 v1/v2 运行继续可读，但新的 character 写入只接受当前 v2；持久化仍限有界元数据，不保存 Key、人物秘密、正文或 Provider 原始输出；
- DeepSeek 与 OpenAI Mock Adapter 共用同一 Validator、同一有界单次结构修复和同一确认后 Apply；禁止跨 Provider 自动 Fallback；
- `pnpm quality` 以退出码 0 全量通过：格式、Lint、TypeScript、20 个文件/117 个 React 测试、47 个 Workflow 测试、87 个 App Core 测试（4 个按设计忽略）、25 个 Tauri 测试、Clippy、Debug/Release 构建与真实窗口 Smoke、Sprint 1～3 回归及 NG-401～NG-405 E2E；
- NG-405 E2E 证明确认前 Provider 请求和写入、跨 Provider 自动请求、反向关系写入、collision/过期源写入及人物秘密泄漏均为 0；成功路径覆盖 1 次 create 和 1 次保持同 ID/路径的 enhance。脱敏报告：`target/ng405-evidence/security-report.json`，SHA-256 `021edd69097a66923b331adf5451c340cdd447a5afb7531d2f62994a81127429`；
- 自动化遵守 S4-Q007，仅使用双 Mock Provider，未发起真实 API 请求。

### 产品验收记录（2026-08-25）

产品负责人明确回复“批准 NG-405 验收，开始 NG-406”。

当前状态：**NG-405 已获产品验收；NG-406 技术完成，等待产品验收；NG-407 未授权。**

## 12. NG-406——实现确定性路由和中文默认契约

### Story

作为作者，我需要从明确按钮、中文命令或含糊对话进入正确的已实现工作流，以便自然使用功能，同时始终知道 AI 是否仅在做意图分类。

### 用户价值

明确操作即时、离线且零成本；含糊请求可以得到建议，但低置信度不会替用户做决定，更不会触发生成或写入。

### 实施范围

- 为五个已实现工作流建立固定 UI 按钮和简体中文展示；
- 建立版本化确定性意图规则，只识别精确且单义的中文命令；
- 新增 `RouterDecisionV1`、本地 Validator 和两家 Provider固定分类请求；
- Router 只接收本条含糊文本和已实现 Workflow ID/说明，不读取项目或聊天历史；
- UI 在 Provider 分类前提示接收方、固定模型、发送范围和少量 API 用量；
- 按 S4-Q006 冻结高/中/低置信度规则；
- 高置信度只打开建议表单，中低置信度、多意图和未知结果打开固定选择器；
- 无 Key、离线、Vault 错误、取消或畸形输出直接降级为本地选择器；
- 记录脱敏 Router 元数据：Provider、Workflow ID、置信度级别、状态和用量，不保存原文；
- 为 Registry 增加“可被路由”只读标志，但不暴露内部 Prompt 或工具。

### 安全约束

- 按钮和确定性命令 Router 请求数为 0；
- Provider Router 没有工具、项目会话、文件句柄、Canon Proposal 或 Apply 权限；
- 分类结果不能自动 Prepare、Generate、发送项目 Context、确认或写入；
- 只允许返回当前 Pack 中 `executionAvailable` 的五个 Workflow ID；
- Router 失败不允许结构修复之外的循环重试，也不跨 Provider；
- 用户原文不进入 App State、日志、错误、证据或后续工作流 Context，除非用户在打开表单后重新确认其内容；
- 高风险与未实现能力不能通过相近文本映射；
- 默认资源、规则和测试不含韩语命名、韩语命令或韩国语境分支。

### 验收条件

- [x] 五个明确按钮均零 Provider 请求并打开正确表单。
- [x] 固定中文命令规则确定、版本化且无多义匹配。
- [x] 含糊分类前显示 Provider、模型、发送范围和用量提示。
- [x] `RouterDecisionV1` 只能返回已实现 Workflow ID 与置信度。
- [x] 高置信度也只打开表单，不生成、不写入。
- [x] 中低置信度、多意图、未知和畸形结果展示固定选择器。
- [x] 无 Key/网络/Vault 时选择器离线可用。
- [x] Router 看不到项目、聊天历史、搜索或编辑器内容。
- [x] 分类失败不自动请求另一 Provider。
- [x] Router 原文 Canary 在禁止位置泄漏数为 0。
- [x] 简体中文默认和无韩语专用逻辑静态/组件测试通过。
- [x] 普通 AI 对话契约不被 Router 替换或破坏。

### 测试与证据

- 确定性中文命令 Golden、冲突词、多意图和未实现意图矩阵；
- RouterDecision Schema、两家 Adapter、畸形/超限/取消/错误/禁止 Fallback 测试；
- 高/中/低置信度和无 Key 离线选择器组件测试；
- Provider 请求体断言只有固定规则、本条文本和工作流目录；
- 零项目 Context、零工具、零 Canon 写入和 Canary 扫描；
- 五个入口的键盘、焦点、折叠栏与窄屏回归。

### 完成定义

NG-406 只有在明确入口完全离线、含糊路由可见可取消、所有结果都停在工作流表单之前，并证明 Router 永远没有项目读取或写入权限时才算完成。

### 技术完成记录（2026-08-25）

- Workflow Pack 已加入版本化确定性意图规则、严格 `RouterDecisionV1` Schema、固定 Router 指令和只读 `routable` 标志；只登记 project、setting、material、plot、character 五个已实现工作流；
- 六个精确中文命令走 Rust 本地规则且 Provider 请求数为 0；多意图、相近句式、高风险或未实现能力不会被本地误路由；
- 含糊文本先展示当前 Provider、固定模型、发送范围和少量 API 用量提示，只有用户明确同意才发起可取消分类；请求只含本条文本与固定工作流目录，不含项目会话、Canon、聊天、搜索或编辑器内容；
- DeepSeek/OpenAI 使用同一严格 Validator；`high` 只打开建议表单，`medium`/`low`、未知、畸形、取消和无 Key 均停在固定选择器，不自动 Prepare、Generate、Confirm 或 Apply；仅允许一次同 Provider 结构修复，瞬时错误不重试且禁止跨 Provider Fallback；
- App State 已事务迁移到 v8，只保留至多 20 条终态 Router 元数据；不保存用户原文、Prompt、Response、远端 Payload、项目标识或文件内容；
- 普通“发送”对话路径保持独立且回归通过，离线项目打开、Markdown 手动编辑、原子保存、搜索与校验不受 Router 或 Key 状态影响；
- `pnpm quality` 以退出码 0 全量通过：格式、Lint、TypeScript、21 个文件/125 个 React 测试、53 个 Workflow 测试、87 个 App Core 测试（4 个按设计忽略）、27 个 Tauri 测试、Clippy、Debug/Release 构建与真实窗口 Smoke、Sprint 1～3 回归及 NG-401～NG-406 E2E；
- NG-406 脱敏报告：`target/ng406-evidence/security-report.json`，SHA-256 `062a6eeeed3fe0db7f6101ab1113a0cc25d21efedf6b2d84c666633b2b76d506`；报告确认项目读写、同意前请求、自动执行、跨 Provider Fallback、原文/远端 Payload 持久化均为 0；
- 自动化遵守 S4-Q007，仅使用 DeepSeek/OpenAI Loopback Contract，未发起真实 API 请求。

### 产品验收记录（2026-08-25）

产品负责人明确回复“批准 NG-406 验收，开始 NG-407”。

当前状态：**NG-406 已获产品验收；NG-407 已获授权；G4 未批准。**

## 13. NG-407——建立实体工作流 Golden Test 与 G4 E2E

### Story

作为产品负责人，我需要用可重复的跨 Provider、跨工作流和故障矩阵验证实体基础，以便批准 G4 Workflow Foundation，而不是依赖单次人工演示。

### 用户价值

用户可以信任不同 AI Provider 只改变内容建议，不改变项目格式、确认规则、实体身份或磁盘安全性。

### 实施范围

- 在验证过的系统临时根中建立高熵合成项目、实体和隐私 Canary；
- 覆盖 project-create 的输入、预览、确认、事务、重复与故障恢复；
- 覆盖 setting/material/plot/character v2 的 Registry → Context → Generate → Proposal → Confirm → Apply → Validate；
- 覆盖每类未确认、取消、迟到事件、重复确认、过期 Manifest/Proposal/Source Hash；
- 覆盖名称 collision、保留路径、重复/缺失 ID、过期关系、第二 Writer、外部修改、恢复阻断和 Validation 失败；
- 覆盖 v1 人物读取、显式 v2 升级与零后台迁移；
- 覆盖按钮/命令零 Router 请求、含糊 Router 高中低置信度、无 Key 离线选择器；
- 覆盖 DeepSeek/OpenAI Loopback 的四类实体结构化输出和禁止 Fallback；
- 验证 Provider 切换后 Serializer Golden、Frontmatter、ID、路径和最终字节不变；
- 扫描 SQLite、日志、错误、DOM、截图元数据、项目外测试产物和 G4 证据；
- 将 Sprint 4 E2E 加入 `pnpm quality`；
- 生成 `target/g4-evidence/security-report.json` 与真实 Tauri G4 窗口截图。

### 安全约束

- 只操作精确验证的临时根和合成 Canon，不读取真实用户项目或真实 Key；
- 默认流程 Cargo 离线、无外网、无真实 Provider 请求；
- 证据不含 Brief、Router 原文、Prompt、Response、Context、Proposal、Diff、实体秘密、剧情 Reveal、绝对路径或完整 Canon；
- 不能通过降低 Schema、缩小 Canary、跳过恢复、允许 Fallback 或减少负面矩阵让 Gate 通过；
- G4 E2E 不注册或启动 Sprint 5 Storytelling 工作流。

### 验收条件

- [x] 最终 Pack 的五个 Workflow、版本、资源和 Hash 与预期完全一致。
- [x] project-create 在确认前零写入，成功 Scaffold 完整，冲突/故障不覆盖。
- [x] 四类 AI 实体在两家 Mock Provider 下进入相同本地 Schema。
- [x] Provider 切换不改变同一结构化输入的 Serializer Golden。
- [x] 每个成功 Run 最多写一个已确认 Canon 文件。
- [x] 未确认、取消、过期、冲突、无效关系和失败路径写入数为 0。
- [x] 半写、静默覆盖、自动编号和后台迁移数为 0。
- [x] 故事账本和 Sprint 5 目录在全部实体流程中保持原字节。
- [x] 明确路由 Provider 请求数为 0；含糊路由 Canon 写入数为 0。
- [x] 跨 Provider 自动请求数为 0。
- [x] Prompt/Response/Context/Proposal/Router/Secret Canary 泄漏数为 0。
- [x] App State 删除/损坏与 Workflow 故障不影响离线编辑。
- [x] Sprint 1/2/3 全量回归通过。
- [x] Debug/Release 构建、真实 Tauri G4 Smoke 和统一质量入口通过。
- [x] G4 报告和截图不含 Key、正文、用户路径或真实手稿。

### 测试与证据

- 五工作流 Pack/Registry 精确资源与版本报告；
- project-create 事务/回滚/重复/快照变化矩阵；
- 四实体 Schema/Serializer Golden 与两家 Loopback 矩阵；
- Context 来源、Manifest、路由、ID、关系和 v1 升级矩阵；
- Proposal/Apply/Recovery 请求与写入计数；
- App State/离线编辑与 Sprint 1～3 回归；
- Canary 泄漏扫描报告；
- `target/g4-evidence/security-report.json`；
- `target/g4-evidence/tauri-g4-window.png`；
- 完整 `pnpm quality` 日志。

### 完成定义

NG-407 只有在自动化证据证明五个 Workflow 资源精确、Provider 切换不改变格式、每个成功 Run 只写一个已确认文件、全部负面路径零写入、跨 Provider 自动请求和敏感泄漏均为 0 时才算完成。

### 技术完成记录（2026-08-25）

- G4 聚合器已固定校验最终 Pack 的 5 个 Workflow、24 个构建期资源和 Pack Hash `96b31eab09aa68b7a5e32ba9bfeae79d4a99ef25503422a6932f89532d012fcf`，并记录每个 Workflow 与资源的内容 Hash；
- project-create 的 11 个目录、4 个文件和 15 步回滚矩阵，以及 setting/material/plot/character v2 的 DeepSeek/OpenAI Loopback、Schema、Serializer Golden、Proposal、Apply 与 Recovery 证据均在同一次质量运行中完成汇总；
- 高熵合成 Canary 已扫描 SQLite、日志、错误、DOM、PNG 元数据和脱敏报告；Story Ledger、`outlines/`、`chapters/` 等 Sprint 5 Sentinel 保持原字节，临时根经安全前缀验证后清理；
- 汇总矩阵确认：每个成功 Run 最多 1 个已确认 Canon 写入；负面路径写入、半写、静默覆盖、自动编号、后台迁移、明确路由 Provider 请求、含糊路由 Canon 写入、跨 Provider 自动请求、Fallback 和敏感 Canary 泄漏均为 0；
- `pnpm quality` 以退出码 0 全量通过：格式、Lint、TypeScript、21 个文件/125 个 React 测试、53 个 Workflow 测试、87 个 App Core 测试（4 个按设计忽略）、27 个 Tauri 测试、Clippy、Debug/Release 构建与真实窗口 Smoke、Sprint 1～3 回归、NG-401～NG-406 E2E 和 NG-407/G4 汇总；
- 脱敏报告：`target/g4-evidence/security-report.json`，SHA-256 `c16bdcaa67c691c33e80f8c15dfcfca9eec452cbd2ace7c89ca425c454a94f67`；真实窗口截图：`target/g4-evidence/tauri-g4-window.png`，SHA-256 `50b58cfe1b40facb689a3646e11a26e918236f455ef47fda24b42170ab25dde3`；
- 自动化遵守 S4-Q007，仅使用 DeepSeek/OpenAI Loopback Contract，未读取真实项目、真实 Key 或调用真实 Provider API。

### 产品验收记录（2026-08-25）

产品负责人明确回复“批准 NG-407 验收，进入 G4 Review Gate”。

当前状态：**NG-407 已获产品验收；G4 已通过并关闭 Sprint 4；Sprint 5 Story Packet 已获准创建，但 NG-501 尚未授权。**

## 14. 跨 Story 验收矩阵

| 能力 | NG-401 | NG-402 | NG-403 | NG-404 | NG-405 | NG-406 | NG-407 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Workflow Pack | project v1 | setting v1 | material v1 | plot v1 | character v2 | 路由目录 | 精确汇总 |
| Project/Scaffold | 主责 | 使用 | 使用 | 使用 | 使用 | 项目前置判断 | E2E |
| Entity ID | Project ID | setting ID | material ID | plot ID | character ID/升级 | 只读目录 | 冲突矩阵 |
| Context Manifest | 无 Provider | world 来源 | 实体来源 | 人物/设定/素材 | 关系来源 | Router 无项目 Context | 汇总 |
| Structured Output | 无 | Setting V1 | Material V1 | Plot V1 | Character V2 | RouterDecision V1 | 双 Provider |
| Serializer/Golden | Scaffold | world Markdown | material Markdown | main plot | character v2 | 无 Canon | 主责 |
| Canon Apply | 创建事务 | 单文件 | 单文件 | main create/replace | 单人物 create/replace | 禁止写入 | 恢复汇总 |
| 隐私 | 元数据边界 | Setting Canary | Material Canary | Reveal Canary | Secret Canary | Router Canary | 全扫描 |
| Local-first | 全离线 | AI 不可用时编辑 | AI 不可用时编辑 | AI 不可用时编辑 | v1 仍可编辑 | 离线选择器 | 全回归 |
| G4 演示 | 必需 | 必需 | 必需 | 必需 | 必需 | 必需 | 汇总 |

## 15. Sprint 4 Gate G4

只有全部技术条件和产品批准满足，Sprint 4 才能结束：

- [x] NG-401～NG-407 均满足 Definition of Done 并获得产品验收。
- [x] G3 单 Manager、Manifest、Proposal Hash、确认和可恢复 Apply 未被旁路。
- [x] 最终 Pack 只注册 project/setting/material/plot/character 五个已实现 Workflow。
- [x] Pack、Workflow、Schema、模板、Serializer 和 Golden Test 一一对应。
- [x] project-create 完全离线，确认前零写入，冲突/重复/故障不覆盖。
- [x] 用户可从空目录建立项目、设定、素材、主剧情和人物 Canon。
- [x] 模型不能指定实体 ID、路径、Frontmatter、Markdown、链接、工具或文件操作。
- [x] Rust ID、Frontmatter、路径和 Serializer 在 Provider 间保持一致。
- [x] 每个 AI Run 只允许一个单文件 Proposal 和一次独立确认写入。
- [x] material 被拒绝成为 Canon 时无隐藏持久化。
- [x] plot/main create/replace 和 character v1 → v2 升级均有完整 Diff 与 Hash 绑定。
- [x] 重复/缺失 ID、无效关系、collision、过期 Hash 和第二 Writer 均零写入。
- [x] 按钮和明确命令零 Router 请求；含糊 Router 永远零 Canon 写入。
- [x] Router 只显示已实现 Workflow，并使用简体中文置信度说明。
- [x] 当前 Provider 失败不触发另一 Provider 请求。
- [x] Prompt、Response、Context、Proposal、Diff、Router 原文和实体秘密不进入持久状态或证据。
- [x] 无 Key、无网络、Vault/Workflow/App State 故障时离线编辑和项目创建可用。
- [x] Sprint 1～3 全量回归、统一质量入口、Debug/Release Build 和真实 Tauri G4 演示通过。
- [x] G4 证据不包含 Key、正文、用户路径或真实手稿。
- [x] 产品负责人批准进入 Sprint 5 Story Packet。

### G4 Review 记录（2026-08-25）

- 结论：**技术审查通过，建议批准 G4；等待产品负责人最终批准。**
- Gate 清单 20 项中，19 项技术与既有 Story 产品验收条件已满足；仅“产品负责人批准进入 Sprint 5 Story Packet”保持未勾选。
- `pnpm quality` 以退出码 0 重新全量通过；G4 报告确认 5 个 Workflow、24 个固定资源、2 个 Loopback Provider、每 Run 最多 1 个已确认 Canon 写入，全部负面写入、格式不一致、跨 Provider 自动请求、真实凭据读取和敏感泄漏计数均为 0。
- 最新脱敏报告 SHA-256 为 `c16bdcaa67c691c33e80f8c15dfcfca9eec452cbd2ace7c89ca425c454a94f67`；真实 Tauri G4 截图 SHA-256 为 `50b58cfe1b40facb689a3646e11a26e918236f455ef47fda24b42170ab25dde3`。
- 未发现新的架构歧义或范围变更；未调用真实 Provider，未注册或启动 Sprint 5 工作流。
- 建议的下一批准口令：`批准 G4，关闭 Sprint 4，创建 Sprint 5 Story Packet`。

### G4 产品批准记录（2026-08-26）

产品负责人明确回复“批准 G4，关闭 Sprint 4，创建 Sprint 5 Story Packet”。G4 的 20 项条件现已全部满足，Sprint 4 正式关闭；该授权只允许创建 Sprint 5 Story Packet，不自动通过 Sprint 5 Review Gate，也不授权开始 NG-501。

## 16. 待确认决策

### S4-Q001——project-create 是否调用 AI，以及元数据向导字段

发现的歧义：Sprint 4 把 project-create 列为 Workflow，但项目尚未建立时没有项目会话或 Context；若为了生成项目简介而调用 AI，会让第一步依赖 Key/网络，并扩大隐私和失败面。元数据向导也需要冻结最小字段。

建议：project-create 固定为 `deterministic-write`，不调用 AI。向导包含必填标题、可选作者显示名、可选类型和可选一句话前提，写作语言固定简体中文；所有内容只进入可预览的 `project.md`，技术元数据保持无正文。

影响：接受后新用户无需 Key 即可建立项目，后续可手动编辑 `project.md`；拒绝并要求 AI 辅助项目简介，需要新增无项目 Context 的 Provider 流程、用量说明和单独 Story。

状态：**已接受**（2026-08-24）。

### S4-Q002——实体工作流的默认 Context 是否自动读取关联目录

发现的歧义：设定、素材、剧情和人物可以从大量 Canon 获益，但自动扫描目录会让发送范围失控，也会让长篇项目迅速超过预算。

建议：除有界 Brief、`project.md` 和根 `AGENTS.md` 外，所有实体来源都必须由用户在 Source Picker 逐项选择；采用本 Packet 5.4 的目录白名单、数量和 16 KiB 总预算。未选择来源不读、不发，超预算要求用户减少选择。

影响：接受后隐私和成本可解释，但用户需要多一步选择；拒绝并要求自动关联，需要定义可证明的自动优先级、预算和排除规则，实质提前引入 Sprint 5 Context Builder。

状态：**已接受**（2026-08-24）。

### S4-Q003——实体 ID 与旧人物迁移策略

发现的歧义：关系和跨文件链接需要稳定身份，但 Sprint 3 人物文件没有 ID；批量后台迁移会修改用户 Canon，按文件名引用又会在改名后失效。

建议：Rust 为新实体生成带类型前缀的 UUID；模型不能返回 ID。v1 人物继续可读和手动编辑，只有用户选择“完善人物”时才能通过单文件 replace Proposal 显式升级到 v2；不批量迁移。关系只能引用本次明确选择且存在的实体 ID。

影响：接受后身份稳定且迁移可审查，但旧人物在升级前不能成为结构化关系目标；拒绝则需要选择路径身份、可变 slug 或一次批量 Migration，并重新评审多文件写入风险。

状态：**已接受**（2026-08-24）。

### S4-Q004——实体目标路径与主剧情文件策略

发现的歧义：设定、素材和人物可以自然使用中文可读文件名；主剧情既可以固定一个文件，也可以允许多个 Plot/支线。多文件会提前引入选择、合并和路由复杂度。

建议：设定写 `world/<名称>.md`，素材写 `materials/<名称>.md`，新人写 `characters/<人物名>.md`，主剧情固定 `plot/main.md`。create 冲突全部阻断；replace 只修改选中原路径，不重命名、不自动编号。支线和多 Plot 留给后续 Story。

影响：接受后目标确定、G4 容易审查；拒绝并允许多个 Plot，需要增加 Plot ID 选择、主从关系和章节 Context 优先级。

状态：**已接受**（2026-08-24）。

### S4-Q005——未接受素材是否保存为工作草稿

发现的歧义：material-create 强调“用户决定是否成为 Canon”。若拒绝后仍保存到 App State 或隐藏目录，就会形成第二套内容来源；若完全丢弃，用户不能稍后恢复该次结果。

建议：未确认成为 Canon 的素材只保留当前进程内存；用户放弃、关闭或重启后清除，不写 App State、聊天历史或隐藏目录。用户若想保留，可确认成为 Canon，或复制内容后用右栏手动创建 Markdown。

影响：接受后权威边界简单且私密，但未接受结果不可恢复；拒绝则必须设计显式“非 Canon 草稿”目录、生命周期、搜索和发布排除规则。

状态：**已接受**（2026-08-24）。

### S4-Q006——含糊 Router 的置信度与 UI 行为

发现的歧义：架构要求低置信度让用户选择，但数字阈值和英文 `Confidence: high` 对普通用户不直观；自动执行工作流又可能产生误解。

建议：`RouterDecisionV1` 使用 `high | medium | low` 固定枚举，UI 显示“较明确 / 需要确认 / 不确定”，不直接显示英文 Confidence。`high` 只打开建议表单；`medium`、`low`、多意图和未知结果展示工作流选择器。任何级别都不自动 Prepare、Generate 或写入。

影响：接受后行为清晰、无需争论模型分数；拒绝并使用数值阈值，需要冻结跨 Provider 校准方法和阈值回归数据。

状态：**已接受**（2026-08-24）。

### S4-Q007——G4 是否要求四实体双 Provider 真实运行

发现的歧义：S3-Q005 已把真实双 Provider Conformance 留到 NG-703；Sprint 4 新增四类结构化输出，若 G4 又要求双 Live，会重复受 Key、额度和网络阻断。

建议：G4 使用 DeepSeek/OpenAI Loopback Contract、真实本地 Validator、真实 Canon Apply/Recovery 和真实 Tauri Mock E2E；产品负责人可自愿执行真实实体生成，但不作为 G4 阻断项。NG-703 仍负责 Windows 内部测试候选前的双 Provider Live Conformance。

影响：接受后 Gate 可重复且不处理真实 Key；拒绝则需在 G4 前准备两家凭据、额度、网络和不保留实体正文的人工 Live 流程。

状态：**已接受**（2026-08-24）。

## 17. 实施决策权限

以下属于 Story 内低风险实现选择，可由实施者选择并在验收记录中说明：

- Crate/模块内部命名和文件拆分，只要依赖方向不反转；
- JSON Schema Validator、UUID 和 Canonical JSON Library；
- 各字段在本 Packet 上限以内的更严格字符/数组限制；
- Source Picker 的排序、筛选和键盘交互；
- 实体预览卡片与 Diff 的视觉布局，只要完整内容和目标可审查；
- G4 合成 Fixture 名称、Canary 值、报告字段排序和截图时间；
- 稳定简体中文错误文案，只要错误码和脱敏边界不变。

以下改变范围，必须停止并请求新产品决定：

- 让 project-create 调用 AI 或在项目创建前保存 Provider Context；
- 自动扫描/发送未选择的 Canon、章节、搜索、聊天或未保存编辑器；
- 允许模型/前端指定 ID、路径、Frontmatter、Markdown、链接、工具或操作；
- 批量迁移旧人物、多文件 Proposal、关系反向同步、自动重命名/移动/删除/合并；
- 为未接受素材增加隐藏草稿库或把正文存入 App State；
- Router 自动开始生成、自动确认、自动写入或读取项目 Context；
- 注册 Sprint 5+ 工作流或实现章节、连续性、Ledger、评审、视觉、EPUB；
- 加载 Novelist/Codex Plugin、`SKILL.md`、Agent YAML、Python 或用户工作流；
- 增加 Provider、任意模型、跨 Provider Fallback、Multi-agent 或并行 Run；
- 增加 Android、云同步、产品内 Git、安装器、签名、自动更新或公开发布。

## 18. Sprint 4 Story Packet Review Gate

开始 NG-401 前，需要产品负责人确认：

1. Sprint 4 可演示结果、七个 Story 和 G4 Gate 符合预期；
2. 接受或修改 S4-Q001～S4-Q007；
3. project-create 保持离线确定性，不把 AI 作为创建项目依赖；
4. 实体 Context 采用显式 Source Picker，不自动扫描整个项目；
5. 实体 ID 由 Rust 生成，旧人物只逐文件显式升级；
6. 目标路径和 `plot/main.md` 策略可接受；
7. 未接受素材不形成隐藏持久化；
8. Router 任何置信度都只打开表单，不生成或写入；
9. Workflow Pack 继续是 NovelGainker 原生固定资源，不兼容或运行 Novelist；
10. 第一版继续单 Manager、单 Run、两家固定 Provider 和禁止自动 Fallback；
11. 团队继续按 1 人严格顺序执行；
12. Review Gate 通过后只授权开始 NG-401，除非产品负责人另行批量授权。

当前结论：**NG-401～NG-407 已获产品验收；G4 已通过并关闭 Sprint 4；Sprint 5 Story Packet 已获准创建，NG-501 尚未授权**（截至 2026-08-26）。

后续授权记录：

- 2026-08-25，产品负责人明确回复“批准 NG-401 验收，开始 NG-402”；
- 2026-08-25，产品负责人明确回复“批准 NG-402 验收，开始 NG-403”；
- 2026-08-25，产品负责人明确回复“批准 NG-403 验收，开始 NG-404”；
- 2026-08-25，产品负责人明确回复“批准 NG-404 验收，开始 NG-405”；
- 2026-08-25，NG-405 已完成实现并通过统一质量门；
- 2026-08-25，产品负责人明确回复“批准 NG-405 验收，开始 NG-406”；
- 2026-08-25，NG-406 已完成实现并通过统一质量门；
- 2026-08-25，产品负责人明确回复“批准 NG-406 验收，开始 NG-407”；
- 2026-08-25，NG-407 已完成实现并通过统一质量门；
- 2026-08-25，产品负责人明确回复“批准 NG-407 验收，进入 G4 Review Gate”；
- 2026-08-25，G4 Review Gate 技术审查通过，20 项中 19 项满足，仅等待产品负责人批准进入 Sprint 5；
- 2026-08-26，产品负责人明确回复“批准 G4，关闭 Sprint 4，创建 Sprint 5 Story Packet”；
- 最新授权已关闭 Sprint 4 并允许创建 Sprint 5 Story Packet，但不自动通过 Sprint 5 Review Gate 或授权 NG-501。

建议的批准口令：

`S4-Q001～Q007 全部接受，批准 Sprint 4 Story Packet Review Gate，只授权开始 NG-401`

批准记录：2026-08-24，产品负责人使用上述口令批准。

## 19. G3 交接记录

G3 于 2026-08-24 由产品负责人明确批准并关闭 Sprint 3：

- NG-301～NG-306 已全部完成技术实现、统一质量门和产品验收；
- G3 的 18 项技术条件全部通过；
- 双 Provider Loopback、确认写入、取消/过期/冲突零写入、Applying 中断恢复和 App State/离线编辑回归均通过；
- 未确认/取消/过期/冲突写入、半写、自动跨 Provider 请求和 Canary 泄漏均为 0；
- `target/g3-evidence/security-report.json` 与真实 Tauri 窗口截图 Hash 一致；
- 按 S3-Q005，真实双 Provider Live 不作为 G3 阻断项，仍由 NG-703 在 Windows 内部测试候选前负责；
- 产品负责人回复“批准 G3，关闭 Sprint 3，创建 Sprint 4 Story Packet”；
- 该批准只关闭 Sprint 3 并授权创建本 Packet，不自动通过 Sprint 4 Review Gate，也不授权 NG-401。
