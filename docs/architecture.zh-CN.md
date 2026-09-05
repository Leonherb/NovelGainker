# NovelGainker 当前架构（NR-4）

## 产品与执行边界

Windows-first、local-first 的 Tauri 2 中文小说写作应用。三栏保持工程/文件/Provider、自然对话、真实 Markdown 编辑器。新工程无需预先有人物或章节；AI 自主选择方法与工具，Host 不按关键词路由。

NR-401～406 已通过各自完整质量门。NR-G4 保持已批准；NR-G5 技术提交完成，等待产品批准。实施范围以已批准 NR-4-D1 为准，不进入 NR-5。真实 Provider Live Smoke 未执行，本次最小只读检查明确放弃；NR-6 完整语义/兼容性验证保留。

## 运行结构

```text
React 三栏
  ├─ Conversation：自然回复 + Host 保存结果 / 局部 Diff / 确认 / Undo
  └─ Markdown editor：Host 编辑保护回执之后才可输入
          ↓ 28 typed IPC；Conversation 仍为 3 条 + 原 Event
Rust Host
  ├─ Conversation Store → Memory / 最多 1 次 Compactor
  ├─ 内置 Novelist 1.1.0：9 能力 / 37 资源
  ├─ 最多 4 次 Agent：最多 2 读 + 1 变更 → 最终无工具回复
  ├─ read_context：实际方法/原文 → 完整回执
  ├─ propose_change：来源/读取校验 → 项目事务协调 → Canon Kernel
  ├─ Change Store：不可变快照 / 持久提交 / CAS / Undo / recovery
  └─ Provider-native adapters + Windows CredentialVault
```

写权限会话注册 `novelist.read_context` 与 `novelist.propose_change`，wire 名分别为 `novelist_read_context`、`novelist_propose_change`。无写 Lease 会话保留只读路径。能力包无效时原子降级为无工具聊天，不加载用户 Skill、外部缓存、Python 或任意 Shell。

## 数据事实源

| 数据 | 位置 | 边界 |
| --- | --- | --- |
| Canon | 工程 Markdown 与 assets | 作者内容；不因状态清理删除 |
| 可见 History | `.novelgainker/conversation/session.json` | 完整自然消息，不保存工具协议 |
| Memory | `.novelgainker/memory/state.json` | 可丢弃五栏目摘要，仅从可见 History 派生 |
| Task | `.novelgainker/tasks/session.json` | 有界状态、Hash、字节数，不存原始参数/结果 |
| Change | `.novelgainker/changes/{changeId}.json` | 规范化候选、不可变 before/after、来源引用、回执、Undo 意图 |

项目元数据仍为严格 schemaVersion 2。`.novelgainker/**` 不进入小说索引/出版。原生 Continuation 仅存当前 Turn 内存，不进入上述 Store、IPC 或日志。

## 五类维护与来源

| 类型 | 目录 | 能力方法 |
| --- | --- | --- |
| character | characters | create-character |
| world（frontmatter kind=setting） | world | create-setting |
| material | materials | create-material |
| plot | plot | create-plot |
| outline | outlines | novel-story-telling |

新建由 Host 分配 UUID 和文件名，候选使用 `id: new`。更新须完整读取目标、Hash 匹配、保留身份；搜索摘要/截断方法不产生写入证据。分段读取必须连续且完整。当前 AI 写入面限小型规范 UTF-8 文档；不能证明规范化文本与磁盘字节相同时拒绝更新，不擅自转换编码。

Host 从持久 Conversation 绑定消息 ID、角色和 Hash：用户决定至少引用当前用户消息；接受建议同时引用原建议；AI 建议只留对话；正式章节独立来源延至 NR-5。用户明确引用章节的资料任务仍可执行，但章节本身不能通过 NR-4 写工具修改。

来源真实性不等于语义授权真实性。模型判断是否接受建议、是否只读、是否有语义矛盾；Host 不伪称 Hash 能证明意图。结构性丢失、风险或编辑占用升级为待确认；范围不明拒绝并重新准备，无 force-overwrite。

## 事务、读隔离与 Undo

同工程共享短事务 Mutex，持有跨进程 Writer Lease；等待 Provider 时不持事务锁。手动保存与 AI 保存共用协调。先持久化快照和 Applying，逐文件走安全原子写入，再校验整批并保存 Committed 标记。

提交标记前失败只在目标仍等于本次 afterHash 时逆序恢复；外部改动不覆盖，保留 recovery_required 并阻止新写。提交后 Provider/Task 失败不否认已保存。重开先恢复变更，再按回执对账 Task，不重放 Provider。Task succeeded 不是已保存证据。

应用内列表/搜索使用一致读；编辑器单文件读与写事务互斥。工具只读在恢复未决时明确标注 snapshotConsistency=recovery_required，且不签发写回执。多文件只保证应用协调与可恢复部分失败，不承诺对任意外部编辑器原子可见。

Undo 是显式本地逆事务：全目标 afterHash/CAS/dirty 预检后写入 undoRevision 和 Applying；创建移除本次未修改文件，更新恢复 before-image；全部完成后标 Undone。逆事务未提交中断时恢复到撤销前 Committed，不能误报从未保存。原始快照不翻转。外部修改时保留证据；“重试安全恢复”仍只接受已知版本，恢复后重开工程刷新写权限状态。

## UI 与 IPC

新增 `canon_change_list`、`canon_change_detail`、`canon_change_decide`、`canon_editor_state`，生产 IPC 共 28 条。decide 仅接受工程 Session、changeId、revision 和封闭动作 confirm/discard/undo/retry_recovery；编辑登记仅接受 opaque fileId、revision 和布尔值，不新增任意路径/正文写入口。Manifest、Handler、Permission、Capability、TS Client 五层精确一致。

编辑器读取时先在 Host 保留编辑占用；干净失焦释放，重新聚焦须收到登记回执才能输入，脏草稿始终保留保护。干净但正在聚焦编辑也保守占用，AI 可能转待确认。过期候选不能仅靠点确认覆盖，应重新读取准备。

Host 回执显示“已保存 / 待你确认尚未保存 / 无变化 / 待恢复 / 已撤销”。Diff 来自保存的原文与候选，不来自模型自述。决定成功或失败后都重新加载事实；局部结果读取失败不禁用聊天。保存刷新文件树，未保存编辑不被刷新覆盖。

Event 是低延迟路径，Store 是终态真相；原有非重叠对账与监听就绪要求保留。所有非空 AI 文本保留；只有真正未完成的 Provider 终态附加未完成提示。待确认 AI 草稿不增加关窗阻断；仅手工未保存 Markdown 可触发保存/放弃/取消，正式关闭立即退出，不等待 AI。

## 冻结预算

NR-4 最多两读一变更、4 Agent + 1 Compactor、累计 17,408 请求输出 tokens。单候选正文合计 8 KiB、最多 4 目标；Tool Result 24 KiB；累计 Continuation 160 KiB；循环预留 192 KiB；实际 Provider Body 256 KiB。每次发送检查完整序列化体，写入前另预留最终结果封装空间。零自动重试、零并行工具。

变更库 64 MiB / 100 条 / 10 待确认；实际容量包含临时文件，预留事务及 Undo 状态空间。配额耗尽不自动清除历史，不删除 Canon；丢失快照不能重建历史 Undo。

历史 NR-3 只读回退仍保留 2 Agent、1 Tool、96 KiB 的合同回归；不是 NR-4 写权限会话的预算。最终验证与已知限制见 [交付状态](sprint-plan.zh-CN.md) 和 [实施记录](reviews/novelist-agent-runtime-nr4-implementation-log.zh-CN.md)。
