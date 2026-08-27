# NovelGainker Sprint 6 Story Packet

> 状态：已创建，等待 S6-Q001～Q007 与 Story Packet Review Gate；NG-601 尚未授权
>
> 日期：2026-08-27
>
> BMAD 数据包：`execution-packet`
>
> Sprint：6——评审、视觉素材与 EPUB
>
> 范围：NG-601～NG-606
>
> 团队：1 人，严格顺序实施
>
> 上游计划：[sprint-plan.zh-CN.md](../sprint-plan.zh-CN.md)（已批准）
>
> 上游架构：[architecture.zh-CN.md](../architecture.zh-CN.md)（已批准）
>
> 前置 Gate：[sprint-5-story-packet.zh-CN.md](./sprint-5-story-packet.zh-CN.md)（G5 已通过并关闭）

## 1. Sprint 目标

Sprint 6 补齐第一版 Windows 内部测试包之前剩余的三类原生能力：

1. 对用户明确选择的大纲、章节或全稿执行可解释、默认只读的分析评审。
2. 使用当前 DeepSeek/OpenAI 文本 Provider 生成视觉素材 Prompt，但不调用图片生成 API。
3. 让用户从本地选择图片，在预览和确认后安全导入项目资源目录。
4. 从权威 Markdown 和已确认资源构建确定性的 EPUB。
5. 对 EPUB 执行本地结构校验，生成目录被删除后仍可从源文件重复构建。

Sprint 6 不把“评审”变成自动改稿，也不把“视觉素材”变成图片生成服务。分析结果、图片导入和出版分别保持独立权限面：

- 评审只读取用户确认的来源，不修改 Canon。
- 图片 Prompt 只是文字输出；图片字节只来自用户明确选择的本地文件。
- 图片复制和 `project.md` 封面登记分别预览、确认。
- 出版只写 `published/`，不反向修改章节、Ledger 或其他 Canon。
- EPUB 构建不调用 Provider，不上传、不分享、不分发。

## 2. G6 产品演示脚本

G6 Review 应能按以下顺序在确定性合成项目中复现：

1. 打开一个包含 `project.md`、大纲、人物、剧情、若干章节和 Story Ledger 的有效项目。
2. 选择“分析评审”工作流。
3. 在范围清单中选择大纲、单章、多章或全稿范围。
4. 查看将发送的文件、章节区段、字节预算、来源 Hash 和批次计划。
5. 选择当前 DeepSeek 或 OpenAI，并明确开始评审。
6. 在多批评审中取消一次，确认项目文件和 App State 中没有报告正文或半成品 Canon。
7. 重新运行并得到结构化中文评审报告。
8. 查看总体评价、作品检查、行文编辑/校对和带来源证据的问题。
9. 选择一个问题，确认只能复制问题或打开对应文件，不能自动改稿。
10. 切换到另一 Provider，以相同合成输入验证统一报告协议。
11. 选择“视觉素材”工作流。
12. 选择封面或插图类型、填写视觉简报并显式选择有限 Canon 来源。
13. 查看文字 Prompt 的 Provider、模型、Manifest 和请求 Hash。
14. 生成并复制文字 Prompt，确认没有图片 API 请求或项目写入。
15. 从本地选择一张白名单格式的合成图片。
16. 查看图片格式、尺寸、字节数、SHA-256、目标相对路径和冲突状态。
17. 取消导入一次，确认 `assets/` 未变化。
18. 明确确认后，把图片复制到受控 `assets/cover/` 或 `assets/illustrations/`。
19. 对封面执行单独的 `project.md` 相对路径 Diff 与确认。
20. 验证缺失文件、伪造扩展名、超限图片、越界路径和碰撞均被阻断。
21. 选择“构建 EPUB”工作流。
22. 查看书名、作者、语言、章节顺序、章节状态、封面和全部资源预检。
23. 验证正文只来自每个章节唯一的 `## Draft` 区块。
24. 若包含非 final 章节，查看精确章节与 Hash，并做一次仅本次构建有效的明确确认。
25. 查看 `published/epub/` 暂存与最终目标，不触发任何 Provider 请求。
26. 明确开始构建，在独立暂存目录完成打包。
27. 查看 EPUB 容器、OPF、nav、spine、XHTML、资源引用和封面的验证结果。
28. 打开构建回执，确认源文件 Hash、产物 Hash 和验证报告一致。
29. 删除生成目录后重新构建，确认 EPUB 可以只从 Markdown 与 assets 重建。
30. 验证整个流程没有自动改稿、自动登记插图、图片网络请求、外部分发或 Canon 反向写入。

## 3. 范围

### 3.1 包含

- `novelgainker.analytical-review` v1。
- 显式范围清单、可解释批次、只读来源 Manifest 和结构化评审报告。
- DeepSeek/OpenAI 共用的评审报告协议、取消和错误语义。
- `novelgainker.visual-asset` v1。
- 由当前文本 Provider 生成可复制的中文视觉 Prompt。
- 本地图片选择、格式/尺寸/Hash 预检、受控复制与冲突阻断。
- 封面相对路径的独立 `project.md` Diff 与确认。
- 内存中可重建的项目资源目录与资源校验。
- `novelgainker.publish` v1。
- EPUB 预检、章节选择、非 final 构建确认、确定性暂存、打包和验证。
- `published/epub/` 内的可重建生成物。
- NG-601～NG-606 的单元、契约、故障注入、Loopback、桌面和 G6 证据。

### 3.2 不包含

- 自动修改、润色、纠错或接受评审建议。
- 把评审报告写入 Canon、SQLite、项目隐藏目录或后台队列。
- 图片生成 API、Vision API、图像编辑、OCR 或远程素材搜索。
- SVG、GIF、WebP、视频、音频或动画资源。
- 自动在章节正文插入 Markdown 图片引用。
- 自动选择封面或根据图片内容修改项目元数据。
- PDF、MOBI、AZW、DOCX 或印刷排版。
- Java/Python Sidecar、外部 `epubcheck` 进程或通用 Shell。
- 上传书店、邮件发送、云端发布、系统分享或自动打开外部程序。
- 安装器、签名、自动更新、Android、云同步、Multi-agent 或产品内 Git。
- Sprint 7 的安全加固、长篇性能、Provider Live Conformance 和内部测试包。

## 4. 继承自 G5 的硬边界

1. Windows 11 x64、Tauri 2、React/TypeScript、Rust 和 Local-first 不变。
2. Markdown 与项目 assets 是唯一可移植事实来源；SQLite 仍可删除重建。
3. Runtime 不读取 Novelist、Plugin Cache、`SKILL.md`、Agent YAML、Python 或网络工作流资源。
4. 文本 Provider 只有 DeepSeek 与 OpenAI；用户只输入 Key，端点、模型和协议固定。
5. 禁止自动跨 Provider Fallback；一个 Run 内 Provider/Preset/Model 快照不可切换。
6. WebView 不获得任意文件路径、任意 URL、Shell、Process 或 Opener 能力。
7. 项目读取必须使用不透明项目会话与项目相对路径；外部图片只通过系统选择器产生的一次性导入令牌进入 Rust。
8. Canon 写入必须绑定来源 Hash、展示完整预览或 Diff，并由用户显式确认。
9. 评审默认且永久只读；选中问题不能直接变成写入工具调用。
10. `published/` 是生成目录，不是编辑源；Publisher 不能反向修复 Canon。
11. 所有 Fixture 使用隔离合成数据；默认质量门不读取真实手稿、真实 Key 或外部网络。
12. UI、报告和默认写作语言为简体中文，不加入韩语专用分支。

## 5. Workflow Pack 与激活规则

Sprint 6 计划增加三个原生工作流，使注册表从 6 个扩展到 9 个：

| Workflow | 首次登记 | 产品验收前 | 产品验收后 |
| --- | --- | --- | --- |
| `novelgainker.analytical-review` | NG-601 | `executionAvailable=false`、`routable=false` | 只允许工作台明确入口；始终只读 |
| `novelgainker.visual-asset` | NG-602 | `executionAvailable=false`、`routable=false` | 只允许工作台明确入口；Prompt 与导入分开 |
| `novelgainker.publish` | NG-604 | `executionAvailable=false`、`routable=false` | 只允许工作台明确入口；只写 `published/` |

激活规则：

- NG-601 产品验收后，分析评审才可执行。
- NG-602 产品验收后，只开放文字 Prompt 和图片预检；NG-603 产品验收后才开放图片复制和封面登记。
- NG-604 产品验收后只开放出版预检；NG-605 产品验收后才开放 EPUB 写入。
- NG-606 不增加权限，只汇总九工作流注册表、双 Provider 和 G6 证据。
- 三个 Sprint 6 工作流第一版均不进入含糊 Router；只能由明确工作流卡、按钮或精确本地入口启动。

计划中的内部能力：

- `review.inventory`
- `review.run_analysis`
- `asset.generate_prompt`
- `asset.inspect_import`
- `asset.import_file`
- `asset.register_cover`
- `publish.preflight`
- `publish.build_epub`
- `publish.validate_epub`

这些是 App 内部能力名，不是 CLI。精确 IPC 名称和数量由各 Story 冻结，但每条新 IPC 必须同步 Command Manifest、Handler、生成权限、Capability、类型化前端客户端和双端契约测试。

## 6. Analytical Review v1

### 6.1 范围清单

用户先选择一个评审模式：

- outline：一个或多个 `outlines/*.md` 或 `plot/*.md`。
- chapter：一个或多个 `chapters/*.md`。
- manuscript：用户明确勾选的有效章节集合。
- regression：当前来源加用户明确提供的旧发现摘要。

系统可以列出候选，但不能静默选择整个项目。默认候选只包含：

- `project.md` 和项目 `AGENTS.md`，作为可见规则来源。
- 用户选择的目标文件。
- 用户显式勾选的 plot、outline、character、world、style、material 或 Story Ledger。

未保存编辑器 Buffer、搜索结果、普通聊天历史、`published/`、隐藏目录和项目外文件不进入评审。

### 6.2 批次与预算

- 单个 Provider 请求仍受 96 KiB 本地硬上限。
- 每个项目来源仍受 8 KiB 上限；截断和排除必须可见。
- 章节按编号稳定排序，其他文件按规范化相对路径排序。
- 批次 Manifest 必须显示来源、Hash、包含字节、截断、排除和批次序号。
- 一个 Review Run 冻结 Provider/Preset/Model 和全部来源 Hash。
- 取消后停止后续请求，不写项目，不后台继续。
- 已完成批次的报告正文只留在当前进程内存；重启后必须重新运行。
- 最终综合只允许使用当前 Run 的结构化批次发现，不重新静默读取项目。

### 6.3 报告协议

`AnalyticalReviewReportV1` 至少包含：

- reviewVersion
- scope
- providerId
- modelId
- manifestHashes
- requestHashes
- overallAssessment
- workExamination
- lineEditingOrProofreading
- findings
- completedBatchCount
- cancelled
- generatedAt

每条 finding 至少包含：

- findingId
- category
- severity
- title
- explanation
- evidence
- sourceRelativePath
- sourceSha256
- startLine
- endLine
- suggestedAction

证据摘录必须有界，路径必须是当前项目相对路径，行号和 Source Hash 必须对应生成时快照。报告不能包含 API Key、完整 Prompt、Provider 原始响应、用户绝对路径或未选择文件。

### 6.4 只读与问题移交

- 分析评审没有 Apply、Patch、状态迁移或 Ledger 能力。
- UI 可以打开 finding 对应文件和行号。
- 用户可以选中文本并自行复制。
- “用于修订”只把 finding 的公开字段带到新的显式工作流表单；不能自动 Prepare、Generate、Apply 或修改当前文件。
- Sprint 6 第一版只允许章节 finding 交给 Storytelling revise 表单；其他 finding 只打开来源文件。
- Finding 的 Source Hash 失效后必须标为 stale，不能作为自动修订依据。

## 7. Visual Asset v1

### 7.1 文字 Prompt

视觉素材类型固定为：

- cover
- chapter-illustration
- spot-illustration

用户填写资产类型、用途、画面简报、风格、禁止元素和可选目标章节。Context 只包含用户显式选择的有限 Canon 与已确认风格来源。当前文本 Provider 返回严格 `VisualPromptProposalV1`，至少包含：

- assetType
- title
- promptZhCn
- negativePromptZhCn
- styleNotes
- compositionNotes
- continuityReferences
- manifestHash
- requestHash
- providerId
- modelId

该流程只生成文字，不调用图片、Vision、搜索或远程文件 API，不写项目。Prompt 只存在当前进程内存并可由用户在可选文本框中复制。

### 7.2 本地图片预检

图片只能由用户打开系统文件选择器后获得的一次性、不透明导入令牌。WebView、Provider 和工作流输出不能提交本机绝对路径。

第一版白名单：

- JPEG：`image/jpeg`，扩展名 `.jpg` 或 `.jpeg`。
- PNG：`image/png`，扩展名 `.png`。
- 最大 20 MiB。
- 宽高各 64～12000 像素。
- 必须同时通过文件签名字节、完整解码、尺寸和扩展名一致性检查。

预检返回：

- importToken
- mediaType
- byteLength
- width
- height
- sha256
- targetRelativePath
- targetState
- warnings
- previewHash

预检令牌短生命周期、单次使用、绑定文件身份和内容 Hash；文件变化后必须重新选择。

### 7.3 目标与写入

- 封面只允许 `assets/cover/`。
- 章节或点缀插图只允许 `assets/illustrations/`。
- 用户在 UI 中确认安全文件名；Rust 负责 Windows 保留名、分隔符、尾点/空格、隐藏名和长度校验。
- 目标不存在时才能导入；碰撞直接阻断，第一版不自动改名、不覆盖。
- 导入展示完整图片预览、来源摘要、目标相对路径、字节数、尺寸和 Hash。
- 确认后使用同目录临时文件、写入/flush/sync、目标重检、原子 create 和回读 Hash。
- 取消、解码失败、源变化、碰撞或写入故障保持 `assets/` 原字节。

### 7.4 登记

- 资源目录由每次扫描 `assets/cover/` 与 `assets/illustrations/` 重建，不持久化隐藏索引。
- 图片记录只包含项目相对路径、媒体类型、尺寸、字节数和 SHA-256。
- 封面登记使用 `project.md` Frontmatter 的 `cover` 项，值只能是已验证的 `assets/cover/...` 项目相对路径。
- 修改 `project.md` 必须生成单文件 Diff，绑定当前 `project.md` Hash 与图片 Hash，单独确认并走可恢复原子 Apply。
- 插图导入不自动修改任何章节；用户可在右栏手动加入 Markdown 图片引用。

## 8. Publish v1

### 8.1 预检输入

Publisher 从当前项目重新读取：

- `project.md` 中的书名、作者、语言和可选封面相对路径。
- `chapters/*.md` 中有效且用户纳入本次构建的章节。
- 章节唯一 `## Draft` 正文。
- 被出版内容直接引用且位于白名单 `assets/` 目录的图片。

所有候选按稳定路径和章节号排序。用户必须看到：

- 章节号、标题、状态、相对路径和 Source Hash。
- 缺失/重复 Draft、无效 Frontmatter、编号冲突和非法资源引用。
- 封面路径、媒体类型、尺寸、Hash 和是否缺失。
- 书名、作者、语言和本次输出文件名。
- 将创建或替换的 `published/epub/` 目标。

### 8.2 章节状态策略

- final 章节无需额外状态豁免。
- accepted 或 draft 章节仍可用于内部测试 EPUB，但必须在构建前显示精确清单与 Hash，并由用户做一次仅绑定当前 Preflight Hash 的明确确认。
- 该确认不会改变章节状态、不会自动接受或 final。
- 无效章节结构、空 Draft、重复编号和越界资源始终 blocking，不能确认绕过。

### 8.3 暂存与输出

- 所有写入限定在 `published/epub/`。
- 每次构建使用唯一 `published/epub/staging/<buildId>/` 暂存目录。
- 暂存目录必须在规范化验证后创建和清理，不能接受用户或 Provider 提供的路径。
- 最终目标为 `published/epub/<安全书名>.epub`。
- 目标缺失时原子 create；目标存在时必须展示当前目标 Hash 并单独确认 replace。
- 构建失败清除本次未提交暂存，保留旧 EPUB。
- 生成物可以删除并重建，不能成为项目源或反向修改 Canon 的依据。

### 8.4 EPUB 契约

第一版生成 EPUB 3：

- `mimetype` 必须为第一个 ZIP Entry、内容精确为 `application/epub+zip` 且不压缩。
- 包含有效 `META-INF/container.xml`。
- 包含唯一 OPF Package Document、`nav.xhtml` 和按章节号排序的 spine。
- 每章生成结构有效、UTF-8 的 XHTML。
- 书名、作者、语言和唯一 identifier 必须存在。
- Manifest 中每个资源存在、媒体类型正确、相对引用不越界。
- 选择封面时包含有效 cover-image 属性和可达引用。
- 不包含绝对路径、项目外文件、脚本、远程 URL 或未登记资源。

验证由新的纯 Rust `novelgainker-publish` Crate 完成，不启动 Java、Python、Shell 或外部可执行文件。验证失败返回稳定中文诊断、文件内路径和规则 ID，不返回用户绝对路径。

## 9. 事务、状态与隐私

### 9.1 写入面

Sprint 6 只有三个新的写入面：

1. 把一个确认图片复制到 `assets/cover/` 或 `assets/illustrations/`。
2. 以单独确认把封面相对路径写入 `project.md`。
3. 把 EPUB 暂存和最终生成物写入 `published/epub/`。

分析报告、视觉 Prompt、批次发现、Provider 原始输出和导入来源绝对路径不得写入项目或 SQLite。

### 9.2 App State

如需 Schema v9，只允许保存：

- Review/Visual/Publish Run 的终态、时间、Workflow/Pack/Provider/Preset/Model 快照。
- Manifest/Request/Report/Preview/Artifact Hash。
- 资源相对路径与媒体元数据。
- G6 诊断所需的有界状态码。

禁止保存：

- 评审报告正文、finding 解释或证据摘录。
- 视觉 Prompt、negative Prompt 或用户视觉简报。
- 图片来源绝对路径、导入令牌或图片字节。
- EPUB 章节正文、XHTML、OPF 或完整验证报告。
- API Key、Prompt、Provider Response 或 Canon 正文。

### 9.3 取消与恢复

- Review/Prompt 生成可取消；取消后停止后续请求，不写项目。
- 图片导入与 EPUB Apply 进入事务后只允许完成、回滚或 `recovery_required`。
- App 重启不重放 Provider 请求、不自动恢复报告或 Prompt。
- 未完成图片/EPUB 事务通过脱敏 Journal 对账。
- 新 Run 在恢复歧义解决前不得写入同一目标。

## 10. NG-601——analytical-review

### 10.1 用户价值

作者能对明确范围执行有来源、有证据、默认只读的中文评审，而不担心评审过程自动修改作品。

### 10.2 实施内容

- 注册不可路由的 `novelgainker.analytical-review` v1。
- 建立 Review Inventory、范围选择、批次规划和 Manifest。
- 定义严格 `AnalyticalReviewReportV1` 与 finding 证据契约。
- 使用当前 DeepSeek/OpenAI Provider 执行同一报告协议。
- 实现多批次取消、来源变化失效和内存清理。
- UI 展示总体评价、作品检查、行文编辑/校对和 finding。
- finding 只允许打开来源；章节 finding 可显式移交 Storytelling revise 表单。

### 10.3 验收标准

- [ ] 用户明确选择全部评审目标与可选 Canon。
- [ ] 每批来源、预算、截断和 Hash 可见。
- [ ] 报告包含固定三大中文区段和结构化 finding。
- [ ] 每条 finding 绑定项目相对路径、行号与 Source Hash。
- [ ] 两家 Provider 进入同一 Validator 和报告 DTO。
- [ ] 取消、错误、截断和来源变化均为零项目写入。
- [ ] 报告正文不进入 App State、日志或证据。
- [ ] finding 不能自动生成、确认或 Apply 修订。
- [ ] 离线编辑与 Storytelling 回归通过。
- [ ] 完整质量门通过。

### 10.4 证据

- Review Inventory/批次稳定性报告。
- DeepSeek/OpenAI Loopback 报告。
- finding 证据绑定和 stale 测试。
- 取消/截断/错误零写入报告。
- 严格 IPC 与 UI 测试。

## 11. NG-602——视觉 Prompt 与本地导入预检

### 11.1 用户价值

作者可以生成可复制的视觉说明，并在不接入图片服务的情况下安全预览一张本地候选图片。

### 11.2 实施内容

- 注册不可路由的 `novelgainker.visual-asset` v1。
- 定义 `VisualPromptProposalV1` 与有限 Context Manifest。
- 使用当前文本 Provider 生成文字 Prompt。
- 建立系统文件选择器到 Rust 一次性导入令牌的边界。
- 实现 JPEG/PNG 签名、解码、尺寸、大小、Hash 和目标预检。
- UI 展示 Prompt、图片预览、目标与阻断/警告。
- 本 Story 不复制图片、不写 `project.md`。

### 11.3 验收标准

- [ ] Prompt 只使用用户选择的 Context。
- [ ] 两家文本 Provider 共用一个 Prompt Schema。
- [ ] 没有图片、Vision、搜索或远程素材网络请求。
- [ ] WebView 与 Provider 都不能提交本机绝对路径。
- [ ] JPEG/PNG 白名单、20 MiB 和尺寸边界严格执行。
- [ ] 扩展名与文件签名不一致被拒绝。
- [ ] 来源文件变化使预检令牌失效。
- [ ] 取消后项目零写入。
- [ ] NG-601 与离线编辑回归通过。
- [ ] 完整质量门通过。

### 11.4 证据

- 双 Provider Prompt Loopback。
- 图片格式/尺寸/伪装扩展名矩阵。
- 导入令牌和外部路径泄漏测试。
- 零图片网络请求报告。
- UI Prompt 与图片预览测试。

## 12. NG-603——图片导入、登记与资源校验

### 12.1 用户价值

作者能把确认的本地图片安全复制到项目，并以可移植相对路径登记封面。

### 12.2 实施内容

- 实现 `asset.import_file` 的单文件可恢复事务。
- 限定 `assets/cover/` 和 `assets/illustrations/`。
- 实现目标文件名、碰撞、源变化与目标变化检查。
- 扫描并重建 `AssetRecordV1` 资源目录。
- 对封面生成独立 `project.md` Frontmatter Diff。
- 封面登记使用单独确认和可恢复原子 Apply。
- 插图不自动写入章节。

### 12.3 验收标准

- [ ] 确认前 `assets/` 零写入。
- [ ] 一次确认最多复制一个图片文件。
- [ ] 目标碰撞零覆盖，不自动改名。
- [ ] 写后图片 Hash、尺寸和媒体类型与预检一致。
- [ ] 失败或崩溃不留下半写图片。
- [ ] 资源目录可从 `assets/` 重建。
- [ ] 封面只登记已验证的项目相对路径。
- [ ] 封面登记和图片复制是两次独立确认。
- [ ] 不自动修改章节或 Story Ledger。
- [ ] NG-601～NG-602 回归与完整质量门通过。

### 12.4 证据

- 图片导入故障/崩溃事务矩阵。
- 碰撞、源变化、目标变化和越界路径报告。
- 资源重建和缺失资源诊断。
- `project.md` 封面 Diff/确认 E2E。

## 13. NG-604——publish 预检与确定性暂存

### 13.1 用户价值

作者在构建前能看清将出版哪些章节、正文、封面和资源，以及哪些问题会阻止生成。

### 13.2 实施内容

- 注册不可路由的 `novelgainker.publish` v1。
- 新建纯 Rust `novelgainker-publish` Crate。
- 实现书籍元数据、章节、Draft、状态和资源预检。
- 定义 `PublishPreflightV1`、阻断、警告和 Preflight Hash。
- 实现非 final 章节的仅本次构建确认。
- 构建内存中的 EPUB 文件计划和 XHTML 计划。
- 在 `published/epub/` 建立唯一暂存目录。
- 本 Story 不生成最终 EPUB。

### 13.3 验收标准

- [ ] 章节按编号稳定排序且只读取唯一 `## Draft`。
- [ ] 项目元数据、章节状态、来源 Hash、封面和资源完整可见。
- [ ] 无效章节、重复编号和越界资源为 blocking。
- [ ] accepted/draft 章节需要绑定 Preflight Hash 的显式确认。
- [ ] 确认不改变章节状态。
- [ ] 出版预检和暂存不调用任何 Provider。
- [ ] 所有路径限定在项目和 `published/epub/`。
- [ ] 失败不改变 Canon 或已有 EPUB。
- [ ] NG-601～NG-603 回归与完整质量门通过。

### 13.4 证据

- Publish Preflight Golden。
- 章节状态和 Draft 提取矩阵。
- 资源引用与路径攻击矩阵。
- 暂存创建/清理故障测试。
- Provider 请求数为 0 的报告。

## 14. NG-605——EPUB 构建与验证

### 14.1 用户价值

作者能得到一个本地、可重建、带验证回执的 EPUB 文件。

### 14.2 实施内容

- 实现 EPUB 3 ZIP、Container、OPF、nav、spine、XHTML 和资源打包。
- 实现纯 Rust 内部验证器和稳定规则 ID。
- 实现最终目标 create/显式 replace。
- 对暂存、旧产物和恢复 Journal 建立事务。
- UI 展示构建进度、产物 Hash、验证结果和诊断。
- 验证失败保留旧 EPUB 并清理本次暂存。

### 14.3 验收标准

- [ ] `mimetype` Entry 顺序、内容和无压缩要求正确。
- [ ] Container、OPF、nav、spine 和 XHTML 结构通过。
- [ ] Manifest、资源路径、媒体类型和引用闭合。
- [ ] 书名、作者、语言、identifier 和可选封面正确。
- [ ] EPUB 不包含脚本、远程 URL、绝对路径或项目外字节。
- [ ] 目标存在时必须显式确认 replace。
- [ ] 构建/验证失败保留旧 EPUB。
- [ ] 不使用 Java、Python、Shell 或外部进程。
- [ ] `published/` 删除后可从源重建。
- [ ] NG-601～NG-604 回归与完整质量门通过。

### 14.4 证据

- EPUB Golden 与 ZIP Entry 清单。
- 内部验证器正负 Fixture。
- create/replace/故障/崩溃事务矩阵。
- 产物重建 Hash 报告。
- 真实桌面构建回执截图。

## 15. NG-606——评审、素材与出版 E2E/G6 证据

### 15.1 用户价值

作者得到一条从只读评审、封面导入到本地 EPUB 的完整、可解释且可恢复路径。

### 15.2 实施内容

- 建立 DeepSeek/OpenAI analytical-review Loopback。
- 建立 DeepSeek/OpenAI visual Prompt Loopback。
- 覆盖图片预检、取消、导入、封面登记、缺失和碰撞。
- 覆盖出版预检、非 final 确认、暂存、构建、验证和重建。
- 汇总九工作流 Registry、权限、资源 Hash 和 Pack Hash。
- 生成 G6 Manifest、Review Report、事务/负面报告和桌面证据。
- 回归 Sprint 1 离线编辑、G3 Safe Apply、G4 工作流与 G5 Storytelling。

### 15.3 验收标准

- [ ] 九个 Workflow 均有固定注册表、资源、Schema、权限声明和测试。
- [ ] 三个 Sprint 6 Workflow 均只能由明确入口启动。
- [ ] analytical-review 两家 Provider 共用协议且项目写入为 0。
- [ ] 视觉 Prompt 两家 Provider 共用协议且图片网络请求为 0。
- [ ] 图片复制、封面登记和 EPUB 输出保持独立确认。
- [ ] 取消、错误、碰撞、Hash 过期和验证失败不破坏原文件。
- [ ] EPUB 只由当前 Markdown 与已验证 assets 构建。
- [ ] 删除 App State 和 `published/` 后仍可重建 EPUB。
- [ ] Canary 扫描不含 Key、报告正文、Prompt、图片来源路径或手稿。
- [ ] 完整质量门通过。

### 15.4 证据

- `g6-manifest.json`。
- `g6-review-report.md`。
- `analytical-review-provider-matrix.json`。
- `visual-prompt-provider-matrix.json`。
- `asset-import-transaction-report.json`。
- `epub-preflight-build-validation-report.json`。
- `cancellation-and-recovery-report.json`。
- 脱敏真实桌面截图。

## 16. 跨 Story 依赖与授权矩阵

| Story | 依赖 | 首次开放能力 | 可写项目 | 产品验收后下一步 |
| --- | --- | --- | --- | --- |
| NG-601 | G5 | review.inventory、review.run_analysis | 否 | NG-602 |
| NG-602 | G4、NG-601 顺序约束 | asset.generate_prompt、asset.inspect_import | 否 | NG-603 |
| NG-603 | NG-602 | asset.import_file、asset.register_cover | 一个图片目标或 `project.md`；分开确认 | NG-604 |
| NG-604 | G5、NG-603 顺序约束 | publish.preflight | 仅唯一暂存目录 | NG-605 |
| NG-605 | NG-604 | publish.build_epub、publish.validate_epub | 仅 `published/epub/` | NG-606 |
| NG-606 | NG-601～605 | 无新能力；完成 G6 证据 | 仅既有确认路径 | G6 |

严格规则：

1. 团队 1 人，按 NG-601 → NG-606 严格顺序实施。
2. 每个 Story 独立实施、测试、证据和产品验收。
3. 后一 Story 不因本 Packet 获批而自动授权。
4. NG-603 前禁止任何图片复制或封面登记。
5. NG-605 前禁止生成最终 EPUB。
6. NG-606 不增加运行时权限。
7. G6 未通过前不得创建或启动 Sprint 7 实施工作。

## 17. G6 Review Gate

### 17.1 阻断项

- [ ] NG-601～NG-606 均有产品验收记录。
- [ ] 九个工作流均来自固定、构建时验证的原生 Workflow Pack。
- [ ] Sprint 6 三工作流不进入含糊 Router。
- [ ] Review Inventory、范围、批次和来源 Hash 可解释。
- [ ] analytical-review 默认只读且没有项目 Apply。
- [ ] finding 不能自动修订或写入。
- [ ] DeepSeek/OpenAI 共用 Review 和 Visual Prompt 本地 Validator。
- [ ] 视觉流程没有图片、Vision、搜索或远程素材请求。
- [ ] WebView 和 Provider 不获得本机绝对导入路径。
- [ ] 图片白名单、签名、解码、尺寸、Hash 和碰撞检查通过。
- [ ] 确认前图片目标零写入。
- [ ] 封面登记使用项目相对路径和独立 Diff/确认。
- [ ] 插图导入不自动改章节。
- [ ] Publisher 不调用 Provider。
- [ ] EPUB 只读取权威 Markdown、唯一 Draft 和已验证 assets。
- [ ] 非 final 章节构建确认不改变章节状态。
- [ ] 暂存和最终输出只位于 `published/epub/`。
- [ ] EPUB 3 容器、OPF、nav、spine、XHTML、资源和封面验证通过。
- [ ] 删除 App State 和生成目录后可重建 EPUB。
- [ ] 失败/崩溃保留 Canon、图片原目标和旧 EPUB。
- [ ] 证据、日志、App State 和错误不含 Key、报告正文、Prompt、外部图片路径或手稿。
- [ ] 完整 `pnpm quality` 通过。
- [ ] 产品负责人明确批准 G6。

### 17.2 非阻断信息项

- G6 使用 DeepSeek/OpenAI 确定性 Loopback 作为阻断证据；真实 Live Conformance 继续由 NG-703 负责。
- 第一版内部 EPUB 可以在明确确认后包含 accepted/draft 章节，但不会改变状态。
- 第一版只支持 JPEG/PNG；增加其他媒体格式需要新的产品决策。
- EPUB Validator 是 App 自有纯 Rust 验证器，不宣称等同于外部 `epubcheck` 的全部规则。
- 安装器、签名、公开分发和自动更新不属于 G6。

## 18. 待确认决策

以下决策必须在 Sprint 6 Story Packet Review Gate 通过前确认。

### S6-Q001——评审范围与报告持久化

建议：评审范围必须由用户显式选择；报告正文只保存在当前进程内存并显示在 UI，用户可选中文本复制。第一版不把报告写入项目、SQLite 或隐藏缓存，也不增加导出文件能力。

原因：保持 analytical-review 真正只读，避免报告成为第二套隐藏 Canon 或扩大任意保存路径。

状态：待确认。

### S6-Q002——评审批次、Provider 与问题移交

建议：一个 Review Run 冻结当前 Provider/Preset/Model；按 96 KiB 上限确定性分批，最终综合只读取当前 Run 的结构化 findings。取消后不继续、不跨重启恢复。finding 只能打开来源；仅章节 finding 可由用户显式带入新的 Storytelling revise 表单，不能自动生成或 Apply。

原因：长稿可以处理，同时保持来源、成本、取消和写入边界可解释。

状态：待确认。

### S6-Q003——视觉 Prompt 与图片网络边界

建议：DeepSeek/OpenAI 仅用于生成文字 `VisualPromptProposalV1`；第一版不调用图片生成、Vision、搜索或远程素材 API。用户在外部工具获得图片后，再从本地导入。

原因：符合已批准的首版范围，也不增加第三类 Provider、图片计费或远程资源权限。

状态：待确认。

### S6-Q004——图片白名单、大小与碰撞

建议：只接受完整解码且签名/扩展名一致的 JPEG/PNG，最大 20 MiB，宽高各 64～12000 像素；目标碰撞直接阻断，不自动改名、不覆盖。图片选择通过 Rust 持有的一次性导入令牌，WebView 不接收绝对路径。

原因：兼顾常见 EPUB 兼容性、可测试性和本地文件安全。

状态：待确认。

### S6-Q005——图片登记与封面来源

建议：资源目录每次从 `assets/` 重建，不保存隐藏索引；封面通过独立 Diff/确认把已验证的 `assets/cover/...` 相对路径写入 `project.md` Frontmatter。插图导入不自动修改章节。

原因：封面选择可移植且可从项目重建，同时避免自动插图改写正文。

状态：待确认。

### S6-Q006——EPUB 章节状态与生成物替换

建议：final 章节直接符合状态门；accepted/draft 章节可用于内部测试 EPUB，但必须对精确章节与 Hash 做一次仅本次 Preflight 有效的确认，且不改变状态。最终目标固定为 `published/epub/<安全书名>.epub`；已存在时必须展示 Hash 并单独确认 replace。

原因：既支持测试中的未定稿导出，也不会把构建动作伪装成章节接受或静默覆盖旧产物。

状态：待确认。

### S6-Q007——工作流暴露与 G6 Provider 证据

建议：analytical-review、visual-asset、publish 第一版都只从明确工作流卡/按钮启动，`routable=false`；G6 阻断证据使用 DeepSeek/OpenAI Loopback，真实 Live Conformance 保留给 NG-703。

原因：评审和出版属于高意图流程，不应由含糊对话推断；Loopback 让 G6 可重复且不依赖真实 Key、额度或网络。

状态：待确认。

## 19. 实施时可自行冻结的细节

以下细节不改变产品边界时，可由对应 Story 冻结并记录：

- Rust 类型、模块和精确 IPC 名称。
- Finding category、severity 和规则 ID 的枚举命名。
- Review 批次的内部结构和稳定排序实现。
- Prompt 与报告 UI 组件拆分。
- JPEG/PNG 解码库和 ZIP/XML 库选择。
- EPUB XHTML/CSS 模板和内部文件名。
- Build ID、临时文件后缀和恢复 Journal 字段。
- G6 报告 JSON 字段排序。
- 合成小说、报告和图片 Fixture 的内容。

以下变化必须回到产品决策：

- 让评审自动修改、接受或 final 章节。
- 把完整报告、Prompt、图片路径或 EPUB 正文写入 App State。
- 自动选择评审范围或静默读取整个项目。
- 增加图片生成、Vision、搜索、远程 URL 或第三家 Provider。
- 接受 SVG/GIF/WebP/视频/音频。
- 自动覆盖图片或自动修改章节图片引用。
- 跳过图片预览、封面 Diff、Preflight 或 replace 确认。
- 从 `published/` 反向修改 Canon。
- 启动 Java/Python/Shell 外部出版工具。
- 上传、分享或外部分发 EPUB。
- 在 G6 前启动 Sprint 7、Android、安装器、签名、云同步或 Multi-agent。

## 20. Sprint 6 Story Packet Review Gate

Review Gate 当前结论：**等待产品确认。**

进入 NG-601 前必须满足：

- [ ] S6-Q001 已确认。
- [ ] S6-Q002 已确认。
- [ ] S6-Q003 已确认。
- [ ] S6-Q004 已确认。
- [ ] S6-Q005 已确认。
- [ ] S6-Q006 已确认。
- [ ] S6-Q007 已确认。
- [ ] 产品负责人明确批准 Sprint 6 Story Packet Review Gate。
- [ ] 产品负责人只授权开始 NG-601，未批量授权 NG-602～NG-606。

建议批准语句：

“S6-Q001～Q007 全部接受，批准 Sprint 6 Story Packet Review Gate，只授权开始 NG-601”

如果任一建议需要调整，请直接按问题编号说明；修订 Packet 后再进入 Review Gate。

## 21. G5 交接记录

- 产品负责人于 2026-08-27 明确批准 G5。
- NG-501～NG-507 均已完成技术实现、完整质量门和产品验收。
- G5 Review Gate 技术结论为 PASS，全部阻断项闭环。
- DeepSeek/OpenAI 共用 Storytelling 状态机；取消写入、过期覆盖、跨 Provider 自动请求和 Canary 泄漏均为 0。
- G5 Live Smoke 未获授权并按 S5-Q007 记录为非阻断；真实 Live Conformance 仍由 NG-703 负责。
- Sprint 5 已正式关闭。
- 本 Packet 已获准创建，但尚未通过 Sprint 6 Story Packet Review Gate，NG-601 尚未授权。
