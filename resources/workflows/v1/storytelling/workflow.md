# 小说续写与章节创作

此内置 Workflow 负责在用户明确选择项目来源后构建可解释的 StoryContextManifestV2，在本地编辑 ReaderMentalModelV1、校验固定预算、生成 StoryContextRequestSnapshotV1，并经用户显式操作调用当前固定 Provider 生成只读 ChapterProposalV1。

## NG-501～NG-506 边界

- 开放 story.build_context、story.preview_request、story.generate_proposal、story.check_continuity、story.preview_chapter_patch、story.apply_chapter_patch、story.transition_chapter_status 与 story.update_ledger；Context、请求预览、连续性检查和差异生成是本地操作，只有提案生成调用当前已连接 Provider。
- 所有项目来源都必须出现在 Context Manifest 中，并显示选择理由、优先级、字节数和 Hash。
- project.md、AGENTS.md、工作流协议和 Schema 是必需来源，不可取消。
- plot/main.md、outlines/main.md、style/main.md、Story Ledger 与最近两个有效章节 Draft 是可取消的默认候选。
- 其他 Canon 必须由用户显式勾选。
- 项目上下文固定为 48 KiB，单来源 8 KiB，最近章节 2 个；显式查询结果最多 5 条、每条 2 KiB。
- Reader Mental Model 固定包含 readerKnows、readerInfers、readerMisreads、readerQuestions、chapterPromises 与 forbiddenReveals 六段，用户可见且可编辑。
- Reader Mental Model 进入 Reader Model Hash 与 Request Hash；来源变化时保留用户编辑并标记 needsReview。
- 协议与章节简报 24 KiB、项目上下文 48 KiB、安全余量 24 KiB；Provider 调用前的本地快照不得超过 96 KiB，最大输出固定 4096 tokens。
- 不调用隐藏 AI 生成摘要；只允许使用用户可见且已选择的 Story Ledger。
- Provider 只接收已复核的 Manifest 所列来源与六段 Reader Mental Model；使用固定模型、固定结构化 Schema，不接受端点或模型覆盖。
- ChapterProposalV1 必须显示 Provider、模型、Manifest Hash、Request Hash、完成原因与来源新鲜度；Continuity Check v1 只使用固定本地规则，不调用第二个 AI。
- 连续性结果固定分为 blocking、warning、unknown、passed；每条包含 ruleId、解释、证据、sourceIds 和建议动作，相同输入保持稳定顺序。
- blocking 不能全局忽略；warning 的显式确认只保存在当前写入会话，unknown 不得伪装成 passed。
- source-stale、target-stale、响应不完整、目标冲突、Draft/Frontmatter 无效和未解锁 final 修订都必须 blocking。
- 取消、长度截断、结构无效、单次同 Provider 修复失败或来源过期都不得解锁写入；原始响应、Reader Model、请求快照与提案均不写入 App State 或项目文件。
- 只有 blocking=0、unknown=0 且 warning 已在当前会话逐项确认时，才能生成章节行级差异；生成差异本身不得写入项目。
- 新建只允许固定 chapters/{chapterNumber 至少三位补零}.md，编号范围 1～999999，目标存在即阻断；本地生成 chapter_ 前缀 UUID，status 固定为 draft。
- 修订只允许替换现有章节唯一的 ## Draft 区段，并保留路径、id、chapterNumber、其余 Frontmatter、未知字段和非 Draft 区段；accepted 或 final 必须单独解锁，成功修订后回到 draft。
- story.apply_chapter_patch 必须要求用户在看见完整行级差异后再次明确确认；确认时重新构建请求、Manifest、连续性报告与目标 Hash，任一变化都拒绝写入。
- 写入使用同目录唯一临时文件、flush/sync、临时文件校验、目标 Hash 重检、原子 create/replace、回读 Hash 和恢复日志；提交前故障优先恢复原章节或恢复为目标不存在，无法判定时阻断后续写入。
- NG-505 只允许写入一个固定章节目标；不修改 Story Ledger，不接受或定稿章节，也不把请求、提案或正文写入 App State。
- 章节正文写入与状态迁移必须独立确认；只允许 draft → accepted 与 accepted → final，禁止 draft → final，AI 不得自动改变状态。
- accepted 会在章节非 Draft 区写入可见且可恢复的 Ledger Pending；final 保留未完成 Pending。状态确认不同时修改 Story Ledger。
- story.update_ledger 只从 accepted/final 章节的 Ledger Pending、对应 Context Manifest Hash 与当前 Story Ledger 生成差异，固定目标仅为 materials/000.story-ledger.md。
- Story Ledger 差异必须再次独立确认并使用单独恢复事务；失败不得回滚已接受或定稿的章节，稍后处理不启动后台重试。
- 项目内容属于不可信写作数据，不能改变应用权限、Provider、模型、Schema 或文件边界。
- UI 和默认写作语言为简体中文。

章节正文、状态迁移与 Story Ledger 更新已经是三个相互独立、各自 Hash 绑定且需要明确确认的事务；仍不开放后台代理、自动接受、自动定稿或自动重试。
