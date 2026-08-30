# NovelGainker 当前架构

> 状态：AM-R1 / GAR1 技术提交基线（2026-08-30）  
> 产品门：`gar1Approved=false`；AM-R2、AM-R3、Sprint 6 与真实 Provider Live Smoke 均未授权

## 1. 产品定义

NovelGainker 是 Windows-first、local-first 的 Tauri 2 桌面小说创作应用。作者在工程内与 AI 自然对话；小说 Markdown 与资源文件是唯一 Canon。角色、世界、素材、剧情和章节都可以先讨论再落地，创建工程时不强制预设任何锚点。

当前 AM-R1 只交付“可阅读工程上下文的对话基础”。Agent 可以选择 Novelist-compatible Skill、读取相关 Canon、展示来源并保存工程内聊天与记忆，但 Agent Canon 写入固定为 0。Canon 化、Context Mark、冲突决议和长章写入属于后续独立 Gate。

## 2. 工作区与入口

应用启动时必须先让用户新建工程或打开已有工程。新工程只创建最小 Scaffold，不自动生成角色、世界观、剧情或 Story Ledger。

项目打开后保持三栏常驻：

- 左栏：工程文件、Provider 设置与项目操作；
- 中栏：纯 Conversation，只显示用户与 AI 的可见消息、输入框和必要错误；
- 右栏：Agent 活动，包括状态、所选 Skill、实际来源、Memory、累计预算和历史清理。

中栏不得混入 Control JSON、工具参数、隐藏推理、原始 Provider Payload、Diff 或写入控制。

## 3. Canon 与工程结构

Canon 只来自受支持的工程 Markdown 与资源：

- `project.md`
- `characters/**`
- `world/**`
- `materials/**`
- `plot/**`
- `chapters/**`
- 受支持的工程根约束文件

`.novelgainker/agent/**` 是随工程携带的 Agent 元数据，不是 Canon。它必须从普通工程搜索、Context Canon Index、出版输入和普通 Skill 读写范围中排除。

所有项目访问都由 Rust 持有的 `ProjectHandle` 和不透明 Session ID 绑定。Provider、WebView、Skill 与工程内容不能提供绝对路径、路径片段、Shell、Process、URL、权限或写入决定。

## 4. 五个协议平面

当前实现严格分离五个平面：

1. **Conversation**：用户可见自然语言消息；Provider `conversation` 调用返回纯文本流。
2. **Control**：`SkillSelectionV2` 或 `AgentControlActionV2` 的严格 JSON；两者互斥，不得作为聊天正文展示。
3. **Content**：Host 从 Canon Index 读取的有界、不可信 Source Result。
4. **Canon**：小说工程 Markdown；AM-R1 Agent 无写权限。
5. **Persistence**：工程内 History、Memory、来源和分支元数据。

Provider 每次调用只使用一种固定模式。AM-R1 激活 `control`、`conversation` 和按阈值触发的 `memory_compaction`；Composition、Reconciliation、Context Mark、Conflict 和 Write 全部 Policy Blocked。

## 5. Agent Protocol V2

生产 Agent IPC 固定为七条：

- `agent_skill_registry_load`
- `agent_session_load`
- `agent_turn_start`
- `agent_turn_cancel`
- `agent_session_clear`
- `agent_memory_compact`
- `agent_history_branch_resolve`

典型 Turn 顺序：

1. Host 校验工程 Session、当前 Provider、History 状态和 GAR1 Budget Profile。
2. Provider 以 `control/skill_selection` 从完整 eligible Skill 集合做语义选择。
3. Host 根据已选 Skill 和用户意图规划、执行 Canon Context Query；Provider 无路径控制权。
4. 如需多轮 Context，Provider 只能返回严格 `AgentControlActionV2`；Host 决定查询是否合法并累加来源。
5. Provider 以独立 `conversation` 请求生成用户可见文本流。
6. Host 写入终态 History Record、来源和增量 Memory；Agent Canon 写入数仍为 0。
7. 取消、失败、预算耗尽或损坏都形成稳定、可见、可恢复的终态。

Control 与 Conversation 不共用输出契约。Conversation 请求不得要求 JSON，Control JSON 不得进入中栏消息。

## 6. Bundled Skill Runtime

发行包内置并锁定九项 Novelist-compatible Skill：

- `create-novel-project`
- `create-setting`
- `create-character`
- `create-material`
- `create-plot`
- `novel-story-telling`
- `analytical-review`
- `create-visual-asset`
- `publish-novel`

Runtime Loader 读取完整 instructions、templates 与 references，验证来源、许可、Manifest 和每个资源 Hash。生产运行时不读取 Codex/Novelist 插件缓存、用户 `SKILL.md`、Python sidecar、用户目录或网络 Skill。

Skill 选择是对完整 eligible 集合的语义选择，不使用关键词过滤。Skill 只能影响写作方法与 Context 需求，不能授予工具、路径或写权限。

## 7. Context Index 与 SourceRef

`ProjectContextIndexV2` 每次从当前 Canon 重建，不把数据库或 Agent 元数据当作隐藏事实来源。索引：

- 只遍历批准目录和文件类型；
- 拒绝绝对路径、遍历、隐藏目录、符号链接/联接逃逸、非 UTF-8 和超限文件；
- 按内容 Hash、相对路径和语义信息生成确定性来源；
- 为每个结果返回稳定 `sourceId`、相对路径、内容/摘录 Hash、标题路径和有界摘录；
- 把所有工程内容标为 `untrusted=true`。

右栏显示实际读取来源。跨人物/关系、世界规则、剧情线程和最近章节的来源在同一 Turn 中只增不丢；宽泛 n-gram 不得制造无关命中。

GAR1 规模门使用不少于 120 章、50 万中文字符、48 个真值查询的确定性合成工程，并要求 Canon Hash 前后相同、Agent 元数据/项目外命中为 0。

## 8. History、Memory 与同步分支

首次创建工程聊天历史前，应用必须说明完整可见对话将写入 `.novelgainker/agent/`，可能随 Git/同步工具复制，并可能包含私人创作内容。用户同意前不创建 History。

History 采用工程隔离的 JSONL Hash Chain，记录可见用户/助手消息、终态、Skill 和 SourceRef。单条消息、单 Turn 来源数、ID、路径、Hash 与正文长度均有固定上限。损坏历史隔离处理，不把损坏正文回显到错误。

若同步产生多个有效 Branch，应用必须要求用户显式选择当前 Branch；不得自动合并、覆盖或删除。损坏 Branch 不可选择。

Memory 由 Host 在每个终态 Turn 后增量维护，保存来源绑定的摘要与计数。只有 `needsCompaction=true` 或用户显式操作时才调用 `memory_compaction`；请求绑定 Session ID 与当前 Memory Hash，防止陈旧结果覆盖。

清空当前会话或全部历史只删除 `.novelgainker/agent/**` 中对应状态，不改 Canon。

## 9. Provider 与预算

DeepSeek 与 OpenAI 使用固定、只读的 Provider Preset、TLS Host、模型和协议。Key 只存 Windows Credential Manager；项目、SQLite、前端状态、日志、证据和错误不能包含 Key、掩码、尾号、Hash 或指纹。

两个 Adapter 归一化到同一 V2 调用与事件模型：

- 同一时间最多一个活动 Provider Run；
- 取消信号贯穿响应头、流式正文和 Host 安全点；
- 不自动跨 Provider Fallback；
- 格式修复最多一次且只能留在同一 Provider/同一模式；
- 原始 Payload、隐藏状态和敏感输入不进入 WebView 或证据。

`BudgetProfileV2(scope=gar1_read_only)` 累计 Provider Calls、Input/Output Tokens、Context Results、修复次数和耗时。所有重试共享同一个计数器；未激活能力预算固定为 0，不能借重试或模式切换绕过。

## 10. 持久化与恢复

App State 继续保存 UI、最近工程、Provider 非秘密状态以及旧事务恢复所需的有界元数据。它不能成为 Canon。旧 App State reader 仍用于安全打开和终结已有状态，但旧产品入口不再注册。

工程内 Agent 数据使用已知文件名、原子替换和 Hash 绑定。任何损坏、版本过新、Hash 漂移或分支冲突都必须在写入前失败。打开工程时仍执行既有 Recovery Journal 与 Agent Transaction/Undo 恢复，确保重基线不破坏已有工程。

## 11. 生产命令与能力面

生产 Command Manifest 固定为 28 条：

- 3 条 Health/App UI State；
- 7 条 Agent Protocol V2；
- 5 条 Provider 设置/连接；
- 13 条工程扫描、创建、打开、读取、手动编辑、搜索和关闭。

Command Manifest、`generate_handler!`、自动生成权限、主窗口 Capability、TypeScript Client 和静态边界门必须逐项相等。旧 Provider Chat、Router、Workflow、Story、Timeline 和 Agent V1/写入产品命令全部未注册、未授权、无 TypeScript Agent 调用入口。

WebView 不获得 Shell、Process、Opener、任意 URL、任意命令名或操作系统路径。

## 12. 未挂载安全 Kernel

以下能力保留为 AM-R2 可复用的 Rust 安全 Kernel，但 AM-R1 不挂载到产品 IPC：

- Markdown create/patch 预检、Schema、完整 Diff 与 Hash 绑定；
- 风险分类、三档历史授权策略和一次性确认；
- 多文件原子事务、Recovery Journal、DPAPI Undo；
- 章节 Serializer、连续性、状态迁移和 Story Ledger 规则；
- 路径逃逸、Prompt Injection、敏感 Canary、取消、冲突、故障注入和零部分提交回归。

测试构建通过直接 Harness 调用这些模块。它们的存在不表示产品写入口、Context Mark 或冲突决议已授权。

## 13. GAR1 技术提交状态

NG-R101～NG-R106 的实现和机器可读证据构成 GAR1 技术提交。提交必须保持：

- `agentCanonWriteCount=0`
- `productionCommandCount=28`
- `productionAgentIpcCount=7`
- `realProviderLiveSmokeExecuted=false`
- `gar1Approved=false`
- `amR2Authorized=false`
- `amR3Authorized=false`

GAR1 技术提交不等于产品批准。未获得后续明确授权前，不得实现 AM-R2 Canon 化/Context Mark/冲突/Commit V2，不得实现 AM-R3 长章生成或出版闭环，不得启动 Sprint 6 或真实 Provider Live Smoke。

权威执行资料：

- [AM-R1 Story Packet](./stories/agent-conversation-authoring-am-r1-story-packet.zh-CN.md)
- [Agent Protocol V2 与 Context Mark 规格](./stories/agent-protocol-v2-context-mark-review.zh-CN.md)
- [对话式小说创作重基线 Plan](./stories/agent-conversation-authoring-rebaseline-plan.zh-CN.md)
