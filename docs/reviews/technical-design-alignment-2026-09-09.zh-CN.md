# 技术方案专项对齐与访谈（2026-09-09）

状态：2026-09-10更新。第一轮源码审查与独立工程修订已完成，本轮已列技术取舍均已确认，实施与验证仍未完成；Q1～Q17的质量与作者验收原则保留，检查触发方式按新T2/T2a修订。技术问题使用T编号。此次任务是查找问题、对齐设计并访谈，不实施生产代码、运行真实Provider或重写历史Gate。

## 基线与范围

- 当前源码HEAD：`34f4e39219d7749aaa10fd640eee22f6ec634357`；开始本轮时已有14份上轮授权的文档变更，保留。
- 技术方案v1审查前SHA256：`afee31d269e477c95f79345bcfcb9f74b64c2bda0fbb5470553777b5c954009e`，252行/31367字节；副本存于 `target/qa/technical-design-v1-before-alignment.md`，用于对比，不冒充历史Gate。
- 67份文档/规则/状态文件沿用前轮盘点并记录当前尺寸与Hash，见 `target/qa/technical-alignment-inventory.json`。本轮重点是目标技术方案，已有当前实现审计和产品决定不重复推翻。
- 两个只读子代理已完成状态/迁移/定稿及Provider/预算/流式/重试事实核查并关闭；主代理抽查关键源码，检查生命周期、协议幂等和用户交错。全程没有编辑生产文件、运行exe/测试或执行Live。

## 判定原则

产品选择已定，不把工程师可自行核验的事实或字段常量转成用户问题。只在持久化保证、时间/费用、兼容与操作体验等取舍确实影响产品承诺时提问。新建议在收到回答前不得成为已批准目标；纯技术矛盾可直接修正文档并留下原因。

## 设计树

```text
Q1～Q17（已确认）
├─ 用户可观察保证
│  ├─ T1：先持久后显示，接受短暂批量延迟（已确认）
│  └─ T2/T2a：逐轮关键词及别称/指代上下文审核；正式审稿仅主动请求（已确认）
├─ 持续执行与状态一致性
│  ├─ 执行状态 / 作者验收 / 当前有效性分离（工程处理）
│  ├─ 控制状态与Canon提交的原子边界（已写工程修订）
│  └─ 普通插话的调度时机（G11/T3已确认输入优先）
├─ 费用与重试
│  ├─ 请求尝试 / 操作意图 / 稳定效果标识（工程处理）
│  └─ 不确定费用的重开恢复与预算消费（已写工程修订）
└─ 长期数据和交付
   ├─ 定稿元数据变化与验收衔接（工程处理）
   ├─ 老版本打开新状态的隔离（G12/T4已确认副本升级及旧版拒绝）
   └─ 最小可用闭环的切片顺序（工程处理）
```

## 问题清单与当前处理

下列位置按审查前v1行号记录；不是运行时缺陷复现声明。

| 编号 | 级别 | v1证据与后果 | 处理方向 / 当前状态 |
| --- | --- | --- | --- |
| TD-01 | 高 | §4.3第76行将StageRun.completed定义为作者已验收；运行生成完成、作者否决、部分验收失效都挤入同一状态 | v1.1 §4.3已分离执行、验收及版本有效性，补事件/转换表 |
| TD-02 | 高 | §4.2/§5说决定、预算、事实确认一致落盘，但未指定唯一事务域；与Canon多文件写入不是同一事务 | v1.1 §4.2明确项目StageControlStore与Canon各自权威、数据库事务及跨Store恢复协议 |
| TD-03 | 高 | §4.4每条输入都增加control revision，导致无关聊天也使旧候选永远过期；单个在途请求又可能拖住即时回应 | v1.1 §4.4已区分输入屏障与授权epoch；G11/T3已确认普通输入优先，保留已生成和已提交成果 |
| TD-04 | 高 | §7同时要求稳定operation标识、不同内容拒绝，又要求参数错误改正后重试，缺少无副作用纠错与已提交重写的区分 | v1.1 §7区分稳定效果槽、不可变意图版本和物理请求；已提交改写须新获准步骤 |
| TD-05 | 高 | §7允许最后已显示片段丢失，Q11的“保留进度”没有量化范围 | T1已确认先持久后显示；v1.1 §7及§13.2落实保证与反例，尚未实施 |
| TD-06 | 中 | v1将自动核对与编辑审稿混在一起，默认审稿不符合用户想要的触发方式 | 2026-09-10 T2/T2a已确认；§5分开每轮相关上下文审核与作者主动审稿，移除阶段/全书自动审稿与费用预留 |
| TD-07 | 高 | §10定稿改变status并使用全文件Hash，而阶段验收绑定之前的draft版本；未定义受控元数据变换如何衔接 | v1.1 §10已增加精确前后Hash与状态差异限定的批准衔接凭据，手改final不授予批准 |
| TD-08 | 高 | §12要求旧程序不能清理新状态，但仅新增目录未说明旧exe如何识别并拒绝；可能只是一条愿望 | 旧入口仅检查工程schema，独立Stage schema无效；v1.1 §12写明事实与升级候选，G12/T4已确认保留原工程、副本验证迁移及旧版拒绝新格式 |
| TD-09 | 中 | §12 B/C最小闭环早于D暂定事实/验收，可能先推出违反Q6/Q7的阶段自主 | v1.1 §12改为B/C/D最低能力合成首条可用闭环，再扩展复杂事实/返工 |

## 源码补充问题与修订

此处“已修订”只表示设计已明确，不能据此宣布生产缺陷已修复。源码均来自本轮HEAD；有关竞态为静态分析结论，没有进行故障复现。

| 编号 | 级别 | 当前实现证据 / 缺口 | v1.1处理 |
| --- | --- | --- | --- |
| TD-10 | 高 | [chapter.rs:81](../../crates/app-core/src/canon_change_store/chapter.rs)同一Turn仅一条有效章节；[canon_change.rs:301](../../crates/app-core/src/canon_change.rs)章节仅一个快照；[publication.rs:252](../../crates/app-core/src/canon_change_store/publication.rs)逐章检查final | §10新增整书批次、每章回执及完成标记；所有出版入口检查整书完成，部分成功不能导出冒充整书 |
| TD-11 | 高 | [canon_change.rs:264](../../crates/app-core/src/canon_change.rs)限制章节携带依赖；[continuity.rs:126](../../crates/app-core/src/canon_change_store/continuity.rs)现核验不能表达完整语义依赖图 | §5新增版本化事实引用、影响队列、逐项复核；Hash没变也可能需复核，不把模型判断当Host证明 |
| TD-12 | 高 | [commands_sessions.rs:94](../../apps/desktop/src-tauri/src/commands_sessions.rs)无活动请求则恢复旧Turn/释放预留；[management.rs:162](../../crates/app-core/src/conversation_sessions/management.rs)预留阻断会话管理；[commands_sessions.rs:58](../../apps/desktop/src-tauri/src/commands_sessions.rs)别名权限有不同边界 | §4.5分开阶段归属与请求预留；捕获运行会话身份；G11/T5已确认暂停并归档，恢复后明确继续 |
| TD-13 | 高 | [conversation_retry.rs:39/112](../../crates/app-core/src/conversation_retry.rs)持久化次数与请求输出上限，并非实际usage；[provider_core.rs:474/1399](../../crates/app-core/src/provider_core.rs)有body边界及usage解析但无完整账本；[ConversationView.tsx:345](../../apps/desktop/src/components/ConversationView.tsx)忽略usage | §8独立物理RequestAttempt，序列化后/HTTP前预留，prepared与dispatching恢复不同；Rust去重结算，未知不释放 |
| TD-14 | 高 | [commands_current.rs:234](../../apps/desktop/src-tauri/src/commands_current.rs)置取消token不代表所有写入停止；[conversation_runtime.rs:224/590/769](../../crates/app-core/src/conversation_runtime.rs)仍有收尾持久化；[canon_change_store.rs:427](../../crates/app-core/src/canon_change_store.rs)存在检查/提交时点 | §4.4用短提交边界确定停止先后，区分正在停止/已停止；保留已提交及必要收尾记录 |
| TD-15 | 高 | [conversation_runtime.rs:679](../../crates/app-core/src/conversation_runtime.rs)走单次流式；[provider_core.rs:1938/2614](../../crates/app-core/src/provider_core.rs)通用路径有内部重试，旧Retry-After仅整数且截到2秒 | §7协调器独占恢复循环，底层一次发送；等待超出边界则暂停，不截短后提前发送；请求各自计账 |
| TD-16 | 中 | [feedback.rs:7](../../crates/app-core/src/tool_runtime/feedback.rs)已有至多4条路径诊断；[conversation_change_runtime.rs:157](../../crates/app-core/src/conversation_change_runtime.rs)按callId判重；[canon_change_store.rs:107/384](../../crates/app-core/src/canon_change_store.rs)Succeeded也可为NoChange/PendingConfirmation | §6/§7补期望值等有界诊断和无效果判据；按输入/来源/错误指纹抑制重复，进展必须有实际凭据 |
| TD-17 | 中 | v1 §8写“按阶段授权”，未落定阶段出现前的规划和无阶段聊天预算归属 | §8规定每个付费请求都必须绑定已授权预算；返工/换run不清零，默认额度展示方式由交互合同补齐 |

TD-02进一步依据：[commands_current.rs:111](../../apps/desktop/src-tauri/src/commands_current.rs)、[app_state_current.rs:374](../../crates/app-core/src/app_state_current.rs)显示现有SQLite为应用级；[task_store.rs:633](../../crates/app-core/src/task_store.rs)独立保存Task；[canon_change_store.rs:422/485](../../crates/app-core/src/canon_change_store.rs)多文件事务逐项应用后写自身Committed。这些Store没有共同的数据库提交点。

TD-05进一步依据：[conversation_runtime.rs:685](../../crates/app-core/src/conversation_runtime.rs)先追加内存并发送事件，[ConversationView.tsx:324](../../apps/desktop/src/components/ConversationView.tsx)立即显示，[conversation.rs:275](../../crates/app-core/src/conversation.rs)到终态才保存正文；不是已实现“只可能丢最后一小段”。[event-cursor:57](../../apps/desktop/src/lib/conversation-event-cursor.ts)拒绝旧事件不代表能补齐丢失正文。

TD-07进一步依据：[chapter.rs:194](../../crates/app-core/src/canon_change_store/chapter.rs)定稿修改status，[chapter_contract.rs:135](../../crates/app-core/src/chapter_contract.rs)Hash覆盖完整文件；不得以忽略status来假装版本未变。

TD-08进一步依据：[project.rs:936](../../crates/app-core/src/project.rs)未知工程schema才拒绝，仍合法的v2不会因为新增Stage目录被拒；[coordination.rs:40/63](../../crates/app-core/src/coordination.rs)lease是进程协调，不能持久阻止旧exe。未发现v2自动清理未知阶段目录的证据，不能夸大为已确认删除漏洞。

## 第一轮技术问题（T1、T2、T2a已确认）

- T1 已确认：用户答复“1. 按建议”。正常本机磁盘前提下，先持久化小批可见文字再显示，接受约0.2～0.5秒批量显示延迟；保证针对应用崩溃/强制退出。该时间是刷新目标，落盘失败或延迟时不得抢先显示。对话/工作草稿与正式Canon提交仍不同。
- T2 已由用户纠正并确认：每次对话涉及关键词即开始上下文审核；正式审稿仅在作者向app的AI提出审稿请求时触发。之前的默认独立阶段审稿建议撤回，不再保留为待批准选项。

- T2a 已确认：用户选择“包括别称和指代，结合前文识别”。“他”“那枚钱”等也应关联前文实体并自动核对，不能只做字面完全匹配。

2026-09-10修订依据：用户在理解阶段审稿示例后明确表示那不是想要的行为，要求逐轮关键词上下文审核，审稿只在作者主动要求时触发。因此同步修订产品流程、技术§5及预算/验收/交付用例和CLAUDE规则。Q12质量目标、Q14整书一次确认继续成立；原“全书检查”改为已有上下文核对结果的汇总和必要格式/版本检查，正式编辑审稿须另有作者请求。不得将自动审稿换名“核对”后照旧每阶段运行。

普通话题审核必须有相关来源、版本和覆盖边界；冲突可在对话中讨论，不能自动修改已确认内容。审稿请求按真实作者意图与范围识别，引用中出现审稿字样或否定请求不触发。未请求审稿不能阻断阶段验收/整书确认，也不产生对应调用或费用预留。

此前列出的普通消息抢占、旧版本使用边界及未完成阶段归档方式现已随G11/G12确认。默认预算、报价不可得和不确定用量必须落实已批准Q10，字段常量由工程师核验，不再重复询问产品原则。

## 第二轮技术问题（T3～T5随G11/G12已确认）

T2/T2a修订已完成并通过文档校验；以下是原技术访谈继续核对的独立取舍，不是完成上下文审核定义的前置条件。此前已发出问题卡；用户随后要求一次列出全部问题、不再逐条回答，故T3/T5合并为G11、T4合并为G12，不再另行催答。用户随后对9～15整套建议答复“按建议”，T3～T5已一并确认。

- T3 已确认：优先回应作者，取消在途生成并保留已持久内容与已提交成果，随后执行相关上下文审核；取消仍可能计费。普通讨论后原授权仍有效可续做，明确停止始终优先。
- T4 已确认：保留原工程，迁移副本验证通过后作为新工作工程，使用旧版实际拒绝的格式；旧版可使用升级前工程，升级后的进度不自动向旧版合并。
- T5 已确认：“暂停并归档”，保留稿件/预算/进度；恢复会话后仍需作者确认继续。

## 15项技术缺口的集中处理

用户答复“1-8按建议，然后9-15你也给我出一版建议”。G01～G08已确认：实体关联生命周期、故事时间/知情范围、聊天批准落资料、流式补查、冲突决定闭环、失败降级、阶段成果对应和防反复改写。主方案§5.3及产品说明已同步，详细见[缺口处理方案](technical-gap-closure-proposal-2026-09-10.zh-CN.md)。

用户对9～15整套建议答复“按建议”，G09～G15现已确认：工具协议、用途预算与不确定费用、插话/归档、副本迁移、分段流式保存、恢复/定稿故障测试及审核专项验收。统一提供建议，不再逐条发问题；T3～T5此次依据明确的整套采纳答复成为已确认。参数与百分比是候选验收目标，不是当前程序成绩。本次仍仅修改文档。

## 本轮工作与检查状态

- 已完成首轮事实审查、17项设计缺口登记及与答案无关的工程修订；T1、T2/T2a、G01～G15及合并的T3～T5均已确认，本轮已列取舍无待答项。
- 已更新目标技术方案v1.1对齐稿；2026-09-10同步产品v1.1及CLAUDE中的T2/T2a目标边界。原v1副本、原始Q1～Q17访谈及历史Gate/Story证据保留，目标文档的检查触发方式已按新决定修改。
- 新增反例覆盖状态、跨Store中断、意图纠错、逐请求账本、会话/停止、整书批次和旧版本保护。只是验收设计，没有运行这些用例。
- 本次T2/T2a修订另验于 `target/qa/context-audit-alignment-check.json`，原技术文档检查记录于 `target/qa/technical-alignment-doc-check.json`；检查只验证引用、声明与文件范围，不宣称代码质量、运行时恢复或新增功能通过。

## 外部技术依据

SQLite的事务承诺针对数据库内的变更，不能据此声称任意Markdown文件替换已被包含在同一数据库事务里；此处仍需应用层意图与Canon回执核对。这是结合本工程文件结构做出的设计判断。[SQLite原子提交说明](https://www.sqlite.org/atomiccommit.html)

如控制状态采用WAL，必须同时设计checkpoint和一致备份，不能把运行中的主数据库文件直接复制视为完整备份。[SQLite WAL](https://www.sqlite.org/wal.html)、[官方备份API](https://www.sqlite.org/backup.html)
