# NovelGainker 当前实现架构

核验：2026-09-09，源码基线 `34f4e39`。本文描述当前实现；分阶段自主、暂定事实验收和有限自动恢复属于[目标策划](product-spec.zh-CN.md)，尚未实现。历史合同保留原时点，不能套用其旧参数描述当前运行。

## 结构与主链路

Windows、本地工程、简体中文，React三栏连接Tauri 2 Rust Host。左栏为项目文件树、会话和Provider，中间为自然对话及内联执行结果，右侧为真实Markdown编辑器。两侧宽度通过拖动调整。没有独立Task Center或用户选择Skill的工作流。

```text
React 三栏界面
    ↓ 38个受限IPC；流式事件 + Store对账
Tauri commands / 项目与会话协调
    ├─ Conversation：先存用户消息、Running与attempt，再启动worker
    ├─ Context：当前会话历史、派生摘要、内置方法、来源与工具事实
    ├─ Agent：Provider回复 → Host工具 → 原生协议续接 → 有界结束
    ├─ Canon：来源/完整读取/Hash → 编辑保护 → 事务 → Committed
    ├─ Publication：当前定稿快照 → 预检/本地EPUB → 产物回执
    └─ CredentialVault：Windows Credential Manager，Rust内使用
```

主入口为 `apps/desktop/src-tauri/src/commands_current.rs`；运行链在 `crates/app-core/src/conversation_runtime.rs`、`conversation_change_runtime.rs`。同物理工程同时一个运行，worker捕获发起会话身份；切换界面不改变结果归属。每个会话拥有独立历史、派生记忆与Task，但共享当前Canon。

AI按语义选择工具和方法。内置Novelist应用投影1.4.0，9能力/37资源；不动态加载外部插件缓存、用户SKILL、Python或任意程序。能力包不可用时降级为无工具聊天。方法目录不代表所有方法都有执行器。

## 工具与有界执行

当前写权限工程提供三个具体工具，wire名称将点改为下划线：

| 工具                      | 实际执行范围                                                           |
| ------------------------- | ---------------------------------------------------------------------- |
| `novelist.read_context`   | 方法、请求的资源、项目资料/章节等受控读取，签发完整读取与来源回执      |
| `novelist.propose_change` | 人物、设定、素材、剧情、大纲，以及每轮至多一个章节；严格来源和版本约束 |
| `novelist.publish`        | 本地EPUB预检或构建；build自带预检，不执行平台发布或图片生成            |

五类资料对应 `characters/world/materials/plot/outlines`。新建由Host分配标识和路径；更新必须完整读取原文、保留身份并匹配Hash。搜索摘要和截断方法不能充当完整读取。章节另有32 KiB与元数据合同。当前NR-5方法读取自动提供指令及Manifest声明的完整必需模板（requiredTemplates），模板与可选参考共用6 KiB正文预算，完整结果继续受64 KiB和回执预留约束。参数诊断最多4项，仅返回Host字段路径、类型和有界枚举；候选提示明确版本、kind及换行要求。具体范围见[实现切片A记录](reviews/implementation-slice-a-2026-09-10.zh-CN.md)。

| 当前NR-5运行边界                      | 上限                                                       |
| ------------------------------------- | ---------------------------------------------------------- |
| Agent / Compactor请求                 | 8 / 1；最后一次Agent无工具                                 |
| 读 / 变更 / 出版工具                  | 4 / 2 / 1；串行，总计最多7次                               |
| 累计请求输出                          | 33792 tokens                                               |
| 五类资料一次候选                      | 至多4目标、正文合计8 KiB                                   |
| 章节                                  | 每轮至多1章，完整文件32 KiB                                |
| 工具参数 / 结果                       | 各64 KiB                                                   |
| 累计原生Continuation / 单次实际请求体 | 512 KiB / 768 KiB                                          |
| 变更库                                | 256 MiB、2048记录、单记录512 KiB；保留待确认及事务预留限制 |

每次请求检查完整序列化体，文件副作用前预留结果空间。单独preflight也占一次出版额度，当前同轮不能先独立预检再调用build。NR-3/4旧预算只用于对应历史合同回归，不作为当前写权限工程上限。

这仍是单轮有界Agent；不存在跨轮StageRun或自动串行完成整部书的调度器。当前对话网络请求零自动重试。符合条件的最新首轮零输出、零工具失败可由用户显式重试一次，沿用原轮预算；具体资格在 `conversation_retry.rs`。有限自动恢复是新目标，需要另建运行合同。

## 文件与事实来源

| 数据                 | 位置与权威                                                                          |
| -------------------- | ----------------------------------------------------------------------------------- |
| Canon                | 工程Markdown和受控资产；唯一稿件来源                                                |
| 默认会话History      | `.novelgainker/conversation/session.json`                                           |
| 默认会话Memory       | `.novelgainker/memory/state.json`；从可见历史派生，可丢弃，不等于全书记忆           |
| 默认会话Task         | `.novelgainker/tasks/session.json`；受控状态、Hash、字节数，不保存原始工具参数/结果 |
| 新会话               | 上述conversation/memory/tasks各自的 `sessions/{UUID}.json`；默认旧路径保留          |
| 会话目录             | conversation内的有界目录；32非归档、256总条目                                       |
| Change / Publication | `.novelgainker/changes/`及出版记录；不可变快照、来源和持久结果，独立于聊天终态      |

项目元数据为严格schemaVersion 2。内部状态不进入稿件索引/出版。隐藏提示、Provider原生Continuation、密钥及绝对路径不进入项目状态、IPC或日志；Continuation只在当轮内存。完整聊天落盘不表示模型能同时读到全部历史。

History单会话16 MiB/10000消息，Task 4 MiB/10000条，Memory 32 KiB；项目聚合上限分别512 MiB/128 MiB/8 MiB，目录另有限额。运行前预留容量，满额不自动删除历史或Canon。详细容量合同见[NR-601](reviews/novelist-agent-runtime-nr601-engineering-contract.zh-CN.md)。

聊天分支只复制所选终态轮次之前的可见聊天，不复制执行/变更回执、不重放工具、不回滚稿件。归档仍占存储，可恢复清空通过归档旧会话并创建新会话实现。导出仅包含允许的可见数据，经本地保存对话框获得具体目标授权，不覆盖旧文件。[NR-602合同](reviews/novelist-agent-runtime-nr602-engineering-contract.zh-CN.md)保留完整约束。

## 提交、编辑、恢复与定稿

Host绑定持久消息引用、方法/原文读取回执和Hash；这些能证明来源真实性，不能证明模型正确理解了作者意图。建议本身不是正式写入授权。当前章节及资料维护采用实际来源与当前章节依赖，过期依赖不能悄悄当成有效事实。

同工程短事务协调加跨进程Writer Lease，网络请求不持事务锁。变更先持久化不可变before/after快照和Applying，再安全替换并核验全部目标，最后记录Committed。只有Committed表示文件保存；后续模型回复、Task落盘或UI读取失败不能否认已有提交。

提交前失败只在目标仍匹配本次写后Hash时安全恢复；未知外部改动保留 `recovery_required`，阻止新写，不据此禁用聊天。重开核对事务与Task，不自动重放Provider。Undo同样是带意图和回执的逆事务，必须全目标匹配afterHash；不能删除恢复证据或强制覆盖解决冲突。多文件操作不承诺对外部编辑器原子可见。

编辑器输入前取得Host编辑保护，脏稿始终受保护。保存与AI写入共享协调；刷新文件树不能覆盖未保存编辑。确认/撤销IPC只接受opaque标识、revision和封闭动作，不接受任意执行路径或替换字节。

章节定稿绑定具体版本，修改或Undo使旧定稿依据失效。EPUB消费当前有效定稿、项目出版信息和受控已有资产，预检通过不等于已生成。当前仅有受限Markdown及PNG/JPEG支持；本地结构校验不代表第三方EPUB认证。

## 对话与界面事实

发送前持久化用户消息和Running。Event提供低延迟显示，Store提供终态真相；监听就绪与非重叠对账保证结果归属，对账不调用Provider。非空异常回复保留并标记未完成；零输出才显示“本次回复未生成”。流式文字未逐段持久化，异常退出可能失去尚未落盘片段。

工具详情已有内联折叠UI，展示工具、状态、时间、记录编号和受控错误；完整结果回放仍未提供。文件保存、聊天生成、章节定稿和EPUB生成是不同状态，不能用一个“已完成”取代。

仅手工未保存Markdown可触发保存/放弃/取消的关窗选择，选择前不取消AI。确认关闭立即取消当前轮并退出，不等待Provider或清理完成；待确认AI候选不增加关窗阻断。重开不自动产生模型费用，也尚不能恢复跨轮阶段任务。

## 验证边界

现行生产IPC共38条，Manifest、Handler、生成Permission、Capability与生产TS客户端必须精确同步。当前源码及已记录证据的边界见[产品现状](product-current-state-report.zh-CN.md)、[交付状态](sprint-plan.zh-CN.md)。脚本化Provider证明Host及协议行为，不证明真实模型语义、小说质量或完整多用户验收。本次文档核验没有运行新的测试。
