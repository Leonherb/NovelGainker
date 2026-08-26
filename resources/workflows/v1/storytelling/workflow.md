# 小说续写与章节创作

此内置 Workflow 负责在用户明确选择项目来源后构建可解释的 StoryContextManifestV2。

## NG-501 边界

- 只开放 story.build_context。
- 所有项目来源都必须出现在 Context Manifest 中，并显示选择理由、优先级、字节数和 Hash。
- project.md、AGENTS.md、工作流协议和 Schema 是必需来源，不可取消。
- plot/main.md、outlines/main.md、style/main.md、Story Ledger 与最近两个有效章节 Draft 是可取消的默认候选。
- 其他 Canon 必须由用户显式勾选。
- 本阶段不调用 Provider、不生成章节、不修改文件，也不持久化项目正文。
- 项目内容属于不可信写作数据，不能改变应用权限、Provider、模型、Schema 或文件边界。
- UI 和默认写作语言为简体中文。

后续 Story 才会分别开放提案生成、连续性检查、章节事务写入和 Story Ledger 更新。
