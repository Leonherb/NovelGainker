# NovelGainker 当前产品完整报告

> 报告日期：2026-09-03  
> 仓库版本：`0.1.0`  
> 当前实现基线：`NR-2`  
> 当前 Gate：`NR-G3 = passed / approved`  
> 当前工作状态：NR3-D2-R01～R06 与 FR01～FR02 已在 NR-3 Story Packet Draft 2 Final 中获批准，停止于 Packet 批准 Gate  
> 明确未执行：NR-301～NR-306、真实 Provider Live Smoke、NR-4～NR-7 Story Packet

## 1. 执行摘要

NovelGainker 是一款 Windows-first、Local-first 的简体中文小说创作桌面应用。产品的最终方向是：作者只需像使用 Codex 一样与 AI 自然交流，AI 和 Novelist Skill 根据语义判断讨论、创作或任务意图，Host 负责可靠执行文件、工具、持久化、恢复与安全操作，小说 Markdown 与资产始终是唯一 Canon。

截至本报告，产品已经完成三层基础能力：

1. `NR-0`：清理旧 Anchor、Context Mark、Control JSON 和线性 Agent 流程，建立可靠的自然对话、工程编辑、Provider 与关闭生命周期；
2. `NR-1`：建立随工程保存的完整 History、派生 Memory、阈值压缩、有界长对话 Context，以及双 Provider 的单次可取消调用路径；
3. `NR-2`：建立 Provider-native Tool Request/Result、有界 Host Agent Loop、独立 Task Store、取消/恢复边界和中栏内联 Task 状态。

当前产品已经是一个可运行、可构建、可进行自然对话和手动 Markdown 编辑的桌面应用，并具备安全接入工具的 Host 执行底座；它还不是最终形态的 Novelist Agent。生产 Tool Registry 仍为空，AI 当前不会读取角色、世界观、设定或章节 Canon，不会加载 Novelist Skill，不会实际执行领域 Tool，也不会创建或更新小说 Markdown。

一句话判断：**当前版本是“可靠的本地小说工程 + 长对话 + 安全 Agent Loop 底座”，不是“会自主维护小说工程的完整 AI 小说 Agent”。**

## 2. 产品目标与最终交互原则

### 2.1 最终产品目标

作者通过一个持续、自然、非线性的 Conversation 完成构思、讨论、决策和创作。AI 应当能够：

- 正常回答小说相关或无关内容，不因缺少设定、任务卡或章节而拒绝继续对话；
- 记住工程内的完整聊天历史，并在上下文过长时自动压缩；
- 由 AI 与 Novelist Skill 进行语义判断，而不是依赖关键词、Anchor 或固定线性流程；
- 在发现新角色、新设定或已有信息变化时，提出或执行相应的工程操作；
- 在作者明确要求写作或确认正文后，把结果落实到正式章节；
- 在 Canon 冲突时向作者说明冲突，并按产品规则请求或执行解决方案；
- 由 Host 执行经过约束的 Tool Request、文件事务、校验、恢复和安全控制。

### 2.2 已冻结的交互原则

- 中栏始终是自然语言 Conversation，不显示 Control JSON、Anchor 卡片或任务卡流水线。
- 普通闲聊、小说讨论、创作要求和任务发布使用同一条对话路径。
- AI/Skill 负责语义判断；Host 不使用关键词猜测用户意图或 Canon 价值。
- 用户可见回复与可选任务执行拥有独立终态；未来工具失败不能抹除正常的自然语言回复。
- Host 是实际执行者：负责 Provider、Context、Tool、文件、事务、History/Memory、取消、恢复和安全。
- 小说 Markdown 与资产是唯一 Canon；Conversation Memory 不是角色卡、设定或章节真值。

## 3. 当前产品状态

| 项目                     | 当前结论                         |
| ------------------------ | -------------------------------- |
| 产品阶段                 | NR-2 已实现                      |
| 最新 Gate                | NR-G3 已批准并通过               |
| Remediation              | NR-G3-R01～R03 已完成            |
| 当前动作                 | 停止于 NR-3 Packet 批准 Gate     |
| 真实 Provider Live Smoke | 未执行、未授权                   |
| NR-3                     | Draft 2 Final 已批准；未授权实施 |
| NR-4～NR-7               | Story Packet 未形成、未授权      |
| Novelist Skill           | 未加载                           |
| Tool Loop                | 基础设施已实现，生产工具数为 0   |
| AI Canon 读写            | 未实现                           |
| 自动重试                 | Conversation Turn 内为 0         |

当前 Gate 状态：

```yaml
status: passed
reviewStatus: approved
nrG3Approved: true
realProviderLiveSmokeExecuted: false
nr3ToNr7PacketAuthorized: false
nextAction: await_product_direction
```

## 4. 当前用户体验

### 4.1 启动与工程选择

应用启动时显示工程选择弹窗。用户必须先完成以下任一动作：

- 新建一个最小小说工程；
- 打开已有 Markdown 小说工程；
- 从最近工程记录中重新打开工程。

在工程打开前不会启动 AI 对话。新建工程不强制预先填写角色、世界观或章节；标题有本地默认值，作者、类型和故事前提均可选。

### 4.2 三栏工作区

桌面宽屏保持三栏：

| 区域 | 当前职责                                                        |
| ---- | --------------------------------------------------------------- |
| 左栏 | 工程导航、文件入口、DeepSeek/OpenAI Provider 设置、本地运行状态 |
| 中栏 | 工程绑定的完整自然对话、发送、流式回复、停止和终态显示          |
| 右栏 | 真实 Markdown 文件查看、编辑、保存、结构诊断和搜索结果定位      |

左右侧栏可折叠；右栏支持拖动和键盘调整宽度。窄屏模式使用抽屉式侧栏，但中栏仍是主要交互区域。

### 4.3 Provider 配置

当前固定支持两个 Provider：

| Provider | 固定协议         | 固定模型          | 固定端点                                    |
| -------- | ---------------- | ----------------- | ------------------------------------------- |
| DeepSeek | Chat Completions | `deepseek-v4-pro` | `https://api.deepseek.com/chat/completions` |
| OpenAI   | Responses        | `gpt-5.6-terra`   | `https://api.openai.com/v1/responses`       |

用户可以保存、替换、删除 Key，选择当前 Provider，并单独执行连接测试。保存 Key 不会联网；连接测试是显式动作，使用固定最小请求，不读取项目正文。

API Key 只在输入框和本次保存调用中短暂存在，随后进入 Windows Credential Manager。应用只能查询配置状态，不能从 UI 或 IPC 读回、显示、导出或记录完整 Key。

### 4.4 自然对话

用户可以发送普通闲聊、小说讨论、构思或直接创作请求。对话不要求角色卡、世界观、章节、Memory 或任务卡存在。

每轮发送遵循：

```text
保存用户消息与 running
        │
        ├─ Context 低于阈值 ──> 一次流式主回复
        │
        └─ Context 达到阈值
               ├─ 最多一次内部 Compactor
               └─ 最多一次流式主回复
```

Compactor 不显示气泡。它普通失败时保留旧 Memory，并继续主回复；用户在 Compactor 阶段停止时取消整个 Turn，不再启动主回复。

### 4.5 手动 Markdown 编辑

当前编辑器支持：

- 列出工程允许范围内的 Markdown 文件；
- 读取 UTF-8、UTF-8 BOM、LF/CRLF 文件；
- 手动编辑并使用按钮或 `Ctrl+S` 保存；
- 保存时保留 BOM 和标准换行风格，混合换行规范化为 LF；
- 使用源 Hash 防止覆盖外部并发修改；
- 保存失败时保留磁盘旧文件和内存草稿；
- 保存后运行结构诊断；格式问题不阻断作者手动保存；
- 对工程源文件进行有界的中英文文本搜索。

当前没有通用的前端“新建源文件”“删除源文件”或 AI 自动写入入口。已有 Canon Apply/Recovery 安全 Kernel 尚未挂载为当前产品能力。

### 4.6 停止、失败与关闭

- 非空 AI 文本始终保留；若终态异常，则附加 `本次内容未生成完成。`。
- 零文本失败或取消显示 `本次回复未生成。`。
- 应用重开后，遗留的 `running` 会转为未生成，不自动重试。
- Event 只负责低延迟显示；Conversation Store 是终态真值，活动 Turn 每 1000ms 进行一次非重叠对账。
- 只有未保存的手动 Markdown 可以阻断关窗，并显示 `保存 / 放弃 / 取消`。
- 确认关闭后取消活动 AI Turn 并立即退出，不等待 Provider、Conversation flush 或工程清理。

## 5. 工程与数据结构

### 5.1 新工程脚手架

新建工程包含：

```text
project.md
AGENTS.md
characters/
materials/
macguffins/
plot/
outlines/
world/
style/
assets/
chapters/
published/
.novelgainker/
  project.json
```

`published/` 是生成物目录，不作为可编辑小说源。`.novelgainker/**` 是应用状态目录，不进入小说索引或出版输入。

### 5.2 数据权威层级

| 数据             | 位置                                      | 性质                | 是否 Canon |
| ---------------- | ----------------------------------------- | ------------------- | ---------- |
| 项目说明与小说源 | `project.md`、源目录 Markdown             | 作者内容            | 是         |
| 图片与其他资产   | `assets/`                                 | 作者内容            | 是         |
| 可见对话         | `.novelgainker/conversation/session.json` | Conversation 事实源 | 否         |
| 派生 Memory      | `.novelgainker/memory/state.json`         | 可丢弃缓存          | 否         |
| 项目元数据       | `.novelgainker/project.json`              | 严格 Schema v2      | 否         |
| Provider Key     | Windows Credential Manager                | 系统凭据            | 否         |
| 最近工程/UI 偏好 | 本机 App State                            | 可恢复本地状态      | 否         |

### 5.3 Project Metadata

当前 Project Metadata 为严格的 `schemaVersion = 2`。旧 v1 只表示可丢弃的旧应用状态：在 writer lease、规范化根路径和递归 reparse 检查通过后，一次性重置 `.novelgainker/**`，但保留小说 Markdown、素材、资产、Provider 设置、UI 偏好、无关项目和系统凭据。

## 6. History、Memory 与长对话 Context

### 6.1 History

完整可见 History 随工程保存在 `.novelgainker/conversation/session.json`。压缩不能删除、折叠、改写或替换历史消息。

每个 Turn 包含稳定 ID、Provider、模型、用户消息、助手真实产生的文本和终态。用户消息与 `running` 在任何 Provider attempt 前先持久化。

### 6.2 Memory

Memory 是帮助继续自然对话的派生摘要，不是角色、世界观、设定、章节或 Canon。它绑定：

- Project ID 与 Conversation Session ID；
- `coveredFromCursor`～`coveredThroughCursor` 的连续 History 区间；
- UTF-8 安全 Segment Cursor；
- 覆盖源字节的 SHA-256；
- 更新时间和严格五栏目自然语言摘要。

摘要栏目固定为：

1. 已确认决定；
2. 正在创作或讨论；
3. 未决问题；
4. 用户明确任务；
5. 已产生但未完成的内容。

Host 校验栏目顺序、唯一性、非空正文、围栏、顶层结构和字节上限；AI 负责栏目内的语义取舍。AI 建议不能被提升为用户决定。

### 6.3 Context 组装顺序

主回复 Context 固定为：

1. 本轮唯一 System 指令；
2. 当前有效 Memory；
3. Memory 之后、预算允许的最近精确 History；
4. 只出现一次的当前用户消息。

Memory 与最近精确 History 冲突时，以最近精确 History 为准。Memory 无效时不弹窗、不迁移、不阻断聊天，直接回退到最近精确 History。

### 6.4 冻结 Profile

| 参数                     |       冻结值 |
| ------------------------ | -----------: |
| 压缩触发阈值             |       64 KiB |
| 受保护最近精确尾部       |       24 KiB |
| Segment 内容上限         |        6 KiB |
| Compactor 输入上限       |       72 KiB |
| Backfill 输入预留        |        8 KiB |
| Memory 摘要上限          |       12 KiB |
| Compactor 请求输出上限   | 1,024 tokens |
| 每次 Agent 请求输出上限  | 4,096 tokens |
| Agent Attempts / Turn    |          ≤ 2 |
| Tool Executions / Rounds |      ≤ 1 / 1 |
| 单 Turn 累计请求输出上限 | 9,216 tokens |

Provider 硬边界为 64 条消息、96 KiB 请求、32 KiB 单消息和 8 MiB 响应。无 Tool 时主回复最多一次；存在合法 Tool Call 时主 Agent Attempt 最多两次；若同 Turn 触发 Compactor，Provider HTTP Attempt 总数最多三次。Conversation Turn 内自动重试为零。

## 7. 当前技术架构

### 7.1 技术栈

| 层        | 技术                             |
| --------- | -------------------------------- |
| 桌面容器  | Tauri 2                          |
| 前端      | React 19、TypeScript 5.8、Vite 7 |
| Host/Core | Rust 2021                        |
| 前端测试  | Vitest、Testing Library、jsdom   |
| Rust 质量 | rustfmt、Clippy、Cargo Test      |
| 包管理    | pnpm 11、Cargo Workspace         |
| 主要平台  | Windows 11 / MSVC                |

### 7.2 运行结构

```text
React UI
  ├─ 启动工程选择
  ├─ Project / Search / Markdown editor
  ├─ Provider settings / local health
  └─ Conversation view
          │ 24 条固定 Tauri Commands 中的有界 IPC
          │ 3 条 Conversation Commands + 1 个 Event
          ▼
Rust Tauri Host
  ├─ Project/session registry + single writer
  ├─ Workspace read/save/watch/search/validation
  ├─ Conversation Store + Memory Store
  ├─ Context Assembler + Compactor
  ├─ Agent Attempt 1 → 0..1 Tool → 0..1 Agent Attempt 2
  ├─ 空生产 Tool Registry + 独立 Task Store
  ├─ DeepSeek/OpenAI Provider-native adapters + CredentialVault
  └─ atomic persistence / recovery / path safety kernels
```

### 7.3 固定 IPC 表面

生产面固定为 24 条 Command：

- Runtime/App State：`health_check`、`app_state_load`、`app_state_save_ui`；
- Conversation：`conversation_session_load`、`conversation_turn_start`、`conversation_turn_cancel`；
- Provider：`provider_status_load`、`provider_key_save`、`provider_key_delete`、`provider_select`、`provider_connection_test`；
- Project/Workspace：`project_scan`、`project_create_preview`、`project_create`、`project_open`、`project_open_recent`、`project_list_files`、`project_read_file`、`app_state_set_current_file`、`project_watch_file`、`project_save_file`、`project_validate_draft`、`project_search`、`project_close`。

流式 Conversation Event 为 `conversation-turn-event`。Manifest、Handler、Permission、Capability 和所有生产 TypeScript Client 必须保持严格一致。

## 8. 安全、持久化与恢复

### 8.1 文件安全

- 只接受经过约束的工程相对路径；拒绝绝对路径、穿越、隐藏目录、Windows 设备名和尾随点/空格。
- 不跟随指向工程外的 symlink、junction 或其他 reparse point。
- 可编辑单文件最大 4 MiB；工程索引最多 5,000 个条目、16 层深度。
- 保存使用源 Hash 进行乐观并发控制，外部修改后停止覆盖。
- 保存通过临时文件、flush、原子替换和恢复日志保护；失败保留旧文件。
- 手动 Markdown 的格式诊断不阻断作者保存，但会影响未来 AI Apply、Final 或 EPUB 资格。

### 8.2 单写者与恢复

同一工程只能有一个 Writer。启动和写入前检查遗留事务；可确定的未完成操作自动完成或回滚，无法安全判定时阻止新写入，避免猜测或覆盖正文。

### 8.3 Provider 与凭据

- 固定 HTTPS Host、端点、协议、模型、超时和本地大小边界；用户不能修改任意 URL。
- Key 不进入项目、Conversation、Memory、日志、Evidence 或前端状态。
- 自动化使用 `127.0.0.1` Loopback，清除 Provider/secret 环境变量并启用 Cargo Offline。
- Conversation 调用使用可中断的 one-attempt 方法；带重试接口仅保留给非 Conversation 的连接测试边界。

## 9. 质量与验证状态

NR-G3 已通过完整本地质量门。最近一次评审验证包括：

- Prettier、ESLint、TypeScript 全部通过；
- 18 个 React Test Files、104 项测试通过；
- Workspace 边界：47 个发布文件、24 条生产 Command、3 条 Conversation Command 闭包通过；
- NR-001～NR-006、NR-106 与 NR-206 Evidence 通过；
- Tool Runtime、Task Store、Provider Core 与 Conversation Runtime 定向测试通过；
- Rust Workspace：109 项 app-core 测试通过、4 项按设计 ignored，其他 workspace/integration tests 通过；
- 10,000 条可见消息规模测试通过；
- 54,000 字节长助手消息被分为 9 个安全 Segment；
- Debug/Release Tauri 构建与 Windows 窗口 Smoke 通过；
- 离线编辑 E2E 和未挂载 Canon/Recovery Safety Kernel 通过；
- 自动化外部请求为 0，真实 Provider Live Smoke 为 false。

冻结 NR-G3 Manifest 仍记录技术提交生成时的 `nrG3Approved = false`；提交后的产品批准记录在 NR-G3 Report Gate Decision 和 BMAD 当前状态中。这是历史证据冻结，不是状态冲突。

## 10. 原始需求对齐矩阵

| 需求                                 | 当前状态                   | 说明                                                                |
| ------------------------------------ | -------------------------- | ------------------------------------------------------------------- |
| 启动时新建或打开工程                 | 已实现                     | 启动弹窗、新建、打开、最近工程均存在                                |
| 中栏自然对话并回复已有内容           | 已实现                     | 完整工程 History、流式主回复、失败恢复均存在                        |
| 新建工程不强制角色/世界观/设定       | 已实现                     | 最小工程可直接创建，扩展元数据可选                                  |
| 用户可先设定角色和世界观             | 部分实现                   | 有对应目录和手动编辑能力，但当前 UI 无通用新建源文件入口            |
| 对话产生角色/设定后自动落到 Markdown | 未实现                     | 需要未来 Skill 判断、Tool Loop 与 Canon Upsert                      |
| AI 每轮读取角色/世界观/设定          | 未实现                     | 当前 Context 不读取 Canon                                           |
| 小说产出使用 Novelist Skill          | 未实现                     | 当前发布面不加载 Skill                                              |
| Context Mark / Anchor                | 已被产品决策移除           | 最终方向改为 AI/Skill 语义判断，不恢复机械锚定                      |
| Canon 冲突时提示用户解决             | 未实现                     | 冲突检测和交互尚未设计、授权或挂载                                  |
| 支持几十万字长期写作                 | 底座已实现，业务闭环未完成 | 10,000 消息和有界 Context 已验证；Canon 读取、Skill 与章节生产尚缺  |
| AI 可执行对话中发布的任务            | 底座已实现，领域执行未实现 | Host Agent Loop 已闭合；生产 Registry 为空，尚无可执行小说领域 Tool |
| 会话记忆与聊天历史随工程保存         | 已实现                     | History 是事实源，Memory 为可丢弃派生状态                           |

## 11. 已完成交付

### 11.1 NR-0：Clean Slate 与可靠 Conversation

- 清理旧 AM/GAR、Anchor、Context Mark、Control JSON、强制 Skill Selection 和旧 Agent 数据路径；
- 建立 schema v2 工程状态和新 Conversation Store；
- 保留 Canon、资产、Provider、编辑与通用安全 Kernel；
- 实现双 Provider 普通自然聊天、可见终态、停止、关闭和恢复；
- 固定三栏 UI、24/3 IPC 与离线质量证据；
- 通过 NR-G1。

### 11.2 NR-1：History、Memory 与长对话

- `NR-101`：History/Memory 合同、派生 Store、Cursor、Hash 和原子写入；
- `NR-102`：字节阈值、近期优先重建、连续覆盖和渐进 Backfill；
- `NR-103`：有界 Compactor、单 attempt、可取消、零重试；
- `NR-104`：Context Assembler、主回复回退和当前消息唯一性；
- `NR-105`：重开、Provider 切换、停止、关闭和无新增 History/Memory UI；
- `NR-106`：候选 Profile、10,000 消息规模测试、预算冻结和 NR-G2 Evidence。

### 11.3 NR-G2 Remediation

- `R01`：双 Provider 动态 System 指令端到端保真及非法合同零出站；
- `R02`：有效 Memory 增量不可执行时禁止退回近期重建；
- `R03`：五栏目摘要严格顺序、唯一性、正文和反例校验。

### 11.4 NR-2：Host Agent Loop 与 Task Runtime

- `NR-201～NR-206`：严格 Tool 合同、双 Provider 原生 Tool Request/Result、有界单 Round Agent Loop、独立 Task Store、中栏内联状态、取消/恢复/预算和离线规模证据；
- Production Registry 保持为空，NR-2 只交付安全执行底座，不交付小说领域 Tool。

### 11.5 NR-G3 Remediation

- `R01`：Tool Result 严格配对、Task 状态矩阵、错误码白名单和非法恢复拒绝；
- `R02`：封闭测试 Registry、具体 Executor 取消资格和不合格注册反例；
- `R03`：同核心编排的 Compactor + Tool 三调用组合证据及新 Memory 实际消费。

## 12. 当前明确未实现的产品能力

- Novelist Skill Registry、加载、版本和语义选择；
- 生产 Novelist Tool 注册与小说领域自主任务执行；
- 将 Canon 读取并组装进模型 Context；
- 角色、世界观、设定、素材、剧情与章节的语义 Upsert；
- AI 新建/更新 Markdown、展示 Diff、确认、Undo 和冲突解决；
- AI 将确认正文提交到正式章节；
- 多 Conversation Session、History 分支/清空/导出和云同步；
- Provider cursor、断点续传和 Conversation 自动重试；
- 真实模型对 Memory 摘要、长篇连续性和写作品质的语义验证；
- 已存在但未挂载 Kernel 的完整 Canon Apply、连续性审查和出版闭环。

## 13. 风险与产品健康判断

### 13.1 当前没有阻断 Gate 的技术问题

NR-G3 已批准通过，三项 Remediation 已关闭，完整质量门通过。当前没有已知问题要求在 NR-2 内再次返工。

### 13.2 主要产品风险

| 风险                                  | 当前判断                                                  |
| ------------------------------------- | --------------------------------------------------------- |
| 用户把 NR-2 误认为完整 Novelist Agent | 高，需要持续明确“Agent Loop 底座”和“已注册领域能力”的区别 |
| 真实 Provider 摘要语义质量            | 未验证；机械结构与失败回退已验证                          |
| 自动维护 Canon                        | 尚不存在，不能通过 Prompt 假装已经实现                    |
| 长篇小说一致性                        | Context 机械容量已验证，Canon/Skill 语义连续性尚未验证    |
| Provider Preset 可用性                | 本地合同已验证，真实网络与账号可用性未验证                |

### 13.3 当前产品适合做什么

- 在本地工程中手动维护 Markdown 小说资料；
- 与 DeepSeek 或 OpenAI 进行工程绑定的自然对话；
- 保留长期聊天历史并在压力下自动维护派生 Memory；
- 可靠停止、关闭、恢复和切换 Provider；
- 作为未来 Novelist Skill、领域 Tool 和 Canon Upsert 的安全底座。

### 13.4 当前产品不应被承诺为什么

- 自动小说工程管理员；
- 已完整接入 Novelist Skill 的写作 Agent；
- 能自动识别并写入角色、世界观或章节的产品；
- 已通过真实模型长篇语义质量验证的生产版本。

## 14. 下一阶段决策入口

NR3-D2-R01～R06 与 FR01～FR02 已在 NR-3 常驻 Novelist Capability Pack / Canon Read Context Story Packet Draft 2 Final 中获产品批准。当前停止于 Packet 批准 Gate，没有自动下一动作；尚未授权实施 `NR-301～NR-306`，不执行真实 Provider Live Smoke，也不形成 NR-4～NR-7 Story Packet。

## 15. 权威证据与阅读入口

- 当前 BMAD 状态：`bmad/workflow-status.yaml`、`bmad/sprint-status.yaml`；
- 当前交付状态：`docs/sprint-plan.zh-CN.md`；
- 当前实现架构：`docs/architecture.zh-CN.md`；
- NR-0 Packet：`docs/stories/novelist-agent-runtime-nr0-clean-slate-story-packet.zh-CN.md`；
- NR-1 Packet：`docs/stories/novelist-agent-runtime-nr1-history-memory-story-packet-draft-2-final.zh-CN.md`；
- NR-G2 Remediation：`docs/stories/novelist-agent-runtime-nr-g2-remediation-story-packet-draft-2.zh-CN.md`；
- NR-G2 技术报告：`docs/reviews/novelist-agent-runtime-nr-g2-technical-submission.zh-CN.md`；
- NR-2 Packet：`docs/stories/novelist-agent-runtime-nr2-host-agent-loop-story-packet-draft-2.zh-CN.md`；
- NR-G3 Remediation：`docs/stories/novelist-agent-runtime-nrg3-remediation-story-packet-draft-2.zh-CN.md`；
- NR-G3 技术报告与批准记录：`docs/reviews/novelist-agent-runtime-nr-g3-technical-submission.zh-CN.md`；
- NR-3 Story Packet Draft 2 Final：`docs/stories/novelist-agent-runtime-nr3-capability-canon-read-story-packet-draft-2-final.zh-CN.md`；
- 冻结机器证据：`target/qa/nr-206/nr-g3-manifest.json`；
- 最终本地质量入口：`pnpm quality`。

## 16. BMAD Routing Brief

### Scope

- Packet type：`brownfield-resume-packet`；
- Project type：Windows desktop novel-writing application；
- Project level：4；
- Runtime context：Codex；
- Confidence：高。

### Current phase

- Implementation Gate boundary；
- NR-2 已实现，NR-G3 与 NR-3 Story Packet Draft 2 Final 已批准；当前停止于 NR-3 Packet 批准 Gate。

### Recommended next move

- 等待产品负责人另行授权后续动作；
- Packet 批准不自动进入实施。

### What not to do yet

- 不把规划中的 Skill、小说领域 Tool 或 Canon 能力写成已实现；
- 不执行真实 Provider Live Smoke；
- 不实施 NR-301～NR-306；
- 不形成 NR-4～NR-7 Story Packet。
