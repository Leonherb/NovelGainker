# NR-3 常驻 Novelist Capability Pack / Canon Read Context Story Packet Draft 2

## 0. 文档状态

- Packet：`NR-3 常驻 Novelist Capability Pack / Canon Read Context`
- 版本：`Draft 2`
- 上游计划：`NR-2～NR-7 Capability Delivery Plan Draft 2`
- 上游 Gate：`NR-G3 = passed / approved`
- 当前实现基线：`NR-2`
- 当前授权：**形成 Draft 2 并进行最终评审**
- 实施授权：**未授权**
- `NR-301～NR-306`：**不得实施**
- 真实 Provider Live Smoke：**未授权**
- `NR-4～NR-7` Story Packet：**不得形成**

本文档已吸收并关闭获产品接受的 `NR3-D1-R01～R06` 评审意见，把 NR-3 路线收敛为最终评审用 Story Packet。它不是实施授权，也不改变当前产品：生产 Tool Registry 仍为空，应用仍不加载 Novelist Pack、不读取 Canon 作为模型上下文、不由 AI 写入 Markdown。

---

## 1. BMAD Routing Brief

### Scope

- Packet type：`execution-packet`
- Project type：Windows desktop novel-writing application
- Project level：4
- Runtime context：Codex
- Confidence：高

### Current phase

- Implementation Gate boundary；
- NR-2 与 NR-G3 Remediation 已完成，NR-G3 已批准；
- 当前只获授权形成 NR-3 Draft 2，尚未进入 NR-3 实施。

### Recommended next move

- 评审 `NR3-D2-R01～R06`；
- 对 Draft 2 进行最终评审；
- 只有产品负责人随后明确授权，才可实施 `NR-301～NR-306`。

### What not to do yet

- 不修改生产代码或启用任何生产 Tool；
- 不执行真实 DeepSeek/OpenAI 请求；
- 不实现 Canon 创建、更新、Diff、Undo 或冲突解决；
- 不形成 NR-4～NR-7 Story Packet。

---

## 2. 产品结果与阶段边界

### 2.1 本阶段要得到的体验

用户继续只面对中栏中的同一个 AI，自然聊天方式不变。每轮 AI 都具有稳定的 Novelist 身份和紧凑能力目录；当它根据语义判断需要小说方法或项目事实时，可自主请求 Host：

1. 加载一项 Novelist 能力的受控指令或参考资源；
2. 列出、搜索或分段读取当前工程允许范围内的 Markdown Canon；
3. 在得到结果后继续输出一条正常自然语言回复。

用户不需要选择 Skill，不需要输入关键词，不需要 Anchor、Context Mark、任务卡或 Control JSON。Host 只执行模型明确提出且通过 Schema、能力范围、路径、大小和取消校验的只读请求，不替模型判断“该用哪项能力”或“哪些内容重要”。

### 2.2 本阶段明确不做

NR-3 不包含：

- 创建、更新、删除、移动、重命名或格式化任何 Markdown；
- 把 Conversation 中的决定自动落实到角色、世界、剧情或章节文件；
- Canon Diff、确认、Undo、冲突解决或 Writer Lease 写事务；
- Story Ledger 更新、章节状态变化、EPUB 构建或资产导入；
- 外部图片 Provider、网络、Shell、Process、Opener、Python Sidecar；
- 多 Tool Round、并行 Tool、后台 Tool 或 Provider 自动重试；
- 新窗口、第四栏、Skill 选择器、Task Center 或右侧 History；
- 多 Conversation Session 或 NR-4～NR-6 能力；
- 真实 Provider Live Smoke。

用户要求写入时，AI 可以在对话中讨论、拟稿或说明建议，但必须明确内容尚未保存；不得伪造成功，也不得把“当前阶段无写工具”转换为“本次回复未完成”。

---

## 3. 继承且不可破坏的合同

NR-3 必须完整继承 NR-0～NR-2：

1. 中栏只保存并呈现自然语言 Conversation；机器 Tool 数据不进入可见消息。
2. History 是完整对话事实源；Memory 是可丢弃派生摘要；Markdown 与资产是唯一 Canon。
3. 普通闲聊、小说讨论、查设定和写作准备使用同一输入框和同一 Turn。
4. 用户消息与 `running` 在 Provider 调用前先持久化；非空回复保留，异常终态只追加既有未完成文案。
5. Compactor 0/1、Agent Attempt 1/2、Provider HTTP Attempt 总数最多 3、Tool Execution 最多 1、Tool Round 最多 1、自动重试 0。
6. Tool、Task 与 Conversation 保持独立终态；只读任务失败不得抹除或改写已产生的非空回复。
7. Event 是低延迟路径，Conversation/Task Store 是事实源；重开只对账，不自动重放 Provider 或 Tool。
8. 原始窗口关闭只可被未保存的手动 Markdown 阻断；确认关闭后取消活动 Turn 并立即退出。
9. 生产 IPC 继续保持 24 Commands、3 Conversation Commands 和现有 Event Channel；NR-3 内部 Tool 不新增前端文件 IPC。
10. Provider Key 只存在于 Rust CredentialVault 调用路径，不进入 Pack、Canon Context、Tool Result、Store、日志或 Evidence。
11. 正式运行时不得读取外部插件缓存、用户 `SKILL.md`、外部 Runtime Host 或项目内伪装的能力文件。

---

## 4. 当前基线与改动表面

| 表面             | NR-2 当前状态                                     | NR-3 计划改动                                    |
| ---------------- | ------------------------------------------------- | ------------------------------------------------ |
| Conversation     | 完整 History、Memory、自然语言终态                | 增加常驻 Novelist 公共合同和紧凑能力目录         |
| Agent Loop       | 空生产 Registry；测试可执行一个 Tool Round        | 注册一个封闭、只读的生产 Tool                    |
| Tool Runtime     | Definition/Arguments/Result、Task Store、取消合同 | 增加合格的 `NovelistReadContextExecutorV1`       |
| Provider         | DeepSeek/OpenAI 原生 Tool Request/Result          | 双方使用同一个只读 Tool Schema 与 Context Bundle |
| Workspace/Search | 安全 list/read/search，供现有 Host/UI 使用        | 复用内核，增加更严格的 Agent-only Canon 投影     |
| Pack             | 仓库已有禁用的 AM-era 适配资源                    | 清理为内置、版本固定、只读运行投影               |
| UI               | 中栏简短 Task Activity                            | 不增加 Skill 卡或新面板，沿用受控活动状态        |
| Canon 写入       | AI 路径不存在                                     | 继续严格为 0                                     |

建议新增或收敛的 Rust 模块职责：

- `capability_pack.rs`：内置 Pack 身份、资源、Hash、目录和能力范围；
- `canon_context.rs`：Agent-only Canon Catalog、list/search/read 与 Context Bundle；
- `tool_runtime.rs`：只暴露封闭生产 Factory，不增加动态注册入口；
- `conversation_runtime.rs`：注入常驻合同、使用只读 Registry、预留 Continuation 字节；
- `provider_core.rs`：继续使用现有原生 Tool/Continuation，不增加新协议；
- `workspace.rs` / `search.rs`：复用路径与读取内核，不把前端 IPC 当作 Agent 权限。

模块名称可在实施时机械调整，但职责和权限边界不得合并成“读取任意文件”的通用接口。

---

## 5. NR3-D2-R01：Pack 来源、身份与 Clean Cut

### 5.1 现状问题

已批准总计划要求使用 `novelist 0.2.0` 的九项 Skill。仓库现有 `resources/skills/novelist/v1` 自报 `skillPackVersion = 1.0.0`，包含旧 `agent_migration_skill_runtime_v1`、`enabledAtRuntime = false`、写入 Tool/Scope 和 `AgentActionPayloadV1`。这些字段属于旧 AM-era 方案，不能直接启用。

### 5.2 建议决策

采用双重、不可混淆的版本身份：

- 来源身份：`sourcePackageId = novelist`、`sourcePackageVersion = 0.2.0`；
- 运行投影身份：`projectionId = novelgainker.novelist`、`projectionSchemaVersion = 1`、`projectionContentVersion = 1.0.0`，简称 `novelgainker.novelist/v1`；
- 来源证据：MIT License、上游仓库、Source Commit、Source Archive SHA-256、上游文件清单及逐文件 SHA-256；
- 来源版本、Commit 与 Archive Hash 必须在仓库内的 app-owned Manifest/Provenance 中互相绑定，运行时不得通过外部插件缓存补证；
- 运行资源：只使用提交进仓库并编译进应用的 app-owned snapshot；
- 外部 `0.2.0` 插件缓存只可作为一次性开发导入来源，不是构建、测试或运行依赖。

双身份的含义是：`novelist@0.2.0` 回答“方法最初来自哪里”，`novelgainker.novelist/v1` 回答“应用实际运行哪一份经过安全适配的内容”。两者不得复用同一个含糊的 `version` 字段。

`resources/skills/novelist/v1` 继续作为唯一 app-owned Pack 根，避免维护第二份平行真相；但其旧 Manifest 必须 Clean Cut：

- 删除旧 Feature Gate、`enabledAtRuntime`、`AgentActionPayloadV1` 和 NR-3 不允许的写入声明；
- 建立严格的新 Pack/Capability Manifest Schema，只包含身份、描述、语义使用说明、资源 ID、只读范围、Hash 和大小；
- 九项能力必须精确且唯一：
  - `create-novel-project`
  - `create-setting`
  - `create-character`
  - `create-material`
  - `create-plot`
  - `novel-story-telling`
  - `analytical-review`
  - `create-visual-asset`
  - `publish-novel`
- 现有 Fixture 可继续作为只读格式参考，但不得在 NR-3 触发文件创建；
- 所有指令形成 NR-3 只读投影：保留小说方法和语义判断，移除宿主命令、旧控制载荷和“当前可写入”的虚假承诺；`Clean Cut` 完成后重新生成运行 Inventory、逐文件 Hash 与 Pack 总 Hash，不得沿用当前 AM-era 资源的旧 Hash。

### 5.3 运行加载

Pack 资源通过 Rust 编译期嵌入或等价的静态 app-owned 表进入发布物。运行时不根据环境变量、项目文件、用户目录或插件缓存发现能力。第一次使用前必须验证：

- Pack Schema、ID、版本和九项能力；
- 资源 ID 只能指向 Manifest 中的相对逻辑资源；
- Inventory 中的字节数与 SHA-256；
- 资源总数、单文件大小和总大小；
- License 与 Attribution 存在；
- 禁止资源类型、绝对路径、`..`、symlink/reparse、可执行脚本和未知字段。

候选 Pack 上限：

| 项目                | Draft 2 候选值 |
| ------------------- | -------------: |
| Capability 数量     |         精确 9 |
| Pack 文件数量       |           ≤ 64 |
| 单资源字节          |       ≤ 32 KiB |
| Pack 总字节         |      ≤ 256 KiB |
| Capability 基础指令 |        ≤ 3 KiB |
| 紧凑能力目录        |        ≤ 4 KiB |

超限、缺失或 Hash 不符时 Pack 整体 Fail Closed，但普通 Conversation 必须回退为无 Tool 的自然回复，不能让应用或聊天无法启动。

---

## 6. NR3-D2-R02：常驻 Agent 与单次组合只读 Tool

### 6.1 常驻身份

每次主 Agent 请求的唯一 System 指令增加两部分：

1. 简短、固定的 Novelist 公共合同；
2. 九项能力的紧凑目录：稳定 ID、简短描述、何时适用。

该目录只帮助模型语义选择，不包含关键词规则。Host 不扫描用户文本、不计算关键词命中、不预选 Capability。能力目录、Capability 资源索引和 `novelist.read_context@1` Definition/Registry 必须由同一份已验证 Pack 原子生成；Pack 失效时三者一起不可用，不允许“目录声称可读、Registry 却为空”的半可用状态。

### 6.2 为什么采用一个组合 Tool

NR-2 已冻结每 Turn 最多一次 Tool Execution/一个 Tool Round。如果把“加载能力”“搜索 Canon”“读取文件”拆成多个 Provider Tool，模型加载能力后将没有第二次 Tool 机会读取 Canon。

因此 Draft 2 建议生产 Registry 只注册：

`novelist.read_context@1`

一次请求同时允许：

- 指定一个模型自主选择的 `capabilityId`；
- 请求该 Capability 的零到若干受控参考资源片段；
- 提交零到八个 Canon list/search/read 操作；
- 返回一个严格、来源明确、大小有界的 `NovelistContextBundleV1`。

只要模型不请求 Tool，Host 就直接保存普通自然语言回复。Tool 调用不是每轮必经步骤。

### 6.3 请求草案

```json
{
  "capabilityId": "novel-story-telling",
  "capabilityResources": [{ "resourceId": "storytelling/story-flow", "cursor": 0 }],
  "canonOperations": [
    {
      "kind": "search",
      "query": "东之王",
      "caseSensitive": false,
      "scopes": ["characters", "plot", "chapters"],
      "limit": 12
    },
    {
      "kind": "read",
      "relativePath": "chapters/001.md",
      "startLine": 1,
      "maxBytes": 4096
    }
  ]
}
```

合同要求：

- `capabilityId` 必须是目录中的精确 ID；
- Canon 操作存在时必须提供 Capability，读取范围取“全局只读范围 ∩ Capability 范围 ∩ 请求范围”；
- 请求不得为空；操作按原顺序执行，最多 8 项；
- Host 不补写 Query、不扩展 Scope、不替模型换 Capability；
- 同一 Context Bundle 只属于当前 Turn，不持久化为 History、Memory 或 Canon；
- Tool Result 只进入 Provider Continuation；Conversation 只提交最终自然语言。

### 6.4 Host 两层校验与失败续聊

有效权限始终为：

```text
AI 请求范围 ∩ Capability Manifest 声明范围 ∩ Host 全局安全范围
```

三类约束只提供输入，Host 是唯一执行校验和 I/O 的主体。校验分两层：

1. 通用 Tool Contract 校验 Call ID、Tool 名称、JSON 对象、字段类型、枚举、必填项和未知字段；
2. `NovelistReadContextExecutorV1` 在任何文件 I/O 前校验 list/search/read 的判别字段矩阵、互斥字段、操作数、分页/字节范围、Capability Resource、Scope 交集、路径、预算和 Cancellation。

由于现有通用 Schema 不表达 `oneOf`、`maxItems` 和数值范围，第二层不得被省略或假装由 Schema 已经覆盖。对名称和 Call ID 可配对的 `novelist.read_context@1` 请求，若 Schema、语义、Scope、路径或预算不合法，Host 必须零文件 I/O 地生成受控失败 Tool Result，并进入 Attempt 2；AI 可自然说明未能核对的部分或依据已有上下文继续回复。此时 Task 可以是 `failed`，Conversation 仍可为 `completed`，不得直接显示“本次回复未完成”。

只有 Call ID、Tool 名称或 Provider Continuation 已损坏到无法形成严格配对，或 Attempt 2 自身失败、取消时，才按既有 Provider/Turn 未完成语义终止。NR-3 不增加第二次 Tool Round，模型若需修正读取请求可在用户下一轮重新请求。

### 6.5 封闭 Registry

`NovelistReadContextExecutorV1` 必须：

- 由唯一内部 Production Factory 构造，并捕获当前 `ProjectHandle`；
- 校验执行 Context 中的 Project/Session/Turn 与捕获对象一致；
- 通过与 R02 Remediation 相同的具体实现资格测试；
- 在所有循环、扫描和读取边界检查 Cancellation；
- 两秒取消保证只绑定该具体实现和版本；
- 不开放 Tauri、前端、Provider 或普通运行时代码可调用的动态注册 API。

NR-3 通过后预期 `productionRegistryTools = 1`，且唯一工具为只读 Context Tool；这不代表任何写 Tool 获得预授权。

---

## 7. NR3-D2-R03：Canon Catalog 与只读范围

### 7.1 Canon 发现范围

Agent Canon Catalog 只投影现有安全工程索引中的 Markdown：

- 根目录只允许精确的 `project.md`；
- `characters/**`
- `world/**`
- `materials/**`
- `macguffins/**`
- `plot/**`
- `outlines/**`
- `style/**`
- `chapters/**`

始终排除：

- `AGENTS.md`、其他任意根级 Markdown、`.novelgainker/**`、隐藏目录与 Project Metadata；
- `published/**` 生成产物；
- 二进制资产正文、临时文件、Recovery Journal；
- 项目外路径、绝对路径、设备路径、symlink/junction/reparse；
- Pack、应用配置、Credential、日志与 Evidence。

`assets/**` 在 NR-3 整体不进入 Agent Context：不读取二进制正文，也不增加资产元数据操作；视觉和出版阶段另行设计。

### 7.2 三类内部操作

| 操作     | 行为                                          | 结果                                           |
| -------- | --------------------------------------------- | ---------------------------------------------- |
| `list`   | 按稳定相对路径列出允许类型，可带 Scope 与游标 | 最多 64 条、可分页、无正文                     |
| `search` | 对模型提供的字面 Query 做确定性搜索           | 最多返回 24 个带 path/line/snippet 的命中      |
| `read`   | 读取明确相对路径的 UTF-8 Markdown 行区间      | UTF-8/行边界安全片段、Hash、范围与 next cursor |

实现复用 `workspace.rs` 与 `search.rs` 的路径、字节和 reparse 安全内核，但必须先生成只含 `project.md` 与八个明确小说源目录的 Agent Canon Allowlist，再对该列表执行 list/search/read；不得直接复用会包含 `AGENTS.md` 或任意根级 Markdown 的完整 Workspace Index，也不得把前端 `project_read_file` 变成任意 Agent 文件接口。

### 7.3 快照与未保存编辑

- 每个返回来源包含 `relativePath`、完整文件 `contentHash`、`sizeBytes` 和返回范围；
- 同一操作读取过程中发现文件变化时返回稳定受控错误或跳过该来源，不拼接两个版本；
- 右栏尚未保存的编辑器缓冲区不是 Canon，Agent 只读取已持久化 Markdown；
- 未保存内容不阻断聊天。用户可先保存或把文本直接放入 Conversation；
- NR-3 不创建持久化 Catalog 数据库；Catalog 每次从现有有界工程索引派生，避免引入第二份 Canon。

### 7.4 空工程与异常文件

空目录、缺少角色/世界/章节、单个文件编码不支持或搜索无结果，都返回合法的空/部分 Context Bundle。只要 Provider 能继续，AI 必须正常解释信息不足或继续讨论，不得把“没有 Canon”视为聊天失败。

---

## 8. NR3-D2-R04：Context 来源、优先级与预算

### 8.1 来源分层

整个 Agent Initial Request 与 Continuation 的逻辑来源必须显式区分：

1. `capability_method`：内置能力方法，不是小说事实；
2. `canon_snapshot`：带 path/hash/range 的已保存 Markdown；
3. `conversation_exact`：Memory 之后的近期精确对话；
4. `conversation_memory`：较早对话的派生摘要。

`NovelistContextBundleV1` 只承载前两类：`capability_method` 与 `canon_snapshot`。`conversation_exact`、`conversation_memory` 和当前用户消息仍由既有 Initial Context Assembler 提供；Tool Result 不复制它们，Provider Continuation 也不得由 Host 再次拼接一份，确保当前用户消息和 History/Memory 都不重复。

权威规则：

- Canon Snapshot 高于 Memory 中与之冲突的小说事实；
- 最近精确用户消息可表达新的意图、纠正或未提交决定，但在 NR-3 不自动改变 Canon；
- 当前用户表述与 Canon 冲突时，AI 应自然说明差异或按用户要求讨论新方案，不得声称文件已更新；
- Capability 方法、参考资料、Canon Markdown 和用户文本都不能授予 Tool、文件、网络或系统权限；
- Canon 中出现的“忽略规则”“执行命令”等文本一律作为不可信小说数据引用。

### 8.2 Context Bundle 候选上限

继续继承 NR-2 的 `MAX_TOOL_ARGUMENTS_BYTES = 16 KiB`、`MAX_TOOL_RESULT_BYTES = 24 KiB` 与 Provider Body `96 KiB`。Draft 2 候选分配：

| 内容                             | 候选上限 |
| -------------------------------- | -------: |
| Capability 基础指令              |    3 KiB |
| 可选 Capability 参考片段合计     |    6 KiB |
| Canon 文本/搜索片段合计          |   10 KiB |
| 来源、游标、状态与 JSON 开销预留 |    5 KiB |
| Context Bundle 序列化总量        |   24 KiB |
| 单个返回片段                     |    4 KiB |
| 单 Turn 唯一 Canon 文件          |        8 |
| Canon 操作                       |        8 |

长资源和长章节只按 UTF-8/行边界返回片段，并带 `truncated` 与下一游标。Host 不让模型请求突破 24 KiB，也不通过压缩或省略来源标识偷偷扩大内容。最终大小按 `NovelistContextBundleV1` 序列化后的实际 UTF-8 字节计算；先预留 Envelope 与完整来源元数据，再按 Capability 基础指令、请求顺序中的参考资源、Canon 操作结果顺序填充。预算耗尽时只在 UTF-8/行边界截断当前片段，并为当前及尚未容纳的操作返回受控 `truncated`/`budget_exhausted` 状态与可用的 `nextCursor`，不得重排结果或丢弃来源。

### 8.3 Provider 请求预留

现有 Main Request 可能已接近 96 KiB，Continuation 再增加 Tool Result 会导致完整 Provider Body 越界。NR-3 必须在 Initial 组装时预留 Tool Definition、Continuation、Context Bundle 和 Provider JSON 开销：

- 候选 `NR3_AGENT_LOOP_RESERVE_BYTES = 40 KiB`；
- System + Memory + Exact History + 当前用户消息仍按最近优先截取；
- 当前用户消息只出现一次且不得被截断；
- 初始和 Continuation 的完整序列化 Body 都必须在出站前验证 ≤ 96 KiB；
- 不提高 96 KiB、32 KiB 单消息、8 MiB 响应或 9,216 requested-output 上限；
- NR-306 用短/长 History、最大 Memory、最大用户消息和最大合法 Context Bundle 做组合规模测试，再在 NR-G4 技术证据中冻结最终 Reserve。

若候选 Reserve 不足，实施必须收窄可选 Context，而不是静默提高 Provider 硬限制。

---

## 9. NR3-D2-R05：自然 Conversation、Task 与失败语义

### 9.1 普通聊天优先

以下场景必须可以不调用 Tool、用一次主 Agent Attempt 正常完成：

- 日常闲聊和与小说无关的问题；
- 不需要项目事实的创意讨论；
- 用户只要求头脑风暴或提出候选方案；
- 新工程没有角色、世界或章节；
- 用户选择不先固定任何设定。

系统指令不得要求 AI 每轮先检查全部 Canon，也不得因未选择 Capability 而拒绝回复。

### 9.2 只读任务闭环

当模型请求 `novelist.read_context`：

1. Attempt 1 可产生可见前置文本和一个 Tool Call；
2. Host 创建独立 Task，执行一次只读 Context 请求；
3. 成功、失败或取消均写入 Task 终态；
4. 成功时 Attempt 2 根据 Context Bundle 输出自然语言；
5. 失败时只要 Turn 未取消，Attempt 2 仍可解释信息不足并继续对话；
6. Conversation 最终只保存合并后的自然语言，不保存 Skill ID、Query、文件清单或 Tool JSON。

Task Activity 沿用当前中栏受控自然语言。可在不扩张 UI 的前提下将活动文案收窄为“正在查阅小说资料……”，但不得展示能力名、文件路径、Query、Arguments、Result 或技术错误码。

### 9.3 稳定失败边界

至少定义并映射以下类别：

- Pack 不可用或完整性失败；
- Capability/Resource ID 非法；
- Canon Scope/Path/Query 非法；
- 文件缺失、编码不支持、读取中变化；
- 请求或结果预算超限；
- 取消、Deadline、Provider 协议失败。

Pack 初始化失败时，Capability 目录、资源索引和 Production Tool Definition/Registry 必须原子回退为不可用；固定 Novelist 对话身份仍保留，但 System 不得再承诺本轮可以读取 Canon。普通聊天继续。对名称和 Call ID 可配对的单次 Canon 请求，Schema、语义或读取失败只终止对应 Task，并通过失败 Tool Result 进入 Attempt 2；中栏可显示受控的“未能查阅相关资料”，但助手气泡不得因此标为失败，也不得生成“本次回复未完成”的假象。只有 Provider/Turn 自身未完成时才使用现有 Conversation 未完成语义。

### 9.4 关闭与恢复

- 用户停止：取消 Provider 或只读 Executor，最多两秒确认只绑定已验证 Executor；
- 确认关闭窗口：不等待 Tool 或 Provider，自然退出；
- 重开：遗留 Task 转为 `cancelled + runtime.interrupted`，不重放读取；
- 只读操作不创建 Recovery Journal，不修改 Markdown 时间戳，不产生后台索引写入。

---

## 10. NR3-D2-R06：离线证据、NR-G4 与 Live 决策

### 10.1 自动化证据

`NR-306` 不是运行时产品能力，不在用户每轮聊天时执行；它只在开发测试与 NR-G4 技术提交中证明 `NR-301～NR-305` 的实现符合批准合同。

NR-306 至少覆盖：

- Pack 精确九项能力、版本、License、Inventory、Hash、大小和自包含发布；
- 生产代码与发布物无外部插件缓存、用户 Skill、Python、Shell、Process、URL 加载路径；
- 旧 `AgentActionPayloadV1`、AM Feature Gate、写 Scope 不进入 NR-3 Runtime Manifest；
- DeepSeek/OpenAI 均捕获同一个 `novelist.read_context@1` Definition、Call 与 Result Continuation；
- 请求体捕获证明相同用户文本原样进入两个 Adapter，携带相同完整能力目录，Host 未注入 `selectedCapability`、Router 结果或关键词标签；
- 本地 Loopback 对相同用户文本分别返回两个不同合法 Capability，Host 必须分别执行 Provider 实际返回的选择，不替换或预选；
- 生产代码扫描与定向测试证明 Provider 返回前不存在用户文本关键词表、正则映射或 Host Capability 分支；三项证据共同证明选择权在模型，而不是证明真实模型一定选得好；
- 无关闲聊、空工程和不需要 Canon 的讨论均为零 Tool、正常回复；
- list/search/read 正例，以及绝对路径、穿越、隐藏目录、`AGENTS.md`、其他根级 Markdown、`.novelgainker`、`assets`、reparse、二进制和越界反例；
- 合成 Canon Fixture 同时包含相关内容、无关干扰内容和禁止路径；验证只把 Allowlist 内命中的来源与片段交给 Attempt 2，无命中时返回合法空结果且 Conversation 继续；
- Canon 内容中的 Prompt Injection 文本不能扩张 Tool 或权限；
- 最大合法 Context Bundle、长 History、最大有效 Memory 与 Provider Body 组合不超过上限；
- Pack、Tool 或单文件失败不污染非空 Conversation；
- 取消、窗口关闭、重开与 Task/Conversation 双终态；
- 操作前后全工程 Canon 文件内容、Hash、mtime 和文件集合不变；
- 生产 Registry 精确 1 个只读 Tool，Canon Writes、External Requests、Skill UI 和新增 IPC 均为 0；
- 完整 `pnpm quality`、Locked + Offline Cargo、Debug/Release Build 和本地窗口 Smoke。

所有 Provider 自动化继续只使用 `127.0.0.1` Loopback，不读取真实 Credential。上述证据只能验证 Pack 自包含、Host 不路由、Canon 读取正确、零写入与协议机械兼容；它不证明真实 DeepSeek/OpenAI 的 Capability 选择质量或小说语义质量，这部分只由 10.3 的独立 Live 决策承担。

### 10.2 NR-G4 技术提交

NR-G4 Report 与机器 Manifest 至少声明：

- Pack ID、来源版本、运行投影版本、能力数、资源数和 Pack Hash；
- Production Tool 数量与精确名称；
- Canon list/search/read 次数与写入次数；
- 最大 Canon 文件数、Context Bundle 字节和完整 Provider Body 字节；
- 关键词 Router、用户可见 Skill 选择、Control JSON、新 IPC、外部请求均为 0；
- 双 Provider 离线请求体与 Continuation 证据；
- `realProviderLiveSmokeExecuted = false`；
- `nrG4Approved = false`；
- `nr4EnteredOrPacketFormed = false`。

### 10.3 进入 NR-4 前的独立决策

NR-306 只记录、不得替用户作出以下选择：

1. 单独授权最小只读 Live Smoke：在合成工程和专用凭据下分别验证 DeepSeek/OpenAI 的自然回复、一次 Tool Call/Result 和一次 Canon Read，写入次数为 0；或
2. 明确放弃该检查，并接受真实 Provider Tool/Capability 兼容性尚未验证的风险。

NR-G4 技术提交可以完全离线形成。沉默不视为 Live 授权；本 Draft 2 的形成、评审或未来实施批准均不包含 Live Smoke。

---

## 11. Story 顺序

### NR-301——内置 Novelist Capability Pack

交付：

- 对 `novelist@0.2.0` 形成 app-owned、可复现、版本固定的九能力运行投影；
- Clean Cut 旧 AM Manifest 与 `AgentActionPayloadV1`；
- 建立严格 Pack Schema、Inventory、Hash、License、Attribution 和静态加载；
- Pack 错误 Fail Closed，但不阻断普通 Conversation。

验收：

1. 九项能力 ID 精确唯一，未知/重复/缺失均失败；
2. 每个资源由逻辑 ID、字节数和 SHA-256 绑定；
3. Runtime 不访问仓库外、插件缓存、用户目录或网络；
4. 指令不声称 NR-3 可以写入；
5. `novelist@0.2.0` 来源版本、Commit、Archive Hash 与运行投影身份在仓库证据内闭合；
6. Clean Cut 后的 Inventory、逐文件 Hash 和 Pack 总 Hash 重新生成；
7. 发布扫描和篡改反例通过。

### NR-302——常驻 Novelist Agent 与内部语义调用

交付：

- 固定公共 Agent 合同与 ≤4 KiB 能力目录；
- 一个封闭的 `novelist.read_context@1` Production Tool；
- 模型选择 Capability/Resource/Canon 操作，Host 只做机械校验和执行；
- 保持一个 Tool Round、零 Router、零 Skill UI。

验收：

1. 每次主回复都有同一公共 Agent 身份；
2. 无 Tool 场景仍只有一次 Agent Attempt；
3. Provider 选择能力，不存在 Host 关键词分支；
4. Production Registry 只有一个已验证只读 Executor；
5. 通用合同与 Executor 两层校验在任何文件 I/O 前完成；
6. 可配对的非法请求转为失败 Tool Result，并由 Attempt 2 继续 Conversation；
7. Capability 目录、资源索引和 Registry 从同一验证结果原子生成；
8. 不新增通用注册入口、IPC 或前端选择器。

### NR-303——Canon Catalog 与只读 Tool

交付：

- Agent-only list/search/read DTO；
- ProjectHandle 绑定、Scope 交集、稳定排序、分页、片段和来源 Hash；
- 复用路径/reparse/字节安全内核；
- 所有 Canon 修改路径保持不可达。

验收：

1. 只有 `project.md` 与八个明确小说源目录进入 Agent Canon Allowlist，`AGENTS.md`、其他根级 Markdown 与 `assets/**` 不进入；
2. 正常 Markdown list/search/read 可在一个组合请求内执行；
3. 空工程、无命中和部分不可读文件返回合法有界结果；
4. 越权路径与二进制读取在 I/O 前拒绝；
5. 长文件可按游标继续，UTF-8 与行边界正确；
6. 操作前后 Canon 文件集合、内容、Hash 和 mtime 不变。

### NR-304——Canon Context 预算、来源与优先级

交付：

- `NovelistContextBundleV1`；
- Capability/Canon/Conversation/Memory 来源标签和冲突规则；
- 24 KiB Result 与候选 40 KiB Agent Loop Reserve；
- Continuation 完整 Body 出站前验证。

验收：

1. Canon 来源带相对路径、Hash、范围和截断游标；
2. Context Bundle 只含 Capability/Canon，不复制 History、Memory 或当前用户消息；
3. Canon 高于冲突 Memory，但用户新意图不被伪装成已写 Canon；
4. Canon 文本不能注入权限或系统指令；
5. 结果按最终序列化字节确定性装配和截断；
6. 最大 Initial/Continuation Body 均 ≤96 KiB；
7. 当前用户消息只出现一次，最近精确 History 优先。

### NR-305——自然对话只读闭环

交付：

- 闲聊、小说讨论、空工程、Canon 查询和写入请求共用一条 Conversation；
- Tool 成功/失败/取消与回复独立；
- 中栏只显示受控活动文案；
- 关闭、停止、重开保持 NR-2 行为。

验收：

1. 无关闲聊不调用 Tool；
2. 缺少角色/设定/章节不阻止回复；
3. AI 可按语义主动读 Canon；
4. 写入要求可被讨论或拟稿，但文件不变且不会假装保存；
5. 可配对 Tool 失败进入 Attempt 2，不把已完成自然回复改为失败占位；
6. Pack 失效时目录、资源与 Registry 原子降级，普通聊天继续；
7. History/Memory 不保存 Tool Payload 或 Capability 指令。

### NR-306——规模回归、Live 决策记录与 NR-G4 技术提交

交付：

- 双 Provider、Pack、Canon Read、Context 预算、零写入与 UI/IPC 回归；
- NR-G4 Manifest 与技术报告；
- 单独记录进入 NR-4 前的最小只读 Live 决策点；
- 停止于 NR-G4 技术提交。

验收：

1. `pnpm quality` 完整通过；
2. 所有 Provider Evidence 为本地 Loopback；
3. 请求捕获、Provider 选择行为和生产扫描三类证据共同证明 Host 无关键词 Router；
4. 合成相关/干扰/禁止来源 Fixture 证明 Canon 检索范围和相关性；
5. Production Registry 精确一个只读 Tool；
6. Canon 写入、外部请求、新 IPC、Skill UI 均为 0；
7. Manifest 与 Report 机器声明一致；
8. 证据不声称已经验证真实模型语义质量；
9. `nrG4Approved = false`，未形成 NR-4～NR-7 Packet。

---

## 12. 实施顺序与停止条件

若本 Packet 后续经过评审并获得明确连续实施授权，唯一允许顺序为：

1. `NR-301`：Pack Clean Cut、来源与完整性；
2. `NR-302`：常驻 Agent、能力目录与封闭组合 Tool；
3. `NR-303`：Canon Catalog/list/search/read；
4. `NR-304`：Context Bundle、来源优先级与请求预留；
5. `NR-305`：自然 Conversation 只读闭环；
6. `NR-306`：离线规模回归与 NR-G4 技术提交；
7. 停止。

任一 Story 失败时不得跳过、弱化测试、提高硬上限或提前形成 Gate。连续实施授权如果未来给出，也只覆盖 NR-301～NR-306，不包含真实 Provider Live Smoke、NR-G4 产品批准或 NR-4 Story Packet。

---

## 13. Draft 2 设计决策清单

### `NR3-D2-R01`——Pack 来源与 Clean Cut

建议接受：

- 来源包固定为 `novelist@0.2.0`，运行投影固定为 `novelgainker.novelist/v1`，两套版本字段与 Hash 证据明确分离；
- 仓库内 snapshot 是唯一运行来源；
- 清理旧 AM Feature Gate、写入声明和 `AgentActionPayloadV1`，不做兼容读取；
- 九项能力以只读投影进入 NR-3。

### `NR3-D2-R02`——单组合 Tool

建议接受：

- 保持 NR-2 的一个 Tool Round；
- Production Registry 只增加 `novelist.read_context@1`；
- Capability 加载与 Canon list/search/read 合并为一次模型自主请求，并由 Host 通用合同与 Executor 两层校验；
- Host 不使用关键词 Router。

### `NR3-D2-R03`——Canon 范围

建议接受：

- Agent Canon Allowlist 只包含根 `project.md` 与八个明确小说源目录；
- list/search/read 使用 Agent-only 更严格 DTO；
- `AGENTS.md`、其他根级 Markdown、`assets/**` 与未保存编辑器缓冲都不进入 NR-3 Canon Context；
- 不持久化第二份 Catalog，不读取 `.novelgainker/**` 或二进制正文。

### `NR3-D2-R04`——Context 预算与权威关系

建议接受：

- Result 保持 24 KiB，Provider Body 保持 96 KiB；Context Bundle 只承载 Capability 与 Canon，不重复 History/Memory；
- 候选 Agent Loop Reserve 为 40 KiB，并由 NR-306 组合规模测试冻结；
- Canon Snapshot 高于冲突 Memory；
- 当前用户新意图不自动成为 Canon；
- 所有 Canon 内容按不可信数据处理。

### `NR3-D2-R05`——Conversation 失败语义

建议接受：

- Tool 和 Capability 永远可选，普通聊天优先；可配对的 Tool 校验失败通过 Attempt 2 继续回复；
- 空工程、无结果、Pack/读取失败不阻断自然回复；Pack 失效时目录、资源索引和 Registry 原子降级；
- 写入要求只讨论或拟稿，不假装保存；
- Task 与 Conversation 独立，UI 不增加 Skill/技术展示。

### `NR3-D2-R06`——Evidence 与 Live 决策

建议接受：

- NR-3 实施和 NR-G4 技术提交默认完全离线；
- NR-306 以请求捕获、Provider 选择、生产扫描、相关性 Fixture 和前后快照证明精确一个只读生产 Tool、Host 无 Router、零 Canon 写入、零外部请求；
- 最小只读 Live Smoke 在进入 NR-4 前单独二选一，当前不授权；
- NR-G4 技术提交后停止，不形成 NR-4～NR-7 Packet。

---

## 14. 当前停止点

本 Draft 2 形成后，下一步仅为：

> 评审 `NR3-D2-R01～R06`

当前不得实施 `NR-301～NR-306`，不得运行真实 Provider Live Smoke，不得形成 NR-4～NR-7 Story Packet。
