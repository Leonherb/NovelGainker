# NovelGainker Agent Migration AM0 Story Packet

> 状态：已由 2026-08-30 RB0/AM-R1 路线取代；仅作为安全契约迁移证据，不是当前产品 Gate
>
> 日期：2026-08-28
>
> BMAD 数据包：`execution-packet`
>
> Program：Agent Migration AM0——Agent 契约与 Skill Pack 供应链
>
> 范围：NG-A001～NG-A004
>
> 团队：1 人，严格顺序实施
>
> 上游计划：[sprint-plan.zh-CN.md](../sprint-plan.zh-CN.md)（Amendment A-001 修订已批准）
>
> 上游架构：[architecture.zh-CN.md](../architecture.zh-CN.md)（Amendment A-001 已批准）
>
> 架构重审：[architecture-unified-novelist-agent-review.zh-CN.md](../architecture-unified-novelist-agent-review.zh-CN.md)（AR-Q001～AR-Q006 已确认）
>
> as-built 基线：[sprint-5-story-packet.zh-CN.md](./sprint-5-story-packet.zh-CN.md)（G5 已通过并关闭）

## 1. AM0 目标

AM0 为统一 Novelist Agent 会话建立可测试、可追踪、可逐 Story 授权的契约基础，但不改变当前 release 的运行时行为。

AM0 完成后，项目必须具备：

1. 可证明来源、许可、固定 commit、文件清单和内容 Hash 的 Novelist-compatible Skill Pack。
2. 九项 Skill 的本地 Manifest、允许工具、上下文边界、风险等级与输出契约。
3. `ProjectAgentSessionV1`、`AgentTurnV1` 和唯一 Writer 状态机。
4. DeepSeek/OpenAI 共用的 Agent Action、Tool Request、Tool Result 与严格 JSON 降级协议。
5. `plan-only`、`review-every-write`、`auto-apply-safe` 的授权与风险矩阵。
6. 有界会话摘要、Turn 元数据、多文件事务与撤销记录的持久化契约。
7. GA0 契约证据，证明新资源与类型尚未接入生产入口，G5 as-built 行为没有变化。

AM0 不是统一 Agent UI、只读 Tool Loop 或自动写入的实现 Sprint。AM0 Story Packet 获批后也只允许按顺序单独授权一个 Story。

## 2. GA0 审查演示

GA0 Review 应能在不调用真实 Provider、不读取真实项目和不改变 release 行为的前提下复现：

1. 从干净工作树读取上游来源锁，显示仓库、commit、子目录、许可证 Hash 和检索日期。
2. 重算 Skill Pack 文件清单、逐文件 SHA-256 与规范化 Pack Hash。
3. 证明九项 Skill 都有唯一 ID、版本、触发说明、允许工具、读写范围、风险等级和资源 Hash。
4. 修改一个未登记资源，构建期验证稳定失败；恢复后 Pack Hash 回到原值。
5. 修改用户插件缓存或删除本机 Novelist 安装，证明 NovelGainker 的构建结果不受影响。
6. 使用 Fixture 解析 `ProjectAgentSessionV1` 与 `AgentTurnV1` 的合法/非法状态转换。
7. 对同一意图分别输入 OpenAI 风格 Tool Call、DeepSeek 风格 Tool Call 和严格 JSON 降级结果，得到等价的规范化 `AgentActionRecordV1`。
8. 拒绝未知字段、重复 Call ID、绝对路径、未注册 Skill/Tool、越权风险声明、循环超限和跨 Provider Fallback。
9. 对授权矩阵执行 Golden Test，证明低风险、安全写入、高风险确认和禁止动作分类稳定。
10. 对持久化与撤销 Schema 执行敏感字段 Canary，证明 API Key、绝对路径、隐藏推理和 Provider 原始响应不可进入契约。
11. 运行完整质量门与 G5 回归，证明普通聊天、Router、表单 Workflow、Canon 写入和 App State 仍保持原行为。
12. 查看 GA0 Manifest，确认没有新增 Tauri Command、Capability、生产 Tool 执行器、数据库 Migration 或 UI 入口。

## 3. 范围

### 3.1 包含

- 上游 Novelist 来源、commit、许可、Attribution、文件清单和本地改动记录。
- 九项 Skill 的兼容语义、必要模板/参考资料、允许工具和风险声明。
- 构建期 Skill Pack 验证、稳定排序、逐文件 Hash 与 Pack Hash。
- 纯类型/纯策略的 Agent Session、Turn、Action、Tool、授权、持久化和撤销契约。
- DeepSeek/OpenAI Mock Adapter 到同一 Agent Action 协议的映射 Fixture。
- 负面 Schema、状态机、权限、Prompt Injection 和隐私 Fixture。
- NG-A001～NG-A004 的单元、Property、Golden、边界和 GA0 证据。

### 3.2 不包含

- 把新 Skill Pack 接入当前 Conversation、Router 或 Workflow Manager。
- 新建或修改 Tauri IPC、Command Manifest、权限、Capability 或前端客户端。
- 新建 Agent Timeline、Skill 选择 UI、Tool 展示、Diff、撤销或授权设置 UI。
- 实际读取、搜索、创建、Patch、删除、移动或重命名项目文件。
- App State 数据库 Migration、会话摘要写入、完整聊天持久化或撤销 Blob 写入。
- 真实 DeepSeek/OpenAI Agent Tool Loop、网络请求或 Live Smoke。
- 删除、禁用或重构 `provider_chat_start`、Router、Workflow Pack、Manager、表单或现有确认 UI。
- Python、Shell、外部 Agent Host、运行时 Plugin Cache、用户 `SKILL.md` 或远程 Skill 下载。
- AM1、AM2、Sprint 6 或 Sprint 7 的任何实现。

## 4. 继承自 G5 与 Amendment A-001 的硬边界

1. Markdown 与项目 assets 仍是唯一 Canon；App State 可删除且不能成为隐藏小说事实来源。
2. Windows 11 x64、Tauri 2、React/TypeScript、Rust、Local-first 和 BYOK 不变。
3. 文本 Provider 只有 DeepSeek 与 OpenAI，固定端点、协议、模型和 Preset。
4. 禁止自动跨 Provider Fallback；Provider 不能授予权限。
5. WebView 与 Provider 不获得任意文件系统、绝对路径、Shell、Process、Opener 或任意 URL。
6. Rust Core 继续独占路径、Schema、Hash、事务、恢复和最终写入决定。
7. 单项目 Agent、多 Skill、唯一活动 Turn、唯一 Canon Writer；不引入 Multi-agent、Handoff 或并行 Apply。
8. 默认目标授权为 `auto-apply-safe`，但 AM0 不开放任何 Agent 写工具。
9. 高风险动作始终单独确认；Prompt、Skill、项目正文和 Tool Result 都不能提升权限。
10. Release 不依赖 Codex、ChatGPT、外部 Novelist Host、Plugin Cache 或 Python 环境。
11. G3～G5 的 Manager、Provider、Validator、Serializer、Hash、原子写入、恢复与测试在 GA2 前保留。
12. 完整聊天默认不持久化；任何含正文的本机状态都必须项目隔离、有界、可清空并通过隐私审查。

## 5. Skill Pack 供应链基线

### 5.1 已观察证据

- 公开上游仓库为 [comfuture/novelist](https://github.com/comfuture/novelist)。
- 2026-08-28 查询到的 `main` commit 为 `64e4a2e1473ff0bad5893eff394ccbf1fa1cd580`。
- 同日 `git ls-remote` 没有返回公开 tag，因此不能用“最新 tag”作为可复现来源。
- 本机缓存目录标为 `0.2.0`，但缓存内 `plugin.json` 没有版本字段，随包 release notes 又记录 `0.1.0`。
- 上游仓库声明 MIT 并包含根 `LICENSE`；本机缓存 payload 只在 README 声明 MIT，没有携带许可证正文。
- NovelGainker 当前 Cargo Workspace 为 `UNLICENSED`；第三方 MIT 内容必须作为独立来源和许可记录处理。

因此，本机插件缓存、目录名和 README 版本文字都不能单独成为发行来源锁。

### 5.2 来源锁

`SkillPackProvenanceV1` 至少包含：

- `sourceRepository`
- `sourceCommit`
- `sourceSubtree`
- `retrievedAt`
- `licenseSpdx`
- `licenseTextSha256`
- `upstreamInventorySha256`
- `adaptationVersion`
- `novelGainkerPackVersion`
- `includedFiles`
- `excludedFiles`
- `localChanges`
- `generatedFromScripts`

来源锁和许可证正文进入仓库；构建和 release 不访问网络。任何来源字段缺失、文件 Hash 漂移、未登记文件、符号链接、可执行文件或许可证缺失都必须使 Skill Pack 验证失败。

### 5.3 Pack 内容边界

第一版 Pack 固定九项 Skill：

| Skill                  | AM0 语义范围                       | AM0 风险声明       |
| ---------------------- | ---------------------------------- | ------------------ |
| `create-novel-project` | 项目 Scaffold、目录与初始规则      | 高风险；AM0 不执行 |
| `create-setting`       | 世界、地点、时代、规则与氛围       | mixed              |
| `create-character`     | 人物、关系、动机、秘密与连续性     | mixed              |
| `create-material`      | 主题、物件、线索、研究与场景种子   | mixed              |
| `create-plot`          | 主剧情、转折、揭示与回收           | mixed              |
| `novel-story-telling`  | 章纲、写章、修订、连续性与读者认知 | high/mixed         |
| `analytical-review`    | 独立、证据化、默认只读评审         | read-only          |
| `create-visual-asset`  | 视觉 Prompt、图片导入与登记语义    | high/mixed         |
| `publish-novel`        | EPUB 预检、构建与验证语义          | high               |

每个 `SkillManifestV1` 至少声明：

- `id`、`version`、`description`、`triggerSummary`
- `instructionResources`、`templateResources`、`referenceResources`
- `allowedTools`、`readScopes`、`writeScopes`
- `riskClass`、`alwaysConfirmActions`
- `contextBudget`、`outputContract`
- `sourceCommit`、`resourceHashes`
- `featureGate`、`enabledAtRuntime`

AM0 中全部 `enabledAtRuntime=false`。

### 5.4 适配规则

- 保留实现产品语义所需的 Skill 指令、模板和参考资料，并保留 MIT 许可与 Attribution。
- 不把 Codex/Claude/Antigravity Adapter、网站、submission 元数据或宿主专用 YAML 当作运行时 Skill。
- 不把 Python 脚本带入 release。脚本行为需要时，记录“上游脚本 → Rust/声明式契约 → Golden Fixture”的迁移映射。
- 不逐字继承宿主专用工具名、Shell 命令、Git 命令、绝对路径或网络能力。
- NovelGainker 的 App Policy、目录 Schema、简体中文体验和安全边界优先于上游 Prompt。
- Pack 升级只能通过新的固定 commit、来源/许可审查、语义 Diff、Golden 回归和产品批准；禁止运行时自动更新。

## 6. Agent Session 与 Turn 契约

### 6.1 `ProjectAgentSessionV1`

Session 记录：

- 不透明 `sessionId` 与 `projectId`
- 当前 `providerId`、`presetVersion`、`modelId`
- `skillPackVersion` 与 `skillPackHash`
- 当前授权策略
- 有界会话摘要版本与 Hash
- 最近 Turn 索引
- 唯一活动 Turn ID
- Writer 租约状态
- 创建、更新时间与终止原因

不同项目的 Session、摘要、Turn、Tool Result、事务和撤销记录不得交叉引用。

### 6.2 `AgentTurnV1`

状态固定为：

`idle → planning → reading → acting → awaiting_high_risk_confirmation → applying → validating → completed`

失败分支固定为：

- `planning | reading | acting → cancelled | failed`
- `applying | validating → completed | recovery_required`

Turn 创建时冻结：

- Project/Session
- Provider/Preset/Model
- Skill Pack 版本与 Hash
- 授权策略
- 用户请求 Hash
- 上下文预算和工具预算

Turn 进行中不能切换 Provider、Pack 或授权策略。取消后不能启动新 Agent Step 或 Tool Call；写事务开始后只能完成、回滚或进入 `recovery_required`。

### 6.3 基础会话策略

`ProjectAgentBasePolicyV1` 是宿主不可变策略，不是第十项 Skill：

- 每条项目消息都进入同一 Session。
- Agent 可以在无需工具时直接回复，但不能进入一个无项目、无权限审计的旁路聊天。
- 一个 Turn 可选择零个、一个或多个 Skill；选择零个 Skill 只允许讨论、澄清或产品帮助。
- Skill 只提供工作方法和输出约束，不能授予工具、路径、网络或写入权限。
- 未打开项目时只允许创建/打开项目和无写入产品帮助。

## 7. Agent Action、Tool 与 Provider 契约

### 7.1 模型输出与 App 记录分离

模型只能生成严格 `AgentActionPayloadV1`：

- `respond`：返回可见回复，不调用工具。
- `request_tools`：请求一个有界、串行的工具批次。
- `complete`：结束 Turn，并给出可见摘要。

模型可建议 `skillIds`、`toolId` 和结构化参数，但不能生成或覆盖 Session ID、Project ID、Provider、授权策略、风险结论、绝对路径、最终文件路径、Writer 租约或确认结果。

App 把通过校验的 Payload 包装为 `AgentActionRecordV1`，增加：

- Session/Turn/sequence
- Provider/Preset/Model
- Skill Pack 版本与 Hash
- 生效 Skill
- App Policy 判定
- Action Hash
- 前序 Action Hash
- 时间与脱敏状态

### 7.2 `ToolRequestV1`

Tool Request 至少包含：

- Turn 内唯一 `callId`
- 注册 `toolId`
- 严格 `arguments`
- 模型声明的用途

项目 Tool 的 Session/Project 绑定由 App 注入，不能来自模型。路径参数只能是经过 Schema 限制的项目相对路径或不透明 Token。

### 7.3 `ToolResultV1`

Tool Result 至少包含：

- `callId`、`toolId`
- `status`
- 稳定错误码
- 有界结构化结果
- `resultHash`
- `truncated` 与截断原因
- 信任标签

Tool Result 是不可信数据，不能改变 System Policy、Skill 许可或授权状态。

### 7.4 Provider 适配

- OpenAI 原生 Tool Call、DeepSeek 原生 Tool Call 和严格 JSON 降级必须映射到同一 Schema。
- 不支持原生 Tool Calling 时，Provider 只能输出一个严格 JSON Action；解析失败最多向同一 Provider 修复一次。
- 未知字段、未知 Action、未知 Tool、重复 Call ID、越权参数和超限批次立即失败。
- Provider 错误、修复失败或 Tool 失败不能自动请求另一 Provider。
- AM0 只使用确定性 Mock/Loopback Fixture，不发真实网络请求。

## 8. Turn 预算

建议第一版固定：

- 每 Turn 最多 8 个 Agent Step。
- 每 Turn 最多 16 个 Tool Call。
- 每个 Tool 批次最多 8 个 Call，并按声明顺序串行。
- 每 Turn 最多涉及 24 个唯一项目文件。
- 每 Turn 最多修改 8 个文件，并且只有一个写事务。
- 每个 Provider 请求继续遵守 96 KiB 本地硬上限。
- 超限时停止当前 Turn，展示原因，并建议用户拆分为下一 Turn；不得后台继续。

AM0 只冻结与测试这些上限，不执行真实工具。

## 9. 授权与风险矩阵

| 等级             | 默认行为                                     | 代表动作                                                                                                 |
| ---------------- | -------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `read-safe`      | 活动项目会话内可执行并记录来源               | 列目录、读取/搜索/校验受支持项目 Markdown                                                                |
| `write-safe`     | `auto-apply-safe` 可执行；其他策略按各自规则 | 新建普通实体 Markdown、局部 Patch 普通 Draft/规划文件、新建不存在的 Draft 章节                           |
| `always-confirm` | 三种策略下都需绑定 Turn/Plan/Hash 的单独确认 | 整文件或整章替换、accepted/final 内容、状态/Ledger、规则与关键元数据、项目外导入、发布构建/替换          |
| `forbidden-v1`   | 拒绝，无确认绕过                             | 任意删除/移动/重命名、任意绝对路径、隐藏目录、符号链接越界、Shell/Process/Opener、权限或 Skill Pack 修改 |

策略行为：

- `plan-only`：全部写动作拒绝。
- `review-every-write`：每个写事务在提交前审查完整 Diff。
- `auto-apply-safe`：仅 `write-safe` 自动提交，完成后展示完整 Turn Diff 与撤销。
- `always-confirm` 不能因用户之前确认过相似动作、当前 Skill 或 Provider 建议而降级。
- 一个 Turn 混合 `write-safe` 与 `always-confirm` 时，整个写事务等待高风险确认，避免部分提交。

保护对象至少包括：

- 项目 `AGENTS.md`、Skill/权限/安全配置。
- `materials/000.story-ledger.md` 与其他 `000.*` 关键状态文件。
- 章节 `status`、accepted/final 章节、整章替换和跨多章批量改写。
- `project.md` 的书名、语言、封面、Schema 与来源映射等关键字段。
- 项目外文件、assets 导入、`published/` 构建和已存在产物替换。

局部更新 `plot/main.md`、普通 world/character/material/outline 文件和 Draft 章节可以按 Action 范围判定为 `write-safe`；整文件替换、结构破坏或超出 Turn 上限时升级或拒绝。

## 10. 持久化、隐私与撤销契约

### 10.1 默认本机状态

建议默认每项目保存：

- 不超过 32 KiB UTF-8 的有界会话摘要。
- 最近 200 条消息索引和 Turn/Skill/Tool 元数据，不包含消息正文。
- 最近 100 个终态 Turn 的 Hash、状态、用量和诊断元数据。
- 最近 20 个可撤销写入 Turn，且最长保留 30 天。
- 当前授权策略；Provider Key 仍只存在 Credential Vault。

完整聊天正文默认关闭。用户显式启用后也必须项目隔离、可清空、有容量上限并有可见隐私说明；无安全存储能力时不得静默降级为明文持久化。

### 10.2 Undo 内容

- SQLite 只保存 Undo 元数据、Blob ID、Hash、大小、过期时间和状态。
- 反向 Patch 或必要 Before Image 存放在项目隔离的本机 Undo Blob，不进入项目目录、日志或证据。
- 含正文的 Undo Blob 使用 Windows 用户上下文保护；解密失败时不尝试猜测或静默覆盖。
- 每 Turn Undo 内容最多 2 MiB，每项目最多 20 MiB。
- 超出可撤销上限的写计划不能进入 `auto-apply-safe`；必须拆分或停止。
- 撤销前重新读取全部目标 Hash。任何目标已被后续人工或 Agent 修改时，撤销拒绝并展示冲突。
- 删除 App State 或用户清空会话会丢失摘要与撤销历史，但不修改 Canon。

### 10.3 禁止持久化

- API Key、Key 特征或 Credential 内容。
- Provider 隐藏状态、隐藏推理、原始响应或未脱敏错误正文。
- 绝对项目路径、项目外导入路径或一次性 Token。
- 未经用户启用的完整聊天正文。
- 无界手稿副本、完整 Context Pack 或 Tool Result 正文。

## 11. NG-A001——Skill Pack 来源、许可与 Manifest

### 11.1 用户价值

作者获得一个自包含、可重复构建、不会随本机插件缓存漂移的小说 Skill Pack。

### 11.2 实施范围

- 固定上游仓库、commit、子目录和检索日期。
- 引入准确许可证正文、Attribution 和第三方清单。
- 选择并登记九项 Skill 的必要指令、模板和参考资料。
- 记录排除项、宿主专用内容和脚本迁移映射。
- 建立 `SkillPackProvenanceV1`、`SkillManifestV1` 和 Pack Manifest。
- 实现构建期清单、Hash、许可、未知文件、符号链接和可执行文件验证。
- 全部 Skill 保持 `enabledAtRuntime=false`。

### 11.3 验收标准

- [x] 来源锁使用公开仓库的完整 commit，不使用缓存目录名或浮动分支作为发布身份。
- [x] 仓库包含准确 MIT 许可证正文、版权/Attribution 和第三方清单。
- [x] 九项 Skill 各有唯一 Manifest、版本、来源、资源 Hash、工具和风险声明。
- [x] Pack 清单稳定排序并产生可重复 Pack Hash。
- [x] 未登记文件、Hash 漂移、许可证缺失、符号链接或可执行文件使构建失败。
- [x] Python、宿主 Adapter、Agent YAML、网站和 submission 元数据不进入运行时 Pack。
- [x] 修改/删除本机插件缓存不改变 NovelGainker 构建或测试。
- [x] 当前 Workflow Pack Hash、运行时注册表、IPC、UI 和 App State Schema 不变。
- [x] 完整质量门与 G5 回归通过。

### 11.4 证据

- `am0-skill-pack-provenance.json`
- `am0-skill-pack-inventory.json`
- `am0-skill-pack-hash-report.json`
- `am0-license-and-attribution-report.md`
- `am0-cache-independence-report.json`

证据由 `pnpm e2e:nga001` 生成到 `target/qa/ng-a001/`，并作为完整质量序列的最后一个 Gate 重放。

### 11.5 技术实现与验证记录

- 公开来源锁定为 `comfuture/novelist` commit `64e4a2e1473ff0bad5893eff394ccbf1fa1cd580`；固定归档和完整 `plugins/novelist` 子树已重新核验。上游子树含 78 个文件，规范化 Inventory Hash 为 `5831708ba45b668dac36c0e6541d25a9abccacfd7e06cb549d503a325b630362`。
- 仓库保存上游 MIT 正文、Copyright (c) 2026 Changkyun Kim、Attribution、`THIRD_PARTY_NOTICES.md`、来源锁和逐资源清单；许可证 SHA-256 为 `88b9a14391d40016e79135d201f277885eabf9b5388279a3ea0a4270e708c109`。
- Pack 固定九项 Skill、36 个受锁资源加 `inventory.json`，总计 37 个资源；Pack Hash 为 `0ad3906792d9f2afcfab5f9e4206cd1285aeed07a640599dea4c1928fc57ba72`。全部 Skill 的 `enabledAtRuntime=false`。
- 原始 Skill 中的 Python/Shell/Git/宿主步骤、韩语默认 Scaffold、Provider Adapter、Agent YAML、网站和 submission 元数据未进入 Pack；13 个上游脚本均记录为等待以后授权的 Rust/声明式迁移映射。
- `crates/novelgainker-agent` 只提供构建期验证和只读元数据，没有生产调用者。构建会拒绝未知文件、资源 Hash 漂移、许可缺失、符号链接、可执行扩展、来源漂移、运行时误激活和未登记工具。
- 独立 E2E 证明生产 Crate 对 `novelgainker-agent` 的依赖数为 0、用户插件缓存引用数为 0、新 Agent/Skill IPC 数为 0、App State Schema 保持 v8、Tauri 命令保持 51 个；旧 Workflow Pack Hash 保持 `1658685537a9fe57b441928816e0fd2af770109e9b9a12a985204774496246a5`。
- 完整 `pnpm quality` 已以退出码 0 通过：27 个前端测试文件/149 项测试、全 Workspace Clippy/Rust 测试、G1～G5、Debug/Release Tauri 构建和桌面 Smoke 均通过。

### 11.6 产品验收门

- [x] 产品负责人明确接受 NG-A001。

产品验收结论：**通过**（2026-08-28）。产品负责人只授权开始 NG-A002；本记录不授权 NG-A003、NG-A004、AM1、AM2 或 Sprint 6。

建议批准语句：

“批准 NG-A001 产品验收；只授权开始 NG-A002”

## 12. NG-A002——ProjectAgentSession 与 Turn 契约

> 状态：技术实现、完整质量门与产品验收已于 2026-08-28 通过

### 12.1 用户价值

作者未来可以在一个项目隔离、可取消、可恢复且不会产生并行 Writer 的持续 Agent 会话中工作。

### 12.2 实施范围

- 建立未接入生产入口的纯 Rust Agent Contract/Policy 模块。
- 定义 Session、Turn、状态转换、冻结快照、预算、取消和恢复类型。
- 定义 Base Policy 与零/单/多 Skill 选择规则。
- 定义唯一活动 Turn 和唯一 Writer 不变量。
- 生成或验证对应 TypeScript/JSON Schema 契约。
- 建立状态机 Property Test 与跨项目隔离 Fixture。
- 不新增 IPC、数据库表、UI 或 Provider 调用。

### 12.3 验收标准

- [x] Session/Turn ID、版本、状态和所有冻结字段都有严格类型。
- [x] 合法状态转换完整；跳转、回退、终态复用和并行活动 Turn 被拒绝。
- [x] Provider、Pack 或授权策略在 Turn 中变化会使 Turn 失败，不静默迁移。
- [x] 取消和 `recovery_required` 语义可由确定性测试复现。
- [x] 不同 Project ID 的摘要、Turn、工具和事务引用无法交叉。
- [x] Base Policy 不是第十项 Skill，也不存在普通聊天旁路。
- [x] 新类型没有生产调用者，不改变当前 release 行为。
- [x] NG-A001 回归和完整质量门通过。

### 12.4 证据

- `am0-session-turn-schema-report.json`
- `am0-state-transition-matrix.json`
- `am0-project-isolation-report.json`
- `am0-runtime-unreachable-report.json`

证据由 `pnpm e2e:nga002` 生成到 `target/qa/ng-a002/`，并作为完整质量序列的最后一个 Gate 重放。

### 12.5 技术实现与验证记录

- `crates/novelgainker-agent` 新增严格、可序列化的 `ProjectAgentSessionV1`、`AgentTurnV1`、`ProjectAgentBasePolicyV1`、冻结 Provider/Pack/授权快照、有界 Turn Budget、项目引用、Writer Lease、取消与恢复类型。
- 固定 11 个 Turn 状态。121 个状态对中只有 23 个合法转换，其他 98 个跳转、回退和所有终态复用均拒绝；同一 Session 只允许一个活动 Turn 和一个匹配 `turnId` 的 Writer。
- Provider、Skill Pack 或授权策略发生快照漂移时，写入前 Turn 进入 `failed`；获得 Writer 后进入 `recovery_required` 并阻断新 Turn，直到显式完成恢复。
- Base Policy 固定 `isSkill=false`、`allowsBypassChat=false`；零 Skill 只允许讨论、澄清和产品帮助，项目工作必须选择一个或多个已登记 Skill。
- JSON Schema、TypeScript 镜像、两份合法 Fixture 和一份跨项目负例共 5 个受锁资源；Contract Set Hash 为 `3cbbc2bb99ccab33299880249b0f72a24acc84e2eb051ad99416864374f03ef8`。
- `novelgainker-agent` 共 13 项测试通过，其中 9 项覆盖 NG-A002 的契约锁、严格反序列化、全状态对性质、生命周期、唯一 Writer、快照漂移、取消恢复、项目隔离、Base Policy 和预算。
- 独立 E2E 证明生产依赖数、生产符号引用数、新 Agent/Skill IPC、新数据库 Migration、新 UI 入口、Provider 请求和项目写入均为 0；App State Schema 保持 v8，Tauri 命令保持 51 个，NG-A001 Pack Hash 不变。
- 完整 `pnpm quality` 已以退出码 0 通过：27 个前端测试文件/149 项测试、全 Workspace Clippy/Rust 测试、G1～G5、Debug/Release Tauri 构建和桌面 Smoke 均通过。

### 12.6 产品验收门

- [x] 产品负责人明确接受 NG-A002。

产品验收结论：**通过**（2026-08-28）。产品负责人只授权开始 NG-A003；本记录不授权 NG-A004、AM1、AM2 或 Sprint 6。

建议批准语句：

“批准 NG-A002 产品验收；只授权开始 NG-A003”

## 13. NG-A003——Agent Action、Tool 与 Provider 适配契约

> 状态：技术实现、完整质量门与产品验收已于 2026-08-28 通过

### 13.1 用户价值

作者使用 DeepSeek 或 OpenAI 时得到同一套可解释、可阻断越权动作的 Agent 行为。

### 13.2 实施范围

- 定义 Action Payload/Record、Tool Request/Result 和稳定错误码。
- 定义 Skill 选择、Tool 批次、Action Hash Chain 和 Turn Budget。
- 建立 OpenAI、DeepSeek 与严格 JSON 降级 Fixture。
- 证明三种输入映射到相同规范化 Action。
- 拒绝未知/重复/越权/超限 Action 与 Tool。
- 定义同 Provider 单次格式修复和禁止 Fallback。
- 使用 Mock Policy；不执行真实项目工具或网络请求。

### 13.3 验收标准

- [x] Action 与 Tool Schema 拒绝未知字段和不支持版本。
- [x] Provider 不能指定 Session/Project、绝对路径、授权结果或 Writer 权限。
- [x] Call ID 唯一，Tool 批次稳定排序并串行。
- [x] 两家 Provider 与 JSON 降级得到等价规范化 Action。
- [x] 最多一次同 Provider 修复；失败后 Turn 停止。
- [x] Step、Tool、文件和写事务预算全部可测试。
- [x] 错误日志不含完整模型输出、Key、绝对路径或项目正文。
- [x] 真实 Provider 请求数和项目写入数都为 0。
- [x] NG-A001～NG-A002 回归和完整质量门通过。

### 13.4 证据

- `am0-agent-action-schema-report.json`
- `am0-provider-normalization-matrix.json`
- `am0-action-negative-matrix.json`
- `am0-turn-budget-report.json`
- `am0-zero-network-zero-write-report.json`

### 13.5 技术实现与验证记录

- `crates/novelgainker-agent` 新增严格、可序列化的 Action Payload/Record、Model Tool Request、应用注入的 Project Tool Binding、串行 Tool Request、稳定 Tool Result/Error、Hash Chain 和仅 Hash 的脱敏错误日志契约。
- Provider 只能声明已登记 Tool 与项目相对路径；Session、Project、Turn、Provider、Skill Pack、有效 Skill、授权结果、Writer 权限、序号和 Action Hash 均由应用注入。未知字段、未知版本、重复 Call ID、绝对路径、未知 Tool、越权字段、超批次和跨 Provider 修复均被拒绝。
- OpenAI 原生 Tool Calling、DeepSeek 原生 Tool Calling 与严格 JSON 降级三份输入生成同一规范化 Action Hash `b7642265f8cbd69f5df6227b23d2c726af3ea330c8cf4251eeff7f883e185fc8`；格式修复最多一次且必须使用同一 Provider，第二次失败后停止 Turn。
- Turn Budget 固定最多 8 个 Agent Step、16 次 Tool Call、每批 8 次、24 个唯一项目文件、8 个修改文件、1 个写事务与 96 KiB Provider 请求；每个上限均有边界值和超限值测试。
- 独立 Action Contract Set 含 10 个受锁资源，Hash 为 `d4015ded6735e1a1612d02b411e7d4e51c4ffd08347a3cfe9a0013e7a9b6856c`；`novelgainker-agent` 共 20 项测试通过，其中 7 项为 NG-A003 契约、归一化、负向、预算、脱敏和零副作用覆盖。
- 独立 E2E 证明 8 类负向案例全部拒绝，错误不含原始 Provider 输出、Key、绝对路径或项目正文；生产依赖、生产符号引用、新 IPC、Migration、Capability、UI 入口、真实 Provider 请求、项目读取和项目写入均为 0。
- App State Schema 保持 v8，Tauri 命令保持 51 个；NG-A001 Pack Hash `0ad3906792d9f2afcfab5f9e4206cd1285aeed07a640599dea4c1928fc57ba72` 与 NG-A002 Contract Set Hash `3cbbc2bb99ccab33299880249b0f72a24acc84e2eb051ad99416864374f03ef8` 均不变。
- 完整 `pnpm quality` 已以退出码 0 通过：27 个前端测试文件/149 项测试、全 Workspace Clippy/Rust 测试、G1～G5、Debug/Release Tauri 构建、桌面 Smoke 以及 NG-A001～NG-A003 独立证据门均通过。

### 13.6 产品验收门

- [x] 产品负责人明确接受 NG-A003。

产品验收结论：**通过**（2026-08-28）。产品负责人授权开始 NG-A004；本记录不授权 GA0、AM1、AM2 或 Sprint 6。

建议批准语句：

“批准 NG-A003 产品验收；只授权开始 NG-A004”

## 14. NG-A004——授权、持久化、事务与撤销契约

### 14.1 用户价值

作者未来可以让 Agent 持续安全更新项目，同时清楚知道哪些动作自动执行、哪些必须确认，以及如何恢复。

### 14.2 实施范围

- 冻结三种授权策略和四级风险矩阵。
- 冻结保护对象、混合风险 Turn、超限和确认失效规则。
- 定义 Session Summary、消息索引、Turn 元数据与完整聊天 Opt-in Schema。
- 定义 Undo 元数据、受保护 Blob、容量、保留期、清空和冲突语义。
- 定义多文件零写入前置校验、事务、恢复和撤销契约。
- 建立风险 Golden、敏感字段 Canary 和 GA0 聚合证据。
- 不实现数据库 Migration、实际 Blob 存储或 Canon 写工具。

### 14.3 验收标准

- [x] 每个注册 Tool/Action 都唯一映射到风险等级和授权行为。
- [x] `plan-only` 零写入；`review-every-write` 事前审查；`auto-apply-safe` 只自动执行 `write-safe`。
- [x] 高风险与禁止动作不能被 Skill、Prompt、历史确认或 Provider 降级。
- [x] 混合风险 Turn 不发生部分自动提交。
- [x] 多文件事务冻结全部源/目标 Hash；任一前置失败时零写入。
- [x] Undo 容量、期限、Hash 冲突、清空和解密失败语义明确。
- [x] 默认持久化不含完整聊天；Opt-in 和敏感 Blob 不允许明文降级。
- [x] App State Schema 仍不迁移，当前运行时不保存新字段。
- [x] GA0 Manifest 证明没有新增生产权限或入口。
- [x] NG-A001～NG-A003 回归和完整质量门通过。

### 14.4 证据

- `am0-authorization-risk-matrix.json`
- `am0-persistence-privacy-schema-report.json`
- `am0-transaction-undo-contract-report.json`
- `am0-sensitive-field-canary-report.json`
- `ga0-manifest.json`
- `ga0-review-report.md`

### 14.5 技术实现与产品验收记录

技术实现结论：**通过**（2026-08-28）。

- 三档策略、四级风险、6 个注册 Tool 与 21 类 Action 已形成唯一映射；风险分布为 4 个 `read-safe`、3 个 `write-safe`、8 个 `always-confirm`、6 个 `forbidden-v1`。
- 高风险确认只接受应用内用户授权，并绑定 Project、Session、Turn、Action Plan Hash、全部 Source Hash 与 Policy Snapshot Hash；绑定变化或重复使用都会失效。
- 混合风险 Turn 作为一个原子事务等待确认；多文件前置校验冻结全部 Hash，任一不匹配时写入计数为 0。
- 默认持久化只允许 32 KiB 摘要、200 条消息索引和 100 个终态 Turn 元数据；完整聊天默认关闭，启用时必须显式 Opt-in、受保护、有容量/期限和隐私提示，禁止不受保护降级。
- Undo 上限为每 Turn 2 MiB、每项目 20 MiB、20 个写 Turn 或 30 天；Blob 不进入 SQLite 正文，解密失败和目标 Hash 冲突均拒绝猜测性恢复。
- 独立 Governance Contract Set 含 10 个受锁资源；GA0 修订后的最终 Hash 为 `a60b41ee0d16be3e035509076c7bdf110a506fb5d8c8203f1b0f948b72450f38`。
- `novelgainker-agent` 共 31 项测试通过；完整 `pnpm quality` 以退出码 0 通过。
- App State Schema 仍为 v8、Tauri Command 仍为 51；新增生产依赖、IPC、Migration、Capability、UI 入口、真实 Provider 请求、项目读取和项目写入均为 0。
- NG-A004 技术实现完成时，`target/qa/ng-a004/ga0-manifest.json` 与 `ga0-review-report.md` 只证明 GA0 技术证据就绪，字段 `ga0Approved` 为 `false`；后续 GA0 Review Gate 仍须单独授权。

产品验收结论：**通过**（2026-08-28）。产品负责人只授权开始 GA0 Review Gate；本记录不批准 GA0，不授权 AM1、AM2 或 Sprint 6。

产品验收确认项（已全部接受）：

1. 确认 `auto-apply-safe` 只覆盖普通实体 Markdown 新建/局部修改和不存在的 Draft 章节，保护对象始终要求确认，删除/移动/重命名与系统能力在 v1 始终禁止。
2. 确认默认不保存完整聊天；未来 Opt-in 必须受保护、有界、可清空并显示隐私提示。
3. 确认 Undo 的 20 Turn/30 天/2 MiB 每 Turn/20 MiB 每项目上限，以及冲突或解密失败时拒绝恢复的产品语义。
4. 确认本轮只有纯契约与证据，没有实现普通聊天运行时读写、数据库 Migration、UI 或 Agent Tool。

批准记录：

“批准 NG-A004 产品验收；只授权开始 GA0 Review Gate”

## 15. 跨 Story 依赖与授权矩阵

| Story   | 依赖             | 首次冻结能力                            | 生产行为变化             | 产品验收后下一步 |
| ------- | ---------------- | --------------------------------------- | ------------------------ | ---------------- |
| NG-A001 | G5、A-001        | Skill Pack 来源、许可、Manifest、Hash   | 无；Pack 未激活          | 可申请 NG-A002   |
| NG-A002 | NG-A001          | Session/Turn/Base Policy 契约           | 无；无生产调用者         | 可申请 NG-A003   |
| NG-A003 | NG-A002          | Action/Tool/Provider 规范化契约         | 无；Mock-only            | 可申请 NG-A004   |
| NG-A004 | NG-A002～NG-A003 | 授权、持久化、事务、撤销契约与 GA0 证据 | 无；无 Migration/Tool/UI | 可申请 GA0       |

严格规则：

1. 按 NG-A001 → NG-A002 → NG-A003 → NG-A004 严格顺序实施。
2. 每个 Story 独立实施、测试、证据、产品验收和文档同步。
3. 后一 Story 不因本 Packet 或前一 Story 获批而自动授权。
4. AM0 不允许生产 Agent Tool、数据库 Migration、新 IPC、新 Capability 或 UI。
5. 任何发现需要改变 AR-Q001～AR-Q006 的内容，必须退回 Architecture Review。
6. 任何上游许可、来源或 Pack Hash 问题先阻断 NG-A001，不能用本机缓存替代。
7. GA0 未通过前不得创建或启动 AM1 实施 Story。

## 16. GA0 Review Gate

### 16.1 验收清单

- [x] NG-A001～NG-A004 均有独立产品验收记录。
- [x] 上游仓库、完整 commit、子目录、许可证正文、Attribution 和文件 Hash 可追踪。
- [x] 九项 Skill 的 Manifest、资源、工具、范围、风险和版本完整。
- [x] Pack 构建不读取用户插件缓存、不联网、不运行 Python/Shell。
- [x] Skill Pack 未激活，不改变 release 行为。
- [x] Session、Turn、Base Policy 与唯一 Writer 契约通过。
- [x] 两家 Provider 和严格 JSON 降级共用 Agent Action/Tool Schema。
- [x] 未注册、越权、重复、超限和 Prompt Injection Action 被拒绝。
- [x] 三种授权策略与高风险矩阵 Golden Test 通过。
- [x] 持久化、隐私、事务与撤销契约没有阻断问题。
- [x] 默认状态不保存完整聊天，敏感内容不明文降级。
- [x] 当前 App State Schema、Tauri Commands、Capabilities、UI 和 Workflow Pack 行为不变。
- [x] G1 离线、G2 Provider、G3 Safe Apply、G4 Workflow 和 G5 Storytelling 回归通过。
- [x] 完整 `pnpm quality` 通过。
- [x] 产品负责人明确批准 GA0。

### 16.2 非阻断信息项

- AM0 只需要 Mock/Loopback Provider 证据；真实 Agent Live Conformance 留给 AM1/AM2 或 NG-703。
- Pack 的上游语义与 NovelGainker Policy 不完全相同时，以已批准 ADR 和本地安全契约为准，并记录适配差异。
- AM0 可以新增纯契约 Crate、JSON Schema 和资源验证，但不得使生产模块依赖或调用它们。
- 完整聊天 Opt-in 的 UI 与实际存储在 AM1 以后实现；AM0 只冻结契约。
- Undo Blob 的实际存储与 Canon 工具在 AM2 实现；AM0 只冻结格式、保护和上限。

### 16.3 独立审查结论（2026-08-28）

GA0 Review Gate 结论：**通过**（2026-08-28）。

首轮正向证据：完整 `pnpm quality` 以退出码 0 通过；四组 AM0 Hash 锁、28 项 `novelgainker-agent` 测试、G1～G5 回归、Debug/Release 构建与桌面 Smoke 均通过。App State 仍为 v8、Tauri Command 仍为 51，新 Agent 契约仍未接入生产运行时。

阻断项：

1. `GA0-R001`：`project.patch_markdown` 与 `project.create_markdown` 当前按 Tool ID 固定为 `write-safe`，但没有不可绕过的分类器把目标路径和修改形态绑定到 `modify_project_rules`、`modify_critical_metadata`、`replace_whole_file` 等高风险 Action。`protectedObjects` 目前只是清单，无法证明保护对象一定升级为 `always-confirm` 或 `forbidden-v1`。
2. `GA0-R002`：`am0-sensitive-field-canary-report.json` 的生成逻辑只读取 Canary 数量便直接报告 `allRejected=true`，没有逐项调用实际拒绝器；同时 `reject_sensitive_text` 只匹配少量标签字符串，不能拒绝不带 `api_key`/`Bearer` 标签的合成 Provider Key 形态。敏感内容不可持久化的证据链因此不成立。
3. `GA0-R003`：`estimatedUndoBytes` Schema 允许 0，`UndoCatalogV1::admits_auto_apply(0)` 也可返回 true，且授权决策没有与可用 Undo Record 强制绑定。当前契约不能证明每个 `auto-apply-safe` 写入都具备可恢复撤销。

产品负责人于 2026-08-28 明确回复“批准 GA0-R001～GA0-R003 修订；只授权修复 GA0 阻断项并重新提交 GA0 Review Gate”。本轮未获得、也未实施生产 Agent 运行时、AM1、AM2 或 Sprint 6 授权。

修订与独立复审结果：

1. `GA0-R001` **已关闭**：`project.create_markdown` 与 `project.patch_markdown` 不再拥有固定 `write-safe` 风险；`WriteToolClassificationV1` 将应用绑定的 Tool Request Hash、Project/Session/Turn、Tool ID、项目相对路径、已验证变更形态、Action、Risk 与 Classification Hash 强制绑定，并由事务 `actionPlanHash` 继续冻结。普通实体创建和 Draft 章节创建使用不同的已验证形态；项目规则、关键元数据、`materials/000.*`、整章替换、发布目标与隐藏配置路径均不能降级为普通安全写入。
2. `GA0-R002` **已关闭**：Canary Fixture 扩展到 8 类，新增两种无标签合成 Provider Key；E2E 生成器实际执行 `governance_contract_tests::sensitive_canary_fixture_is_rejected_by_rust_contract`，只有 Rust 拒绝器逐项通过时才报告 `allRejected=true`。
3. `GA0-R003` **已关闭**：每个 Mutation 和总事务 Undo 预算最小值均为 1；`UndoRecordV1` 新增 `transactionHash`；`UndoCatalogV1::admits_auto_apply` 必须接收已准备的非空 Ready Record，并验证同 Project/Session/Turn、同 Transaction、同 Clear Generation、完整目标与计划结果 Hash、容量和去重条件。

复审证据：最终 Governance Contract Set Hash 为 `a60b41ee0d16be3e035509076c7bdf110a506fb5d8c8203f1b0f948b72450f38`；31 项 `novelgainker-agent` 测试、严格 Clippy、8 项真实 Canary、完整 `pnpm quality`、G1～G5、Debug/Release 构建与桌面 Smoke 全部通过。App State 仍为 v8、Tauri Command 仍为 51，新增生产依赖、IPC、Migration、Capability、UI 入口、真实 Provider 请求、项目读取和项目写入均为 0。`target/qa/ng-a004/ga0-manifest.json` 当前为 `technical_review_passed_product_approval_required`，`blockingFindings=[]`，`resolvedFindings=[GA0-R001, GA0-R002, GA0-R003]`，且 `ga0Approved=false`。

产品批准记录：

“批准 GA0，授权后续 AM1 的所有，并且统一执行到 AM1 结束”

授权解释：允许创建并按推荐决策批准 AM1 Story Packet，连续实施 NG-A101～NG-A104 并提交 GA1 技术审查；不自动批准 GA1，不授权 AM2 或 Sprint 6。

## 17. 待确认决策

以下决策必须在 AM0 Story Packet Review Gate 通过前确认。

### AM0-Q001——上游来源锁

建议：以公开 `comfuture/novelist` 仓库 commit `64e4a2e1473ff0bad5893eff394ccbf1fa1cd580` 作为首个来源候选并在 NG-A001 开始时再次验证；禁止把本机缓存目录名 `0.2.0` 或浮动 `main` 当作发行版本。若 commit 内容或许可证无法复现，NG-A001 阻断并回到产品评审。

原因：当前缓存的版本元数据互相矛盾，且没有许可证正文；公开仓库当前也没有可用 tag。

状态：**已接受**（2026-08-28）。

### AM0-Q002——Pack 收录内容

建议：只收录九项 Skill 所需的指令、模板、参考资料和兼容 Fixture；排除宿主 Adapter、Agent YAML、网站、submission 元数据和 Python 脚本。需要的脚本语义以后移植为 Rust/声明式工具，并保留来源映射、MIT 许可和 Attribution。

原因：保持 release 自包含、最小、可审计，同时避免把 Codex/Claude/Antigravity 或 Python 运行时带入产品。

状态：**已接受**（2026-08-28）。

### AM0-Q003——Base Policy 与九 Skill

建议：统一会话的 `ProjectAgentBasePolicyV1` 是宿主策略，不作为第十项 Skill。一个 Turn 可选择零个、一个或多个九项 Skill；零 Skill 只用于讨论、澄清和产品帮助，但仍处于同一项目 Session 和权限审计中。

原因：既消除普通聊天旁路，又保持九项 Novelist 能力清单稳定。

状态：**已接受**（2026-08-28）。

### AM0-Q004——Turn 上限

建议：首版每 Turn 最多 8 Agent Step、16 Tool Call、24 个唯一文件、8 个修改文件、一个写事务和 96 KiB 单次 Provider 请求。超限后停止并建议拆分新 Turn，不后台继续。

原因：为成本、上下文、故障恢复和多文件原子性提供可测试上界。

状态：**已接受**（2026-08-28）。

### AM0-Q005——授权与风险矩阵

建议：项目内受控读为 `read-safe`；普通实体 Markdown create/局部 patch 和不存在目标的 Draft 章节创建为 `write-safe`；整文件/整章覆盖、accepted/final、状态/Ledger、规则/关键元数据、项目外导入和发布为 `always-confirm`；删除/移动/重命名、任意路径与系统能力在首版为 `forbidden-v1`。

原因：让 `auto-apply-safe` 真正支持持续写作，同时把不可逆或范围扩张动作留在明确确认或后续 Story。

状态：**已接受**（2026-08-28）。

### AM0-Q006——会话与 Undo 持久化

建议：默认保存 32 KiB 摘要、200 条消息索引/Turn 元数据、100 个终态 Turn；Undo 保留最近 20 个写入 Turn或 30 天，每 Turn 2 MiB、每项目 20 MiB。含正文的 Undo Blob 使用 Windows 用户上下文保护；完整聊天默认关闭且不能明文降级。

原因：满足跨重启连续协作与撤销，同时避免无界复制手稿和隐藏 Canon。

状态：**已接受**（2026-08-28）。

### AM0-Q007——Review Gate 的实施授权

建议：AM0 Packet Review Gate 通过时只授权开始 NG-A001；NG-A002～NG-A004 仍需等待各自前置 Story 的技术完成、产品验收和单独授权。GA0 通过后也只允许创建/审查 AM1 Story Packet，不自动授权 NG-A101。

原因：保持 Level 4 brownfield 迁移的逐 Story 风险控制。

状态：**已接受**（2026-08-28）。

## 18. 实施时可自行冻结的细节

在不改变上述产品边界时，对应 Story 可自行冻结：

- Rust 模块、Trait、枚举和稳定错误码名称。
- JSON Schema 文件拆分和 TypeScript 生成方式。
- Hash 清单的规范化 JSON 字段顺序。
- Pack 资源的内部目录名和只读加载 API。
- Mock Provider Fixture 的具体文本。
- Property Test 生成策略和随机种子。
- 证据报告 JSON 字段顺序。
- Windows 用户上下文保护的具体 Rust API 封装。

以下变化必须回到产品决策：

- 改用本机插件缓存、浮动分支或运行时下载 Skill。
- 收录没有许可/来源证据的文件。
- 加入 Python、Shell、外部 Agent Host 或用户 `SKILL.md` 运行时。
- 增加第十项产品 Skill、Multi-agent、Handoff 或并行 Writer。
- 放宽高风险确认、允许模型决定权限或允许跨 Provider Fallback。
- 允许删除/移动/重命名或任意文件系统能力。
- 无界持久化聊天、正文、Tool Result 或 Undo。
- 在 AM0 中接入生产 Agent UI、工具、数据库或网络路径。
- 跳过 NG-A001～NG-A004 的逐 Story 授权和产品验收。

## 19. AM0 Story Packet Review Gate

Review Gate 当前结论：**通过并关闭**（2026-08-28）。NG-A001～NG-A004、GA0-R001～GA0-R003 与 GA0 均已关闭；后续 AM1/GA1 已关闭，AM2 已完成 GA2 技术提交。GA2 产品验收与 Sprint 6 仍是独立授权门。

进入 NG-A001 前置条件（均已满足）：

- [x] AM0-Q001 已确认。
- [x] AM0-Q002 已确认。
- [x] AM0-Q003 已确认。
- [x] AM0-Q004 已确认。
- [x] AM0-Q005 已确认。
- [x] AM0-Q006 已确认。
- [x] AM0-Q007 已确认。
- [x] 产品负责人明确批准 AM0 Story Packet Review Gate。
- [x] 产品负责人只授权开始 NG-A001，未批量授权 NG-A002～NG-A004、AM1、AM2 或 Sprint 6。

建议批准语句：

“AM0-Q001～AM0-Q007 全部按建议接受，批准 AM0 Story Packet Review Gate，只授权开始 NG-A001”

上述决策已全部接受；后续若需变更，必须按问题编号修订 Packet 并重新经过对应 Gate。

## 20. 上游交接记录

- AR-Q001～AR-Q006 已于 2026-08-28 全部按建议接受。
- Architecture Amendment A-001 与修订后的 Sprint Plan 已通过。
- G0～G5 继续作为 as-built 历史和回归基线，不重开旧 Gate。
- Sprint 6 已暂停，现有 Sprint 6 Packet 不授权 NG-601。
- AM0～AM2 的顺序与 GA0～GA2 Gate 已获规划批准。
- 本 Packet 由“按步骤和流程开始迭代计划”的授权创建，并已通过 AM0 Review Gate；该 Gate 只授权了未接入生产入口的 NG-A001。
- 产品负责人于 2026-08-28 明确回复“AM0-Q001～AM0-Q007 全部按建议接受，批准 AM0 Story Packet Review Gate，只授权开始 NG-A001”。
- NG-A001 技术实现、完整质量门与产品验收已于 2026-08-28 通过；当前只授权实施 NG-A002，NG-A003 未授权。
- 产品负责人于 2026-08-28 明确回复“批准 NG-A001 产品验收；只授权开始 NG-A002”。
- NG-A002 技术实现、完整质量门与产品验收已于 2026-08-28 通过；当前只授权实施 NG-A003，NG-A004 未授权。
- 产品负责人于 2026-08-28 明确回复“批准 NG-A002 产品验收；只授权开始 NG-A003”。
- NG-A003 技术实现与完整质量门已于 2026-08-28 通过；当前等待独立产品验收，NG-A004 未授权。
- 产品负责人于 2026-08-28 明确回复“批准 NG-A003 产品验收；授权开始 NG-A004”。
- NG-A003 产品验收已于 2026-08-28 通过；当前只授权实施 NG-A004，GA0、AM1、AM2 与 Sprint 6 未授权。
- NG-A004 技术实现、证据与完整质量门已于 2026-08-28 通过；该时点等待独立产品验收。GA0 Manifest 仅为技术候选，`ga0Approved=false`，当时 GA0、AM1、AM2 与 Sprint 6 均未授权。
- 产品负责人于 2026-08-28 明确回复“批准 NG-A004 产品验收；只授权开始 GA0 Review Gate”。
- NG-A004 产品验收已于 2026-08-28 通过；GA0 Review Gate 首轮独立审查完成，完整质量门通过，但发现 `GA0-R001`～`GA0-R003` 三项阻断。GA0 未通过，修订、AM1、AM2 与 Sprint 6 均未授权。
- 产品负责人于 2026-08-28 明确回复“批准 GA0-R001～GA0-R003 修订；只授权修复 GA0 阻断项并重新提交 GA0 Review Gate”。
- GA0-R001～GA0-R003 已完成纯契约、Schema、Fixture、Golden Test 与证据修订；最终 Governance Contract Set Hash 为 `a60b41ee0d16be3e035509076c7bdf110a506fb5d8c8203f1b0f948b72450f38`，完整 `pnpm quality` 通过，GA0 技术复审无剩余阻断。
- 产品负责人于 2026-08-28 明确回复“批准 GA0，授权后续 AM1 的所有，并且统一执行到 AM1 结束”。GA0 因此关闭；AM1 随后连续实施到 GA1 技术提交并通过完整质量门，GA1 又获独立产品批准。后续 AM2 已完成 NG-A201～NG-A206 与 GA2 技术提交；当前 `ga2Approved=false`，Sprint 6 仍未授权。
