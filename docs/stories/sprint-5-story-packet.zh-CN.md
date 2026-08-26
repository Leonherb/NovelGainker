# NovelGainker Sprint 5 Story Packet

> 状态：Sprint 5 Story Packet Review Gate 已通过；NG-501 已完成技术实现并通过全量质量门，等待产品验收；NG-502～NG-507 未授权
>
> 日期：2026-08-26
>
> BMAD 数据包：execution-packet
>
> Sprint：5——Storytelling Core
>
> 范围：NG-501～NG-507
>
> 团队：1 人，严格顺序实施

## 1. Sprint 目标

Sprint 5 将现有 NovelGainker 工作台从“可选择工作流、可对话、可预览项目文件”推进到第一条真正受控的小说创作闭环：

1. 从 Canon、目标章节大纲、最近章节、Story Ledger 与用户补充中构建可解释上下文。
2. 让用户在调用 DeepSeek 或 OpenAI 前审查上下文清单、预算与来源。
3. 生成结构化 Chapter Proposal，而不是直接覆盖 Markdown。
4. 对提案执行本地、确定性的连续性检查。
5. 以差异预览和显式确认写入章节文件。
6. 章节被用户接受或定稿后，另行确认更新 Story Ledger。
7. 在两种 Provider 上证明相同的状态机、取消语义和本地写入边界。

Sprint 5 完成后，用户应能够在桌面端完成“选择项目上下文 → 生成章节提案 → 检查 → 审阅差异 → 写入 Markdown → 手动修改 → 接受章节 → 更新 Ledger”的完整路径。

## 2. G5 产品演示脚本

G5 演示按以下顺序执行，任何一步都不能依赖隐藏写入：

1. 打开一个有效的 NovelGainker 小说仓库。
2. 选择 Novel Storytelling 工作流。
3. 选择“创建章节”。
4. 输入章节号、标题和章节简报。
5. 展开 Context Manifest。
6. 查看固定规则文件、默认候选、显式选中项和被排除项。
7. 取消一个可选上下文源。
8. 观察预算、内容哈希与 Manifest 同步变化。
9. 选择 DeepSeek 并生成 Chapter Proposal。
10. 在流式生成中验证可取消。
11. 取消后确认没有章节文件、临时文件或 Ledger 变化。
12. 重新生成并完成提案。
13. 查看提案来源、Provider、模型、请求哈希和生成时间。
14. 运行本地连续性检查。
15. 查看阻断、警告、未知三类结果。
16. 在存在阻断项时验证“写入章节”不可用。
17. 修订提案或明确处理问题后重新检查。
18. 展开章节差异预览。
19. 明确确认章节写入。
20. 验证目标 Markdown 原子落盘且通过章节校验。
21. 直接手动编辑 Markdown，保存并重新预览。
22. 将章节状态改为 accepted。
23. 验证状态变更有独立差异与确认。
24. 查看“Ledger 待更新”状态。
25. 单独生成 Ledger 更新提案。
26. 查看 Ledger 差异。
27. 单独确认 Ledger 写入。
28. 切换 OpenAI，以相同输入重复生成路径。
29. 比较两个 Provider 的协议形态、错误显示和取消行为。
30. 验证最终项目没有静默写入、孤立临时文件或未解释的状态变化。

## 3. 范围

### 3.1 包含

- Novel Storytelling 工作流的首个可执行版本。
- 创建章节与修订章节两种入口。
- Context Builder v2 与 Context Manifest。
- 确定性预算、截断与来源优先级。
- Reader Mental Model 摘要区。
- Provider 中立的 Chapter Proposal 协议。
- 本地连续性检查。
- 章节差异预览、事务性写入、校验与恢复。
- draft、accepted、final 状态迁移。
- accepted 或 final 后的独立 Story Ledger 更新提案。
- DeepSeek 与 OpenAI 的等价 Loopback E2E。
- 用户主动授权情况下的可选 Live Smoke。

### 3.2 不包含

- 自动多章生成。
- 后台自主 Agent 循环。
- 自动批准章节或自动更新 Ledger。
- 第二次隐藏 AI 审校。
- 语义向量检索、Embedding、RAG 数据库。
- 离线模型。
- 云端同步、远程伴侣或移动端实现。
- Git 仓库创建、提交、回滚或分支管理。
- EPUB 发布与全书分析，它们属于后续 Sprint。
- Provider 自定义 Base URL、模型名或系统提示词。
- Sprint 6 的高级连续性、分析审校与出版能力。

## 4. 继承自 G4 的硬边界

1. 工作流和资源来自已验证的内置只读 Workflow Pack。
2. Pack Registry、Pack Manifest 与资源哈希仍是唯一运行时来源。
3. 工作流注册成功不等于允许执行；执行能力必须逐 Story 解锁。
4. 文件读取限定在当前用户打开的项目根目录。
5. 写入只允许用户审阅后的显式目标文件。
6. 对话不会自动读取项目、文件、选区或搜索结果。
7. AI Provider 仅允许 DeepSeek 与 OpenAI，用户只填写 API Key。
8. 所有模型和端点仍由应用内置。
9. 项目编辑以 Markdown 为事实来源，用户可随时手动修改。
10. Git 完全由用户自行管理，应用不执行 Git 操作。
11. 简体中文为界面、默认写作语言与内置提示词语言。
12. 不引入 Novelist 插件运行时依赖；NovelGainker 维护自己的 Pack、协议与实现。
13. 取消、崩溃、校验失败不能留下半写文件。
14. 任一新 IPC 都必须经过边界注册、参数校验和前端白名单。

## 5. Storytelling Pack 与激活规则

### 5.1 公共工作流

- 工作流 ID：novelgainker.storytelling
- 显示名称：小说续写与章节创作
- Pack 版本：v1
- 支持动作：create、revise、ledger-update
- 首次进入时默认动作：create

在 NG-501～NG-504 完成前，该工作流仍只可查看。NG-505 完成并通过 Story 验收后，create 与 revise 才可标记为 executionAvailable。ledger-update 只有 NG-506 通过后才可执行。

### 5.2 能力声明

工作流只能声明以下内部能力：

- story.build_context
- story.check_continuity
- story.apply_chapter_patch
- story.update_ledger

前端不得依据工作流名称自行推断权限；所有能力由后端注册表逐项批准。

### 5.3 章节事实来源

- 章节目录固定为 chapters/。
- 可发布正文只取章节 Markdown 的“Draft”二级标题内容。
- Story Ledger 固定为 materials/000.story-ledger.md。
- 项目全局规则来自 project.md、AGENTS.md 和 Pack 内置资源。
- 工作流不得在项目中复制或改写内置 Pack 资源。

## 6. 章节身份、路径和 Markdown 契约

### 6.1 创建章节

用户必须提供：

- 章节号：1～999999 的正整数。
- 章节标题：去除首尾空白后非空。
- 章节简报：允许为空，但界面要明确提示上下文将更少。

系统生成：

- id：chapter_ 前缀加 UUID。
- kind：chapter。
- schemaVersion：当前章节 Schema 版本。
- writingLanguage：默认 zh-CN。
- status：draft。
- chapterNumber：用户输入的数值。

路径使用至少三位补零：

- 1 → chapters/001.md
- 12 → chapters/012.md
- 1000 → chapters/1000.md

若路径已存在，创建动作立即阻断；第一版不自动寻找下一个编号。

### 6.2 修订章节

- 用户从现有 chapters/*.md 中选择目标。
- 保留 id、chapterNumber 与路径。
- 默认只修改“Draft”区域。
- frontmatter、非 Draft 区块和未知扩展字段默认保持原样。
- accepted 或 final 章节进入修订时，必须先明确解锁。
- 修订写入后状态回到 draft，不能保留 accepted 或 final 假象。

### 6.3 最小章节结构

章节文件必须包含：

- YAML frontmatter。
- 非空 title。
- writingLanguage。
- id、kind、schemaVersion、chapterNumber、status。
- 唯一且非空的“Draft”二级标题。

章节写入后必须重新读取并运行章节校验；校验失败触发恢复流程。

## 7. Context Builder v2

### 7.1 Context Manifest

Context Builder 输出稳定、可序列化的 Manifest，至少包含：

- manifestVersion
- workflowId
- action
- projectRootIdentity
- targetChapter
- selectedSources
- excludedSources
- sourceOrder
- perSourceBytes
- totalBytes
- budget
- truncationRecords
- contentHashes
- aggregateHash
- createdAt

每个来源条目至少包含：

- sourceId
- kind
- relativePath 或 builtinResourceId
- selectionReason
- required
- userDeselectable
- priority
- byteLength
- sha256
- truncation
- included

Manifest 必须能在 UI 中展开查看；不得只显示“已加载若干文件”。

### 7.2 来源层级

优先级从高到低：

1. 必需的 Pack 协议与 Schema。
2. project.md。
3. AGENTS.md。
4. 工作流自身规则。
5. 用户显式选择的 Canon。
6. 目标章节大纲。
7. 主情节 plot/main.md。
8. 主大纲 outlines/main.md。
9. 写作风格 style/main.md。
10. Story Ledger。
11. 最近章节 Draft。
12. 用户本次补充和查询结果。

“显式选择”始终高于同级默认候选；必需来源不可取消，其他来源可由用户取消。

### 7.3 默认候选与显式控制

默认勾选但允许取消：

- plot/main.md
- outlines/main.md
- style/main.md
- materials/000.story-ledger.md
- 最近两个有效章节的 Draft

用户可以额外选择：

- world/
- characters/
- materials/
- plot/
- outlines/
- style/
- 其他 chapters/ 文件

系统不得扫描后静默加入所有匹配文件。搜索结果必须由用户明确加入 Manifest。

### 7.4 快照与变更检测

生成前冻结 Manifest 快照。Provider 调用期间若任何已选文件的哈希变化：

- 本次响应可继续接收，但标记为 source-stale。
- 不能直接进入写入确认。
- 用户必须重新构建 Context 或明确重新生成。

## 8. 预算与 Reader Mental Model

### 8.1 固定预算

Provider 硬请求上限沿用 96 KiB；Sprint 5 固定分配：

- 协议、Schema、系统规则、章节简报：24 KiB。
- 项目上下文：48 KiB。
- 安全余量：24 KiB。
- 单一项目来源上限：8 KiB。
- 最近章节：最多 2 个。
- 用户查询结果：最多 5 条，每条最多 2 KiB。
- 最大输出：4096 tokens。

预算按 UTF-8 字节计算。截断必须在 Manifest 中记录原始长度、保留长度与原因。

### 8.2 截断策略

1. 先按来源优先级排序。
2. 必需协议与 Schema 不截断到不可解析。
3. Markdown 优先保留标题结构与与目标章节最相关的连续段落。
4. 低优先级来源先减少，后排除。
5. 任何排除和截断都要可见。
6. 第一版不调用隐藏 AI 生成摘要。
7. 仅可使用项目中已经存在且用户可见的 Story Ledger 作为摘要来源。

### 8.3 Reader Mental Model

生成请求中必须包含一个结构化阅读者心智模型：

- readerKnows：读者已明确知道的事实。
- readerInfers：读者被引导推断但尚未确认的内容。
- readerMisreads：当前允许或预期的误读。
- readerQuestions：本章开始前仍悬而未决的问题。
- chapterPromises：本章应推进或兑现的承诺。
- forbiddenReveals：本章不能提前揭露的事实。

该模型由用户简报、目标大纲、Ledger 与选中上下文组合得出，并在请求前可见、可编辑；编辑结果进入请求哈希。

## 9. Chapter Proposal 与 Provider 契约

### 9.1 Proposal 结构

Provider 输出必须经过统一解析，至少得到：

- proposalVersion
- action
- targetChapter
- title
- draftMarkdown
- intentSummary
- readerModelDelta
- continuityClaims
- ledgerCandidates
- sourceManifestHash
- requestHash
- providerId
- modelId
- generatedAt
- finishReason

仅 draftMarkdown 可进入章节差异；其他字段用于解释、连续性检查和后续 Ledger 提案。

### 9.2 请求与响应边界

- DeepSeek 固定模型：deepseek-v4-pro。
- OpenAI 固定模型：gpt-5.6-terra。
- 用户只提供 API Key。
- Provider Adapter 负责将统一请求转换为各自协议。
- UI 不感知端点、鉴权 Header 或 Provider 原始事件。
- 原始响应不得直接写入项目。
- JSON 或结构化输出解析失败时，只显示可恢复错误和原始文本预览；写入保持禁用。
- finishReason 为 length、cancelled、error 或 safety 时，不得直接进入写入。
- 请求超过 96 KiB 时必须在本地阻断，不得尝试发送。

### 9.3 可追溯信息

提案界面应显示：

- 当前 Provider 与固定模型。
- Context Manifest 哈希。
- 请求哈希。
- 生成开始和完成时间。
- 响应是否完整。
- 来源是否在生成期间变化。
- 连续性检查版本。

API Key、完整鉴权 Header 与敏感错误体不得进入日志或提案元数据。

## 10. 连续性检查 v1

### 10.1 原则

Sprint 5 使用本地、确定性的检查器；不会秘密再次调用 AI。检查结果分为：

- blocking：必须修复或改变提案后重跑，禁止写入。
- warning：允许用户在了解风险后继续。
- unknown：当前规则无法判断，明确留给作者决定。
- passed：规则已验证通过。

### 10.2 阻断检查

- 章节号或路径冲突。
- Chapter Proposal 与目标动作、目标章节不一致。
- 缺失或重复“Draft”区块。
- Draft 为空。
- frontmatter 无法形成有效章节。
- Manifest 哈希与生成时快照不一致。
- 目标文件在预览后发生变化。
- Provider 响应不完整。
- 包含越界绝对路径或非项目相对路径。
- final 章节未解锁便尝试修订。

### 10.3 警告检查

- 标题与目标大纲标题不一致。
- 提案声称新增角色、地点、物件或规则，但未在上下文中找到对应来源。
- Ledger 中存在未解决承诺，而本章没有推进。
- 章节长度显著偏离最近章节。
- forbiddenReveals 中的关键词可能出现在 Draft。
- Reader Mental Model 的 readerKnows 与本章开头存在潜在倒退。
- 本章可能提前兑现未来大纲条目。

### 10.4 Unknown 与可解释性

语义是否真正矛盾通常无法由 v1 规则可靠判断，因此显示 unknown，而不是伪装成 passed。每条结果包含：

- ruleId
- severity
- title
- explanation
- evidence
- sourceIds
- suggestedAction

用户不能通过一个全局“忽略所有”绕过 blocking；warning 可以逐条确认或统一确认，但确认记录仅存在于本次写入会话。

## 11. 差异、写入、状态与 Ledger

### 11.1 章节写入事务

1. 冻结 Proposal、Manifest、连续性结果和目标文件哈希。
2. 生成内存中的目标 Markdown。
3. 展示逐行差异。
4. 用户明确确认。
5. 在同目录创建唯一临时文件。
6. 写入、flush 并关闭临时文件。
7. 对临时文件运行章节校验。
8. 重新确认原目标哈希未变化。
9. 原子替换或原子重命名。
10. 重新读取目标并验证内容哈希。
11. 删除临时文件。
12. 返回带 transactionId 的结果。

任何失败必须恢复原文件；无法自动恢复时，应用保留只读诊断信息并禁止继续写入。

### 11.2 状态迁移

允许：

- draft → accepted
- accepted → draft（显式解锁修订）
- accepted → final
- final → draft（显式解锁修订）

不允许：

- draft → final
- AI 自动修改状态
- 章节写入与状态接受在同一确认中完成

accepted 和 final 状态变更都需要独立差异预览和确认。

### 11.3 Story Ledger 更新

- 仅 accepted 或 final 章节可以产生 Ledger 更新提案。
- 章节状态写入完成后，显示 ledgerPending。
- Ledger 提案从已接受章节、当前 Ledger 和对应 Context Manifest 生成。
- Ledger 差异必须单独预览、单独确认、单独事务写入。
- Ledger 失败不回滚已接受章节。
- Ledger 未更新时保持可见的 pending 状态。
- 用户可以选择暂不更新，不得自动后台重试。
- 更新目标固定为 materials/000.story-ledger.md。

## 12. NG-501——Context Builder v2 与 Manifest

### 12.1 用户价值

作者能看见 AI 将要读取什么、为什么读取、占用多少预算，并能控制所有非必需来源。

### 12.2 实施内容

- 新增 Context Builder 后端服务和只读 IPC。
- 定义 Manifest、来源条目、预算和截断类型。
- 实现固定来源发现、默认候选和显式选中。
- 实现项目内路径规范化和扩展名白名单。
- 实现 UTF-8 字节预算、单源上限和稳定排序。
- 实现每项 SHA-256 与 aggregateHash。
- 在工作流面板提供来源选择、预算条和 Manifest 详情。
- 任何来源变化时重新构建哈希。
- 加入 Fixture 与属性测试覆盖顺序、截断和越界路径。

### 12.3 验收标准

- [x] 相同项目快照和选择产生字节级稳定的 Manifest。
- [x] 必需来源可见但不可取消。
- [x] 默认候选可以取消，额外来源必须显式加入。
- [x] 不会静默加载整个仓库。
- [x] 超预算来源按规则截断或排除并说明原因。
- [x] 所有路径都在项目根目录内。
- [x] 任一文件变化都会改变相应哈希和 aggregateHash。
- [x] UI 可展开查看每个来源的理由、长度和哈希。
- [x] NG-501 只构建上下文，不调用 Provider、不写项目文件。
- [x] 单元、Fixture、边界与格式检查通过。

### 12.4 非目标

- 不生成章节。
- 不开放 Storytelling 执行能力。
- 不写入缓存索引或数据库。
- 不实现语义搜索。

### 12.5 证据

- Context Fixture 报告。
- 路径越界和符号链接测试。
- Manifest 稳定性快照。
- UI 截图。
- 质量命令摘要。

### 12.6 技术完成记录（2026-08-26）

- 已加入只读、不可路由、不可执行的 `novelgainker.storytelling` Workflow；固定 Pack 现含 6 个 Workflow、28 个编译期资源，规范化 Pack Hash 为 `24616ecda847c1df3f3e9b4550e70ef987b8b2c62e7cfe0b3985680721ce84f9`。
- Context Builder v2 通过项目会话和受控 App Core 读取来源，必需来源、默认候选、显式额外 Canon、稳定排序、UTF-8 截断、48 KiB 上下文预算、8 KiB 单源上限、来源 Hash 与 aggregateHash 均由 Rust 确定性实现。
- 中栏 Storytelling 卡已接入只读 Context Builder；用户可取消默认来源、显式加入额外来源，并展开查看理由、字节数、截断/排除状态和 Hash。该入口不调用 Provider，也不开放章节生成。
- `pnpm quality` 已以退出码 0 全量通过：23 个前端测试文件/129 项测试、59 项 Workflow 测试、87 项 App Core 测试（4 项按设计忽略）、28 项 Tauri 测试，以及 Sprint 1～4、G4 和 NG-501 E2E。
- NG-501 脱敏证据位于 `target/ng501-evidence/context-fixture-report.json`；报告确认 `providerRequestCount=0`、`projectWriteCount=0`、`pathEscapeReadCount=0`、`stableManifestMismatchCount=0`。

当前结论：**NG-501 技术验收通过，等待产品负责人产品验收；NG-502 尚未授权。**

## 13. NG-502——预算与 Reader Mental Model 编辑

### 13.1 用户价值

作者能在发送请求前理解上下文取舍，并明确控制读者此刻知道、误解和期待什么。

### 13.2 实施内容

- 实现 24/48/24 KiB 固定预算分区。
- 实现单源 8 KiB、最近章节 2 个、查询结果 5×2 KiB 限制。
- 显示预算使用、排除和截断记录。
- 提供六段 Reader Mental Model 编辑器。
- 将用户修改后的模型纳入 Context 快照和请求哈希。
- 在来源变化时使旧模型标记为需复核，而非静默覆盖。
- 补充中文默认说明和空状态。

### 13.3 验收标准

- [ ] 预算计算以 UTF-8 字节为准。
- [ ] 96 KiB 上限在调用 Provider 前本地执行。
- [ ] 截断顺序可重复、可解释。
- [ ] Reader Mental Model 六个字段均可查看和编辑。
- [ ] 用户编辑会改变请求哈希。
- [ ] 来源变化不会丢弃用户编辑，而会提示重新确认。
- [ ] 不调用隐藏 AI 生成摘要。
- [ ] 不写项目文件。
- [ ] NG-501 回归与质量检查通过。

### 13.4 证据

- 边界预算测试。
- 中文、多字节字符预算 Fixture。
- Mental Model 状态测试。
- UI 截图和请求哈希证明。

## 14. NG-503——Provider 中立的 Chapter Proposal

### 14.1 用户价值

作者可以用当前选中的 DeepSeek 或 OpenAI 生成可审查的章节提案，且生成本身不会改动项目。

### 14.2 实施内容

- 定义统一 Chapter Proposal 请求与响应 Schema。
- 为 DeepSeek、OpenAI 复用现有流式 Provider Adapter。
- 将 Context Manifest 与 Reader Mental Model 编译为固定中文提示协议。
- 加入响应解析、完整性判断和可恢复错误。
- 显示 Provider、模型、Manifest 哈希、请求哈希和完成原因。
- 支持取消、重试和来源过期状态。
- 在生成完成前保持所有写入操作禁用。

### 14.3 验收标准

- [ ] 两个 Provider 接收语义等价的统一请求。
- [ ] 用户只填写 API Key，不出现自定义端点或模型控件。
- [ ] 取消后无章节、Ledger、临时文件或隐藏状态写入。
- [ ] 不完整或不可解析响应不能进入写入。
- [ ] source-stale 提案不能进入写入。
- [ ] Proposal 保存全部非敏感追溯字段。
- [ ] API Key 不出现在日志、错误、Fixture 或快照中。
- [ ] Loopback 能模拟流式、取消、长度截断和错误。
- [ ] NG-501～NG-502 回归与质量检查通过。

### 14.4 证据

- 两 Provider Loopback 传输报告。
- 取消与错误恢复测试。
- 请求 Schema 快照。
- 无文件变化证明。

## 15. NG-504——本地连续性检查 v1

### 15.1 用户价值

作者在写入前得到可解释的结构和连续性风险清单，不会被一个不透明的“AI 评分”替代判断。

### 15.2 实施内容

- 新增 story.check_continuity 后端能力与只读 IPC。
- 实现 blocking、warning、unknown、passed 四类结果。
- 实现章节结构、Manifest 哈希、文件哈希和响应完整性检查。
- 实现 Ledger 承诺、目标大纲标题、长度偏差和 forbiddenReveals 启发式检查。
- 为每条结果提供证据来源和建议动作。
- 在存在 blocking 时保持写入入口禁用。
- 允许 warning 逐项查看并显式确认。
- 在界面明确标注“本地规则检查，未再次调用 AI”。

### 15.3 验收标准

- [ ] 相同输入产生稳定排序和稳定 ruleId。
- [ ] blocking 始终阻止写入。
- [ ] warning 必须在写入会话内得到明确确认。
- [ ] unknown 不被计为通过。
- [ ] 每条结果具有解释、证据和来源。
- [ ] 连续性检查不调用 Provider。
- [ ] 检查不写项目文件。
- [ ] source-stale 或 target-stale 成为 blocking。
- [ ] NG-501～NG-503 回归与质量检查通过。

### 15.4 证据

- 规则矩阵 Fixture。
- blocking 与 warning UI 截图。
- 无 Provider 调用证明。
- 稳定排序测试。

## 16. NG-505——章节差异与事务性写入

### 16.1 用户价值

作者能先看到准确差异，再用一次明确确认安全创建或修订章节；失败不会破坏原文件。

### 16.2 实施内容

- 新增 story.apply_chapter_patch 能力。
- 实现创建与修订的内存目标文档构建器。
- 实现逐行差异预览和目标哈希冻结。
- 实现同目录临时文件、flush、校验、原子替换与回读校验。
- 实现失败恢复和孤立临时文件清理。
- 实现创建路径冲突、修订期间外部变化阻断。
- 实现章节状态 Schema，但本 Story 只写 draft。
- NG-505 验收后解锁 create、revise 的 executionAvailable。

### 16.3 验收标准

- [ ] 创建只写用户输入章节号对应的单一目标文件。
- [ ] 路径存在时阻断，不自动覆盖或重编号。
- [ ] 修订默认只改变 Draft，保留未知 frontmatter 和其他区块。
- [ ] 用户确认前项目零写入。
- [ ] 确认时再次校验 Proposal、Manifest、连续性和目标哈希。
- [ ] 写入采用同目录事务流程。
- [ ] 校验失败恢复原文件且不留临时文件。
- [ ] 写入结果回读哈希与预期一致。
- [ ] 创建与修订均有差异预览。
- [ ] 只在 NG-505 验收后开放工作流执行入口。
- [ ] NG-501～NG-504 回归与完整质量检查通过。

### 16.4 失败恢复矩阵

| 失败点 | 原目标 | 临时文件 | UI 结果 |
| --- | --- | --- | --- |
| 确认前取消 | 不变 | 无 | 返回提案 |
| 临时写入失败 | 不变 | 清理 | 可重试 |
| 临时校验失败 | 不变 | 清理 | 展示校验错误 |
| 目标哈希改变 | 外部版本保留 | 清理 | 要求重新预览 |
| 原子替换失败 | 不变或恢复备份 | 清理 | 阻断并诊断 |
| 回读哈希不符 | 恢复原文件 | 清理 | 阻断并诊断 |
| 应用崩溃 | 原目标优先保留 | 下次启动清理已识别临时文件 | 恢复提示 |

### 16.5 证据

- 创建与修订 E2E。
- 故障注入恢复报告。
- 临时文件清理测试。
- 差异和确认 UI 截图。
- 完整质量命令摘要。

## 17. NG-506——章节状态与独立 Ledger 更新

### 17.1 用户价值

作者可以明确接受或定稿章节，并在另一次审阅中决定是否把新事实写入全书 Story Ledger。

### 17.2 实施内容

- 实现 draft、accepted、final 状态迁移守卫。
- 实现 accepted 与 final 的独立差异确认。
- 实现显式解锁修订和状态回退到 draft。
- accepted 或 final 后创建 ledgerPending 状态。
- 定义 Ledger Update Proposal Schema。
- 生成固定目标 Ledger 的差异。
- 新增 story.update_ledger 能力与事务性写入。
- Ledger 写入复用校验、哈希冻结、原子替换和恢复机制。
- ledger-update 只有本 Story 验收后才解锁。

### 17.3 验收标准

- [ ] AI 不能自动接受或定稿章节。
- [ ] draft 不能直接进入 final。
- [ ] 状态变更与章节正文写入不是同一次确认。
- [ ] accepted 或 final 修订前必须明确解锁。
- [ ] 修订写回后状态为 draft。
- [ ] Ledger 更新永远是单独提案、单独差异、单独确认。
- [ ] Ledger 失败不回滚已接受章节。
- [ ] 未更新 Ledger 时 pending 状态可见且可恢复。
- [ ] 暂不更新不会触发后台重试。
- [ ] 目标只能是 materials/000.story-ledger.md。
- [ ] NG-501～NG-505 回归与完整质量检查通过。

### 17.4 证据

- 状态迁移表测试。
- 两次确认的 E2E 录像或截图。
- Ledger 故障注入报告。
- pending 恢复测试。
- 完整质量命令摘要。

## 18. NG-507——双 Provider Storytelling E2E 与 G5 证据

### 18.1 用户价值

作者得到一条在 DeepSeek 与 OpenAI 上行为一致、可以取消、可解释且不会静默写文件的创作闭环。

### 18.2 实施内容

- 建立 DeepSeek Loopback Storytelling E2E。
- 建立 OpenAI Loopback Storytelling E2E。
- 覆盖创建、修订、取消、响应截断、Provider 错误和来源过期。
- 覆盖章节写入、手动编辑、状态接受、Ledger pending 和 Ledger 写入。
- 对比两个 Adapter 的统一 Proposal 协议和错误映射。
- 生成 G5 Manifest、哈希和 Review Report。
- 用户明确授权时运行一个最小 Live Smoke；Live 不向项目写入正文。
- 记录隐私、Key 脱敏和日志检查结果。

### 18.3 验收标准

- [ ] 两 Provider 的 Loopback 都走同一个 Context、Proposal、Continuity 和 Apply 状态机。
- [ ] 两 Provider 均证明流式取消后零项目写入。
- [ ] 两 Provider 均证明截断或解析失败不能写入。
- [ ] 创建和修订章节通过。
- [ ] 手动 Markdown 编辑不会被旧 Proposal 静默覆盖。
- [ ] 状态接受和 Ledger 更新保持两次确认。
- [ ] 完整路径可按第 2 节脚本演示。
- [ ] G5 报告列出 Manifest、请求、输出、章节和 Ledger 哈希。
- [ ] 日志与证据不含 API Key。
- [ ] Live Smoke 仅在用户授权时运行；未运行不会使 Loopback 证据失效。
- [ ] 完整质量门通过。

### 18.4 证据

- DeepSeek Loopback 报告。
- OpenAI Loopback 报告。
- 跨 Provider 协议对比。
- 取消、错误和 source-stale 报告。
- 章节事务与 Ledger 事务报告。
- G5 Review Report 与校验清单。
- 可选 Live Smoke 记录。

## 19. 跨 Story 依赖与授权矩阵

| Story | 依赖 | 首次开放的能力 | 可写项目 | 产品验收后下一步 |
| --- | --- | --- | --- | --- |
| NG-501 | G4 | story.build_context（只读） | 否 | NG-502 |
| NG-502 | NG-501 | 预算与 Reader Model（只读） | 否 | NG-503 |
| NG-503 | NG-502、现有 Provider 层 | Chapter Proposal（只读） | 否 | NG-504 |
| NG-504 | NG-503 | story.check_continuity（只读） | 否 | NG-505 |
| NG-505 | NG-504 | story.apply_chapter_patch | 仅目标章节 | NG-506 |
| NG-506 | NG-505 | 状态迁移、story.update_ledger | 目标章节或固定 Ledger；分开确认 | NG-507 |
| NG-507 | NG-506 | 无新能力；完成双 Provider 证据 | 仅按既有确认路径 | G5 |

严格规则：

1. 每个 Story 仍需独立实施、测试、证据和产品验收。
2. 后一 Story 不因本 Packet 获批而自动授权。
3. NG-505 前禁止任何 Storytelling 项目写入。
4. NG-506 前禁止 Ledger 写入。
5. NG-507 不增加运行时权限，只补齐跨 Provider 和 G5 证据。
6. G5 未通过前不得创建或启动 Sprint 6 实施工作。

## 20. G5 Review Gate

### 20.1 阻断项

- [ ] NG-501～NG-507 均有产品验收记录。
- [ ] Context Manifest 对全部来源、排除、截断、预算和哈希可解释。
- [ ] 用户可修改所有非必需上下文选择。
- [ ] Reader Mental Model 可见、可编辑并进入请求哈希。
- [ ] Provider 调用前本地执行 96 KiB 上限。
- [ ] DeepSeek 与 OpenAI 走统一 Proposal 状态机。
- [ ] 取消、截断、错误和 source-stale 均证明零项目写入。
- [ ] 连续性检查不会静默调用第二次 AI。
- [ ] blocking 不能被忽略并写入。
- [ ] 章节创建和修订都有准确差异预览。
- [ ] 用户确认前没有章节写入。
- [ ] 写入具备校验、哈希冻结、原子替换与恢复。
- [ ] 手动编辑后的文件不会被旧提案覆盖。
- [ ] 章节状态由用户独立确认。
- [ ] Ledger 更新只在 accepted 或 final 后提出。
- [ ] Ledger 更新有第二次独立差异和确认。
- [ ] Ledger 失败不破坏已接受章节，并保留 pending。
- [ ] G5 Manifest 与 Review Report 可重复校验。
- [ ] 证据和日志不含 API Key。
- [ ] 完整质量命令通过。

### 20.2 G5 非阻断信息项

- Live Smoke 可在用户授权后执行；无授权时以 Loopback 双 Provider 证据完成 G5。
- 第一版对深层语义矛盾可给出 unknown，不要求伪造确定答案。
- 用户可选择暂不更新 Ledger，只要 pending 状态清晰且可恢复。
- Windows 安装签名、自动更新、移动端和远程功能不属于 G5。

### 20.3 G5 产物

- g5-manifest.json
- g5-review-report.md
- deepseek-storytelling-loopback-report.json
- openai-storytelling-loopback-report.json
- chapter-transaction-report.json
- ledger-transaction-report.json
- cancellation-and-recovery-report.json
- 脱敏截图或录像
- 可选 live-smoke-report.json

## 21. 待确认决策

以下决策均为 Sprint 5 Review Gate 的一部分；在用户接受前不得启动 NG-501。

### S5-Q001——工作流暴露方式

建议：第一版只暴露一个 novelgainker.storytelling v1 工作流，在内部提供 create、revise、ledger-update 三种动作。NG-505 前不标记 create 或 revise 可执行；NG-506 前不标记 ledger-update 可执行。

原因：保持左侧工作流入口简洁，同时不让 UI 展示状态误导实际权限。

状态：已接受（2026-08-26）。

### S5-Q002——章节编号、身份和状态

建议：章节号由用户明确输入 1～999999；路径至少三位补零；存在冲突时阻断，不自动重编号。章节 id 使用 chapter_ 加 UUID。修订保留 id、编号和路径；accepted 或 final 修订后回到 draft。final 必须先解锁，draft 不能直接 final。

原因：用户对仓库结构有最终控制，也避免隐式编号与状态假象。

状态：已接受（2026-08-26）。

### S5-Q003——默认上下文

建议：project.md、AGENTS.md、工作流协议和 Schema 为必需来源；plot/main.md、outlines/main.md、style/main.md 默认勾选但可取消；目标大纲和额外 Canon 由用户显式选择；Ledger 与最近两个 Draft 作为规则候选默认勾选但可取消。

原因：在“足够帮助写作”和“用户知道 AI 读了什么”之间保持可控平衡。

状态：已接受（2026-08-26）。

### S5-Q004——上下文预算

建议：请求硬上限 96 KiB；协议、Schema、简报 24 KiB，上下文 48 KiB，安全余量 24 KiB；单来源 8 KiB；最近章节 2 个；查询结果最多 5 条、每条 2 KiB；输出上限 4096 tokens。第一版不做隐藏 AI 摘要。

原因：与现有 Provider 边界一致，并确保请求可重复、可测试、可解释。

状态：已接受（2026-08-26）。

### S5-Q005——连续性检查深度

建议：Sprint 5 只做本地确定性和启发式规则，不做第二次隐藏 AI 调用。不能可靠判断的深层语义问题显示 unknown；高级 AI 审校留到 Sprint 6。

原因：避免成本、延迟和不可解释性，也避免把概率判断伪装成确定事实。

状态：已接受（2026-08-26）。

### S5-Q006——章节接受与 Ledger 更新

建议：采用两个独立事务和两次确认。先由用户确认章节状态进入 accepted 或 final，再创建 Ledger 更新提案；用户审阅第二份差异后才写 Ledger。Ledger 失败不回滚章节，而保留 ledgerPending。

原因：章节正文和全书状态是两个不同事实边界，必须能独立恢复。

状态：已接受（2026-08-26）。

### S5-Q007——G5 Provider 证据

建议：G5 的阻断证据使用 DeepSeek 与 OpenAI 的确定性 Loopback E2E；真实 Live Smoke 只有在用户明确授权后运行，作为附加证据。真实线上兼容性与发布前认证继续留在 NG-703。

原因：避免因外部账户、网络或模型变化阻断本地产品门，同时仍完整验证两个 Adapter。

状态：已接受（2026-08-26）。

## 22. 实施时可自行冻结的细节

以下细节不改变产品边界时，可由实现 Story 直接冻结并记录，无需单独产品决策：

- Rust 类型和模块名称。
- IPC 命令的精确命名，只要符合能力白名单。
- UI 组件拆分和 CSS 细节。
- Manifest JSON 字段的额外诊断项。
- 临时文件随机后缀格式。
- Diff 库选择。
- SHA-256 实现库。
- 测试 Fixture 的人物和剧情内容。
- 连续性 ruleId 的命名规范。
- G5 报告内部字段排序。

以下变化不能自行冻结，必须回到产品决策：

- 增加自动文件读取范围。
- 增加自动写入或后台写入。
- 允许用户自定义 Provider 端点、模型或提示词。
- 自动接受章节、自动 final 或自动更新 Ledger。
- 将 unknown 当作 passed。
- 跳过差异、确认或写后校验。
- 让 AI 自动决定 Git 操作。
- 在 G5 前扩展到 Sprint 6、移动端或远程功能。

## 23. Sprint 5 Story Packet Review Gate

Review Gate 当前结论：**已通过（2026-08-26）**。

进入 NG-501 前必须满足：

- [x] S5-Q001 已确认。
- [x] S5-Q002 已确认。
- [x] S5-Q003 已确认。
- [x] S5-Q004 已确认。
- [x] S5-Q005 已确认。
- [x] S5-Q006 已确认。
- [x] S5-Q007 已确认。
- [x] 产品负责人明确批准 Sprint 5 Story Packet Review Gate。
- [x] 产品负责人只授权开始 NG-501，未批量授权后续 Story。

建议批准语句：

“S5-Q001～Q007 全部接受，批准 Sprint 5 Story Packet Review Gate，只授权开始 NG-501”

### Sprint 5 Review Gate 产品批准记录（2026-08-26）

产品负责人明确接受 S5-Q001～S5-Q007，并批准 Sprint 5 Story Packet Review Gate，只授权开始 NG-501。七项决策均按建议冻结，Review Gate 正式通过；本次授权不覆盖 NG-502～NG-507，也不提前批准 G5。

如果任一建议需要调整，请直接按问题编号说明；修订 Packet 后再进入 Review Gate。

## 24. G4 交接记录

- G4 已于 2026-08-26 获得产品批准。
- Sprint 4 已关闭。
- Sprint 5 Story Packet 已创建。
- Sprint 5 Review Gate 已通过；NG-501 已完成技术实现并通过全量质量门，等待产品验收。
- 下一步是产品负责人审查并验收 NG-501；只有明确批准后才可开始 NG-502，NG-502～NG-507 当前均未授权。
