# NovelGainker Sprint 2 Story Packet

> 状态：S2-Q001～Q004 与 Story Packet Review Gate 已通过；NG-201～NG-207 已获产品验收；G2 Live Waiver 已记录；G2 已由产品负责人批准，Sprint 2 已关闭
> 日期：2026-08-22  
> BMAD 数据包：execution-packet  
> Sprint：2——DeepSeek 与 OpenAI Provider  
> 范围：NG-201～NG-207  
> 团队：1 人，严格顺序实施  
> 上游计划：[sprint-plan.zh-CN.md](../sprint-plan.zh-CN.md)（已批准）  
> 上游架构：[architecture.zh-CN.md](../architecture.zh-CN.md)（已批准）  
> 前置 Gate：[sprint-1-story-packet.zh-CN.md](./sprint-1-story-packet.zh-CN.md)（G1 已通过）

## 1. Sprint 目标

以 Key-only 方式安全接入 DeepSeek 与 OpenAI 两家固定文本 Provider，并在中栏交付不修改 Canon 的流式对话。

Sprint 2 的核心不是“让前端请求一个兼容接口”，而是建立一个不会泄漏密钥、不会静默改变接收方、不会把模型输出写入手稿的 Provider 基础层：

- 用户只输入 API Key，Base URL、协议、模型、Timeout 和生成参数由 App 的只读 Provider Preset 固定；
- 原始 Key 只通过一次带类型的 Tauri Command 进入 Rust，保存在批准的加密凭据库，保存后不可从前端读回；
- SQLite 只保存 Provider 选择、Preset 版本、连接状态和不透明密钥引用，不保存真实 Key；
- DeepSeek 与 OpenAI Adapter 实现同一个最小标准化契约，但各自显式转换协议差异；
- Provider HTTP 只从 Rust 发起，WebView 不获得通用 HTTP、URL、Header 或网络能力；
- 中栏只发送用户明确输入的对话文本，不自动读取项目文件，不创建提案，不修改 Canon；
- 当前 Provider 和固定模型在发送前、流式生成中和结果上始终可见；
- Provider 失败只进入失败或可重试状态，绝不自动把内容发送给另一 Provider；
- 无 Key、无网络、Vault 不可用或 Provider 故障时，Sprint 1 的浏览、搜索、编辑、校验和保存能力继续可用。

## 2. Sprint 2 可演示结果

G2 Review 应能按以下顺序复现：

1. 启动 Windows 11 x64 内部测试构建，确认不配置 Key 时两个 Provider 都显示“未配置”，AI 发送按钮禁用；
2. 展开左栏 API 设置卡，只看到 DeepSeek、OpenAI、API Key、保存/替换、删除、连接测试和当前选择；
3. 确认 UI 不显示 Base URL、协议、Model ID、Header、Timeout、Temperature 或任意生成参数；
4. 为 DeepSeek 输入合成测试 Key，保存后输入框立即清空，界面只显示“已保存”状态，无法读回原始值；
5. 重启应用，确认 DeepSeek 仍显示已配置，但 SQLite、日志、项目文件和前端状态中不存在完整 Key；
6. 执行连接测试，确认只发送固定的最小合成文本，不发送项目文件或手稿，并显示成功、认证失败、网络不可达或模型不可用的中文状态；
7. 对 OpenAI 重复保存与连接测试，确认两个 Key 独立存储；
8. 显式选择 DeepSeek，在中栏输入普通对话并发送，看到 Provider、固定模型、运行状态、流式文本、停止按钮和最终 Usage；
9. 停止一次生成，确认流停止、状态归类为 cancelled，已收到文本不写入项目文件；
10. 在 Run 结束后显式切换到 OpenAI，再发送一条新消息，确认接收方变化清晰可见；
11. 让当前 Provider 返回认证失败、Rate Limit、Timeout 和畸形流，确认错误脱敏、有界重试且从不请求另一 Provider；
12. 删除其中一个 Key，确认对应 Provider 立即变为未配置，另一个 Provider 和离线编辑不受影响；
13. 删除 App SQLite，确认项目仍可打开；Provider 选择可重建，Key 是否仍保留按凭据库一致性策略安全呈现；
14. 运行统一质量入口、Provider Mock Contract Test、密钥泄漏回归和可选 Live Smoke，满足 G2 Gate。

## 3. 范围边界

### 3.1 本 Sprint 包含

- Sprint 1 App State Schema 的显式 v1 → v2 Migration；
- DeepSeek/OpenAI Provider 本机选择、Preset 版本、连接状态和不透明密钥引用；
- Rust CredentialVault 抽象及 Review Gate 批准的 Windows 凭据后端；
- DeepSeek 与 OpenAI 两个独立 Key 的保存、替换、删除和状态查询；
- 左栏 Key-only 设置、连接状态、连接测试和显式 Provider 选择；
- 固定、只读、带版本和构建时校验的 ProviderPreset v1；
- Provider Core 标准化请求、响应、事件、Usage、错误和取消契约；
- DeepSeek Chat Completions Adapter；
- OpenAI Responses Adapter；
- Rust HTTP Client、TLS 校验、固定主机和明确 Timeout；
- SSE/流式事件解析、顺序号、取消和终态；
- Mock Provider Contract Test；
- 中栏普通流式对话，不读取项目 Context、不写 Canon；
- 显式 Provider 切换与禁止自动跨 Provider Fallback；
- Key、Authorization Header、Prompt、响应正文和错误泄漏回归；
- 两家 Provider 的用户主动 Live Smoke 入口与脱敏结果；
- Sprint 1 离线编辑、安全保存和无 Key 场景的完整回归。

### 3.2 明确不做

- 不支持 Anthropic、Gemini、第三家 Provider、自定义 OpenAI-compatible Endpoint 或本地模型；
- 不允许用户编辑 Base URL、协议、Model ID、Header、Timeout、Reasoning、Temperature 或其他参数；
- 不使用 ChatGPT Plus/Pro 登录、Cookie、消费者订阅或 OAuth 代替 OpenAI Platform API Key；
- 不从 React/WebView 直接发 Provider HTTP 请求；
- 不给 WebView 暴露通用 Stronghold、Windows Credential Manager、HTTP、Shell、Process、Opener 或任意 URL 权限；
- 不把真实 Key、Key 片段、Authorization Header 或响应正文写入 SQLite、项目、日志、错误、Telemetry、截图或测试快照；
- 不自动跨 Provider Fallback，不在重试时更换 Provider；
- 不自动读取项目 Markdown、搜索结果、当前编辑器文本或其他文件作为对话上下文；
- 不实现 Context Manifest、Prompt Compiler、Workflow Pack、Manager、Tool Calling、Proposal、Diff 或 AI Apply；
- 不让模型直接编辑或保存 Markdown；用户只能手动复制需要的文本；
- 不实现图片、Vision、语音、Realtime、Batch、Web Search、File Search 或其他 Hosted Tool；
- 不把 Provider 原生会话、隐藏状态或 Response ID 当作 Canon；
- 不实现云同步、账户、代理服务、订阅计费、Git 管理、Android 或安装器；
- 不削弱 Sprint 1 的项目路径沙箱、原子保存、单 Writer、恢复或离线测试。

## 4. 关键契约

### 4.1 Provider 标识与固定 Preset

第一版只允许两个 Provider ID：

| Provider ID | 显示名 | 固定协议 | 固定 Base URL | 固定模型 |
| --- | --- | --- | --- | --- |
| deepseek | DeepSeek | DeepSeek Chat Completions | https://api.deepseek.com | deepseek-v4-pro |
| openai | OpenAI | OpenAI Responses | https://api.openai.com/v1 | gpt-5.6-terra |

ProviderPreset v1 随应用构建，只读且经过结构校验和内容 Hash 校验。前端可以展示 Provider、模型和 Preset 版本，但不能提交或覆盖以下字段：

- Base URL；
- Endpoint Path；
- Protocol；
- Model ID；
- Authorization Header 名称或格式；
- Timeout；
- Streaming 开关；
- Reasoning、Temperature、Top P、最大输出等生成参数；
- TLS 或证书策略；
- Retry 策略。

生产代码只能连接 Preset 白名单中的 HTTPS 主机。测试代码只允许在明确的测试构建或 cfg(test) 下连接由 Fixture 创建的 Loopback Mock Server。

Preset 更新只能随新的 App 版本和显式 Migration 发生。模型不可用时显示升级指引，不允许用户输入任意 Model ID，也不允许自动选择另一模型或 Provider。

### 4.2 Key 与 CredentialVault

CredentialVault 是 Rust 内部接口，至少提供以下语义：

~~~text
CredentialVault
  put(provider_id, secret) -> credential_ref
  replace(provider_id, secret) -> credential_ref
  delete(provider_id) -> deleted | absent
  status(provider_id) -> configured | absent | unavailable
  with_secret(provider_id, operation) -> operation_result
~~~

边界要求：

- Review Gate 必须先确定 S2-Q001 的 Windows 后端；
- Provider ID 只能是 deepseek 或 openai；
- 原始 Key 只在 put、replace 或一次 Provider 调用的最小作用域内解密；
- Rust 使用秘密类型并尽量零化临时 Buffer，不把 Key Clone 到普通状态对象；
- Key 保存后不提供 get、reveal、export、copy 或返回原始值的 App Command；
- 前端只得到 configured、connectionStatus 和固定文案“已保存”，不显示真实后四位；
- Vault 不可用时 Provider 能力失败关闭，但项目和手动编辑继续工作；
- 删除 Key 是显式用户操作，并有确认；
- 删除或替换在 Vault 与 SQLite 跨存储失败时必须可安全重试和对账；
- App 启动时把“SQLite 有引用但 Vault 无 Secret”归类为未配置，把“Vault 有 Secret 但 SQLite 无引用”归类为需对账，不读取或显示 Secret；
- Key 绝不进入项目目录、环境变量、命令行、浏览器存储、剪贴板、Crash Report 或诊断附件。

### 4.3 App State Schema v2

Sprint 1 的 Schema v1 三张表保持职责不变。NG-201 通过显式 Migration 增加唯一允许的 Provider 状态表，建议逻辑结构如下：

~~~text
provider_settings
  provider_id        TEXT PRIMARY KEY, only deepseek | openai
  preset_version     INTEGER NOT NULL
  credential_ref     TEXT NULL
  is_selected        INTEGER NOT NULL
  connection_status  TEXT NOT NULL
  checked_at         INTEGER NULL
~~~

约束：

- 真实 Key、Key Hash、Key Prefix、Authorization Header、Prompt、Response、Usage 明细和对话正文不进入该表；
- credential_ref 只能是 Vault 生成或 Vault Adapter 固定的本机不透明引用；
- 最多一个 Provider 被选中；没有可用选择时使用非敏感默认提示，但发送仍禁用；
- Migration 在事务内执行，失败时保留可读的 Schema v1，不阻塞手动打开项目；
- 新版本数据库由旧应用打开时继续安全降级，不尝试破坏性回滚；
- 删除 SQLite 后 Canon 项目仍完整；Vault 中遗留 Secret 按对账策略处理，不自动发送请求；
- App State 删除、损坏和迁移失败不能影响手稿保存事务。

### 4.4 窄 Tauri Command 与事件

Sprint 2 建议只新增以下七条窄 Command；Review Gate 修改名称时，必须同步 Manifest、Handler、Capability、前端类型客户端和契约测试：

| Command | 输入 | 输出 | 允许的副作用 |
| --- | --- | --- | --- |
| provider_status_load | 无 | 两家脱敏状态与当前选择 | 只读 SQLite/Vault 状态 |
| provider_key_save | ProviderId、一次性 Key | 脱敏 ProviderStatus | 写 Vault、更新引用 |
| provider_key_delete | ProviderId、确认标记 | 脱敏 ProviderStatus | 删除 Vault Secret、清引用 |
| provider_connection_test | ProviderId | 脱敏 ConnectionResult | 对固定 Provider 发最小测试请求 |
| provider_select | ProviderId | 脱敏 ProviderStatus | 更新本机选择 |
| provider_chat_start | 用户明确输入的 Messages | RunAccepted | 对当前固定 Provider 发请求 |
| provider_chat_cancel | RunId | CancelAccepted | 取消本进程内活动 Run |

Command 约束：

- provider_key_save 是唯一允许 Key 进入 IPC 的命令；Key 不得出现在 Debug、Display、AppError、Trace 或测试失败输出；
- provider_connection_test 和 provider_chat_start 不接受 URL、模型、Header、密钥、绝对路径或文件选择；
- provider_chat_start 在 Rust 侧捕获当前 Provider、Preset 版本和模型，Run 启动后不可更换；
- provider_chat_cancel 只能取消当前进程登记的活动 Run；
- 未登记 Provider、RunId、超长消息、空消息和畸形 DTO 在联网前拒绝；
- Provider Command 与项目 Command 分离，Provider 故障不能持有或破坏 Project Writer Lock；
- 不引入通用 HTTP Plugin Capability 或 Stronghold Guest Capability。

Rust 使用一个固定事件名 provider-run-event 向前端发送严格解析的事件：

~~~text
ProviderRunEvent
  runId
  sequence
  kind: started | text_delta | usage | completed | failed | cancelled
  providerId
  model
  textDelta?
  usage?
  error?
~~~

事件约束：

- sequence 对每个 Run 单调递增；
- 每个 Run 只有一个终态；
- 终态后到达的 Delta 被丢弃并记录无正文指标；
- error 只含稳定 code、Provider ID、可重试性和中文恢复建议；
- 不包含原始响应 Body、Header、Key、服务器堆栈或不受限 Response ID；
- textDelta 只进入当前对话视图，不进入日志或 AppError。

### 4.5 标准化 Provider Core

两个 Adapter 共同实现最小接口：

~~~text
ProviderAdapter
  validate_key(credential_ref, preset) -> ValidationResult
  capabilities(preset) -> CapabilitySet
  generate(request) -> NormalizedResponse
  stream(request) -> Stream<NormalizedEvent>
  cancel(run_id)
  estimate_usage(request) -> optional UsageEstimate
~~~

Sprint 2 的 NormalizedRequest 只允许：

- 系统固定的普通对话说明；
- 用户在中栏明确输入的标准化 Messages；
- 固定输出上限和 Reasoning Preference；
- 当前 ProviderPreset；
- RunId 和取消信号。

Sprint 2 不注册 Tool、不带项目文件、不带 Context Pack、不带输出 Schema、不允许模型提出文件操作。

NormalizedEvent 和 NormalizedResponse 只暴露：

- 文本 Delta 或最终文本；
- Finish Reason；
- Input/Output/Reasoning Token Usage（Provider 提供时）；
- Provider 与固定模型；
- 稳定脱敏错误；
- 可取消、已取消或已完成状态。

Adapter 必须明确处理协议差异：

- OpenAI Responses SSE 事件映射到统一事件；
- DeepSeek Chat Completions SSE Delta 与 data: [DONE] 映射到统一事件；
- Usage 缺失时显示“Provider 未返回”，不伪造精确数字；
- Provider 不支持的字段被明确丢弃或降级，不伪装等价；
- 未知事件类型按向前兼容策略忽略或安全失败，但不得当成文本直接渲染；
- 响应大小、单事件大小、总文本长度和事件数量有明确上限。

### 4.6 对话、隐私与 Canon 边界

- Sprint 2 对话是普通 AI 对话，不是 NovelGainker Workflow；
- 只有用户按“发送”后，中栏输入才离开设备；
- 发送前 UI 必须显示接收方 Provider 和固定模型；
- 不自动附加当前项目名、文件路径、Markdown、搜索查询、诊断或未保存编辑器文本；
- 用户主动粘贴到对话框的文本视为用户明确发送；
- Provider 输出只能显示、停止、重新发送或由用户手动复制；
- 不提供“应用到文件”“替换选区”“生成章节”“创建人物”等入口；
- 对话正文是否跨重启保存由 S2-Q003 决定；Review Gate 前不得创建 conversation 表；
- 无论选择何种会话策略，项目 Markdown 都是唯一 Canon；
- 清空对话不删除项目文件，删除 App State 不修改项目文件。

### 4.7 错误、Retry 与 Fallback

稳定错误至少区分：

- provider.not_configured；
- provider.vault_unavailable；
- provider.authentication_failed；
- provider.permission_denied；
- provider.model_unavailable；
- provider.rate_limited；
- provider.timeout；
- provider.network_unavailable；
- provider.protocol_error；
- provider.response_too_large；
- provider.cancelled；
- provider.run_conflict；
- provider.internal_unavailable。

错误消息必须是简体中文，不显示原始响应 Body、Authorization Header、Key、服务器堆栈或机器路径。

仅瞬时网络错误、HTTP 429 和明确可重试的 5xx 可以在同一 Provider 内有界重试。次数、退避上限和 Retry-After 上限由实施者记录并测试。以下情况不自动重试：

- 认证失败；
- 权限拒绝；
- 模型不存在；
- 畸形请求或畸形响应；
- 用户取消；
- 消息过大；
- 含糊终态；
- 任何 Canon 写入。

任何错误都不得自动请求另一个 Provider。用户必须等待 Run 结束，显式切换 Provider，并重新按发送。

## 5. 依赖与执行顺序

~~~mermaid
flowchart LR
    G1[G1 已通过] --> NG201[NG-201 App State v2 与 Vault]
    NG201 --> NG202[NG-202 Key-only 左栏]
    NG201 --> NG203[NG-203 Provider Core 与 Preset]
    NG203 --> NG204[NG-204 Mock Contract Test]
    NG202 --> NG205[NG-205 流式对话]
    NG204 --> NG205
    NG205 --> NG206[NG-206 显式切换与禁止 Fallback]
    NG201 --> NG207[NG-207 密钥泄漏回归]
    NG202 --> NG207
    NG203 --> NG207
    NG204 --> NG207
    NG205 --> NG207
    NG206 --> NG207
    NG207 --> G2[G2 Provider Ready]
~~~

单人团队按以下顺序执行：

1. NG-201；
2. NG-202；
3. NG-203；
4. NG-204；
5. NG-205；
6. NG-206；
7. NG-207；
8. G2 综合验收。

NG-202 与 NG-203 在依赖图中都只依赖 NG-201，但团队为 1 人，因此仍按上述顺序实施，不并行修改 App State、IPC 和 Provider 边界。每个 Story 单独授权、实施、验证和验收。

## 6. NG-201——扩展本机状态库并实现加密 Vault

### Story

作为使用自有 API Key 的作者，我希望 Key 被安全保存在本机凭据库中，以便重启后继续使用 AI，同时任何项目文件或普通状态库都无法读到原始 Key。

### 用户价值

用户只需输入一次 Key，后续能安全使用对应 Provider；即使分享小说仓库、查看 SQLite 或遇到普通错误，也不会暴露凭据。

### 实施范围

- 引入 Rust CredentialVault Trait，隔离产品契约与具体 Windows 后端；
- 按 S2-Q001 实现批准的 Windows 凭据后端；
- 为 DeepSeek/OpenAI 使用独立、固定、不可由前端控制的 Service/Account 标识；
- 建立 Secret 的保存、替换、删除、存在性和临时借用语义；
- 使用秘密类型、最小作用域和尽力零化；
- 建立 SQLite Schema v1 → v2 显式 Migration；
- 新增 provider_settings，只保存非敏感 Provider 状态和 credential_ref；
- 建立 Vault/SQLite 启动对账和失败恢复；
- 为 Vault 不可用、引用缺失、孤立 Secret、迁移失败返回稳定脱敏错误；
- 登记 provider_status_load、provider_key_save 和 provider_key_delete 三条窄 Command；
- 更新命令 Manifest、Handler、Capability、前端类型客户端和共享契约测试；
- 保证删除 App SQLite、删除 Key 和替换 Key 都不触碰项目 Canon。

### 安全约束

- 不实现 Key 读取、Reveal、Export 或复制接口；
- 不给 WebView 暴露通用 Vault/Stronghold/系统凭据权限；
- Provider ID、Service、Account 和 credential_ref 不能由自由文本拼接；
- 不把 Key 保存到 SQLite、Local Storage、Session Storage、环境变量、命令行或项目；
- 不记录 Key 长度、前缀、后缀、Hash 或可关联指纹；
- Key DTO、Secret 类型和包含 Secret 的错误类型不得派生 Debug/Serialize/Clone；
- 测试只使用带唯一 Canary 的合成 Key，不使用真实开发者凭据；
- Vault 失败关闭 AI 能力，但不能阻止离线编辑或手稿保存。

### 验收条件

- [x] Review Gate 已确认 S2-Q001 的 Windows Vault 后端。
- [x] Schema v1 可事务迁移到 v2，Sprint 1 三张表和数据保持完整。
- [x] provider_settings 只包含允许的非敏感字段。
- [x] DeepSeek/OpenAI Key 可独立保存、替换和删除。
- [x] 保存后前端只得到“已保存”状态，不能读回任何 Key 字符。
- [x] 重启后能判断配置状态，但普通内存状态不持有长期明文 Key。
- [x] SQLite、项目、日志、AppError 和前端快照不包含合成 Key Canary。
- [x] Vault 与 SQLite 任一阶段故障都能安全重试或对账。
- [x] 删除 App SQLite 不修改项目；删除 Key 不修改项目或另一 Provider。
- [x] Vault 不可用时离线项目创建、打开、编辑、保存和搜索回归通过。
- [x] 三条新增 Command 的 Manifest、Handler、Capability 和双端契约一致。
- [x] 没有通用 Stronghold Guest、HTTP、Shell、Process 或 Opener 权限。

### 测试与证据

- Schema v1 → v2 Migration、回滚、重复执行和新版本拒绝测试；
- provider_settings Schema/敏感列负面检查；
- In-memory CredentialVault 契约测试；
- 批准 Windows Vault 后端的隔离集成测试；
- 保存、替换、删除和重启状态测试；
- Vault 成功/SQLite 失败、Vault 失败/SQLite 未写等故障注入；
- SQLite 删除和孤立引用对账测试；
- Tauri Command/Capability 契约快照；
- 合成 Canary 全工作区扫描；
- Sprint 1 离线 E2E 回归。

### 完成定义

NG-201 只有在原始 Key 无任何读取回前端的路径、SQLite 永远不含 Secret，并且 Vault 故障不会影响 Canon 或离线编辑时才算完成。

### 实施记录（2026-08-22）

状态：产品负责人已于后续 Story 授权消息中批准验收；NG-201 已关闭，NG-202 随后获得实施授权。

已交付：

- App State Schema v1 → v2 事务迁移、回滚、幂等重启与 Vault/SQLite 对账；
- Windows Credential Manager 后端和固定 DeepSeek/OpenAI 凭据目标；
- provider_status_load、provider_key_save、provider_key_delete 三条窄 Command 及严格前端客户端；
- 稳定中文错误、动态 Canary 泄漏测试和十八条 Capability/边界检查。

验证证据：pnpm quality 全部十三阶段通过；14 个前端测试文件、69 个测试通过；Rust 工作区 61 个测试通过、2 个子进程 Fixture 测试按设计忽略；Sprint 1 离线 E2E、Debug/Release 构建及两种真实窗口 Smoke 全部通过。验证不需要真实 API Key，不访问 Provider 网络。

## 7. NG-202——实现左栏 Key-only 设置

### Story

作为作者，我希望只输入 API Key 就能配置 DeepSeek 或 OpenAI，以便不用理解 Base URL、模型和协议参数。

### 用户价值

用户获得低门槛、接收方清晰且不会误配到未知服务器的 Provider 设置体验。

### 实施范围

- 将 Sprint 0/1 的禁用 API 卡替换为真实 Key-only 设置；
- 左侧栏和 API 设置卡继续分别可折叠；
- 只显示 DeepSeek 与 OpenAI 两个固定 Provider；
- 每个 Provider 显示配置状态、连接状态、固定模型、保存/替换、删除和连接测试；
- Key 输入使用密码控件，不提供显示明文按钮；
- Key 只保存在组件局部短生命周期状态，不进入全局 Store、URL 或浏览器存储；
- 提交结束后无论成功失败都按确认策略清理输入 Buffer；
- 保存、测试和删除使用不同按钮，不把联网测试伪装成本地保存；
- 选择当前 Provider 是显式操作，UI 始终展示当前选择；
- 未配置、测试中、可用、认证失败、网络不可达、模型不可用、Vault 不可用使用稳定中文状态；
- 未配置 Key 或连接状态不允许时，发送按钮禁用并解释原因；
- 窄窗口下左栏继续作为抽屉，不挤压中栏和右栏；
- 按 S2-Q002 固化保存与连接测试的交互语义。

### 安全约束

- 不显示 Key 后四位或任何派生片段；
- 不支持浏览器自动填充写入非预期字段；
- 不把 Key 放进 React DevTools 可长期访问的全局对象；
- 不在 toast、错误边界、analytics、DOM 属性或 aria-label 中复制 Key；
- 不允许粘贴 Key 触发自动保存或自动联网；
- 连接测试必须由用户明确点击，并提示会调用所选 Provider；
- Provider 设置不修改 project.json 或 Markdown；
- 切换 Provider 不自动发送消息。

### 验收条件

- [x] 左栏只展示 DeepSeek/OpenAI 和 Key-only 控件。
- [x] Base URL、Model ID、协议、Timeout 和生成参数没有可编辑入口。
- [x] Key 保存后输入框清空且无法读回。
- [x] 两家 Provider 配置和删除互不影响。
- [x] 保存、连接测试、选择和删除是四个清晰动作。
- [x] 连接测试前明确显示 Provider、固定模型和可能产生最小 API 用量。
- [x] 未配置、Vault 不可用和无网络时发送按钮禁用且离线编辑可用。
- [x] 当前 Provider 在左栏和中栏发送区一致显示。
- [x] 左栏整体折叠与 API 卡折叠偏好继续恢复。
- [x] 窄窗口、键盘导航、焦点、屏幕阅读器和错误状态可用。
- [x] 前端日志、DOM 快照、Local Storage 和错误对象中没有合成 Key。
- [x] Sprint 1 三栏、右栏调宽和编辑器回归通过。

### 测试与证据

- Key 输入生命周期和提交后清理组件测试；
- 两家 Provider 独立状态和删除确认测试；
- 不展示高级配置的负面 DOM 断言；
- 未配置/离线/Vault 故障状态矩阵；
- 键盘、焦点和窄窗口组件测试；
- DOM、错误和浏览器存储 Canary 扫描；
- 宽屏、窄屏真实 Tauri 视觉证据。

### 完成定义

NG-202 只有在用户界面真正做到“只填 Key”，并且保存后的任何前端状态都无法恢复原始 Key 时才算完成。

### 实施记录（2026-08-22）

状态：技术实现与自动化验证完成，并已与 NG-203～NG-207 一并获得 Sprint 2 统一产品验收。

已交付：

- 左栏真实 Key-only 设置，只包含 DeepSeek/OpenAI、固定模型说明、密码输入，以及保存/替换、确认删除、连接测试和显式选择；
- Key 只存在于组件局部 Buffer，提交前立即清空，成功或失败结束时再次清空；无明文显示、读回、掩码、尾号、浏览器存储或全局 Store；
- 新增 provider_select 窄 Command、Capability 与类型化前端客户端；选择写入 App State，可在未配置 Key 时显式切换并在重启后恢复；
- 中栏发送区与左栏使用同一个 Provider 状态快照展示当前 Provider 和固定模型；
- 稳定中文状态、删除确认与安全焦点、窄窗口和原有折叠偏好回归；
- 十九条 Command/Capability 边界、Key Buffer 提交前清理、无 Provider 网络、无 Key 读回与无浏览器存储检查。

边界说明：NG-202 的“连接测试”按钮会先展示 Provider、固定模型和可能产生的极少 API 用量，但明确显示“待 NG-203 启用”，本 Story 不发送网络请求，也未登记伪造的连接测试 Command。真正的固定合成请求、认证/网络/模型状态更新属于 NG-203 Provider Core。

验证证据：Provider 组件、IPC 契约与应用集成测试通过；Rust Provider 选择、App State 持久化和 Tauri Command/Capability 测试通过；pnpm quality 十三阶段通过，包括 Sprint 1 离线 E2E、Debug/Release 构建及两种真实窗口 Smoke。验证不使用真实 API Key，不访问 Provider 网络。

视觉证据限制：自动 Smoke 在隐藏窗口中运行，能够证明进程和主窗口成立，但本次 PrintWindow 截图只捕获到窗口外壳与空白 WebView，因此不计为 NG-202 产品视觉验收证据。产品负责人验收时仍需在可见桌面窗口中检查宽屏/窄屏布局、折叠、焦点和连接测试说明。

## 8. NG-203——实现 Provider Core 与固定 Preset v1

### Story

作为产品维护者，我希望 DeepSeek 与 OpenAI 通过同一个受控契约工作，以便切换 Provider 不改变项目格式，也不把协议差异泄漏给 UI。

### 用户价值

用户可以在两个固定 Provider 之间选择，同时获得一致的发送、流式显示、取消、Usage 和错误体验。

### 实施范围

- 新增 provider-core Rust Crate 或等价清晰模块；
- 新增 DeepSeek Adapter 和 OpenAI Adapter；
- 建立 ProviderId、ProviderPreset、NormalizedRequest、NormalizedEvent、NormalizedResponse、Usage 和 ProviderError；
- 固化 ProviderPreset v1 的两个 Base URL、协议、模型和 Timeout；
- 构建时校验 Preset Schema、唯一 Provider ID、HTTPS、固定主机和内容 Hash；
- Rust HTTP Client 只允许白名单 Host，并使用正常 TLS 证书校验；
- OpenAI 使用 Responses API；
- DeepSeek 使用 Chat Completions API；
- 实现非流式、流式和取消的底层契约；
- 标准化 Finish Reason、Usage、Rate Limit、Timeout、认证和模型错误；
- 同一 Provider 瞬时错误允许有界重试；
- 响应和事件设置大小、数量和耗时上限；
- 生产代码拒绝 Loopback、HTTP 明文、重定向到非白名单 Host 和用户提供 URL；
- Provider Core 不依赖 React、项目文件系统或 Workflow Pack。

### 安全约束

- Authorization Header 只在 Rust 请求构造的最小作用域生成；
- Header、Key、请求 Body、响应 Body 和 SSE 原文不进入日志；
- HTTP Redirect 必须禁用或逐跳重新验证固定 Host，不能携带凭据到其他主机；
- 系统代理行为要记录并测试，但用户不能输入代理 URL；
- TLS 证书校验不可关闭；
- 不允许 Adapter 调用项目写入、Shell、Process 或任意工具；
- 不使用 Provider SDK 的持久会话作为状态源；
- 不自动发现模型或切换模型；
- 测试 Mock Host 只能在测试构建使用。

### 验收条件

- [x] 两个 Adapter 实现同一个最小 ProviderAdapter 契约。
- [x] Preset v1 与批准架构中的 URL、协议和模型一致。
- [x] 用户输入无法覆盖 Preset 任一字段。
- [x] OpenAI Responses 流和 DeepSeek Chat Completions 流可标准化。
- [x] Usage 缺失、Finish Reason 和取消行为有明确映射。
- [x] 认证、Rate Limit、Timeout、模型不可用和协议错误被稳定分类。
- [x] 只允许固定 HTTPS Host；重定向不能泄漏 Authorization。
- [x] 同 Provider Retry 有明确上限，禁止跨 Provider。
- [x] Provider Core 不读取项目、不写 Canon、不加载 Novelist Plugin。
- [x] 原始请求/响应和 Key 不进入日志、错误或测试快照。
- [x] Crate/模块边界和依赖方向符合现有 Rust Workspace。
- [x] Cargo 锁文件准备后，普通质量测试继续锁定且离线。

### 测试与证据

- Preset Golden/Hash/Schema 测试；
- 任意 URL、HTTP、Loopback、重定向和未知 Provider 负面测试；
- OpenAI/DeepSeek Adapter 映射单元测试；
- SSE 分片、UTF-8 边界、未知事件和终态测试；
- Usage/Finish Reason/Error Golden Test；
- 响应大小、事件数量、Timeout 和取消测试；
- Authorization/Body 日志泄漏负面扫描；
- 依赖方向和生产 Host 白名单静态检查。

### 完成定义

NG-203 只有在两家 Provider 的差异被封装在 Adapter 内、生产网络目标完全由只读 Preset 决定，且任何错误都不会把凭据或原始响应带出 Rust 边界时才算完成。

## 9. NG-204——实现 Mock Provider Contract Test

### Story

作为产品负责人，我希望不使用真实 Key 或外部网络也能验证两家 Provider 契约，以便日常开发和 CI 可重复、低成本且不泄密。

### 用户价值

用户获得经过故障、取消和协议差异验证的 Provider 基础，而不是只在一次真实请求中“看起来能用”。

### 实施范围

- 建立隔离 Loopback Mock Server 和确定性合成 Fixture；
- 对 DeepSeek/OpenAI 运行同一公共契约套件；
- 覆盖认证成功和认证失败；
- 覆盖非流式与多种 SSE 分片；
- 覆盖取消前、首 Delta 后和完成竞态；
- 覆盖 Usage 有/无、Finish Reason 和空响应；
- 覆盖 429、Retry-After、Timeout、连接中断和可重试 5xx；
- 覆盖畸形 JSON、未知事件、重复终态、终态后 Delta 和超大响应；
- 覆盖重定向到非白名单目标且 Authorization 不跟随；
- 断言当前 Adapter 失败时另一 Adapter 请求数为 0；
- 普通测试不读取环境变量中的真实 Key；
- 建立可选、显式、独立的 Live Smoke 入口，但不加入默认 Mock 套件；
- 把 Mock Contract Test 加入统一质量入口。

### 安全约束

- Fixture Key 使用明显不可用的唯一 Canary；
- Mock 只监听 Loopback 和随机端口；
- 测试结束只清理验证过的临时资源；
- 失败日志不打印请求 Header、Body 或完整响应；
- 不以跳过 TLS/Host 检查的生产代码路径让 Mock 通过；
- Live Smoke 必须显式启用、从 Vault 取 Key，不能从命令行参数传 Key；
- CI、截图和测试产物不包含真实 Key、Prompt 或响应正文。

### 验收条件

- [x] 两个 Adapter 通过同一公共 Mock Contract Suite。
- [x] Streaming、取消、Usage、Finish Reason 和错误映射全部覆盖。
- [x] 429/Timeout/5xx Retry 有界且只访问当前 Provider Mock。
- [x] 认证、畸形响应和模型错误不自动重试。
- [x] 重定向不会把 Authorization 发送到第二 Host。
- [x] 当前 Provider 失败时另一 Provider 请求数严格为 0。
- [x] 默认测试不要求外部网络或真实 Key。
- [x] Loopback Mock 不能进入生产 Preset。
- [x] Live Smoke 入口与默认质量测试分离。
- [x] Contract 失败证据不包含 Header、Key、Prompt 或响应正文。
- [x] Sprint 1 离线 E2E 继续在无网络和无 Key 环境通过。
- [x] 统一质量入口能阻断任一 Provider 契约失败。

### 测试与证据

- 两家 Adapter 的公共契约结果矩阵；
- Mock 请求计数和目标 Host 清单；
- SSE/取消竞态报告；
- Retry/Retry-After/Timeout 时序报告；
- 重定向与 Authorization 泄漏负面报告；
- 默认测试无真实凭据环境断言；
- 质量门故障探针；
- 可选 Live Smoke 的脱敏状态模板。

### 完成定义

NG-204 只有在不联网、不配置真实 Key 的普通测试环境中，也能证明两家 Adapter 共享契约且绝不发生跨 Provider 请求时才算完成。

## 10. NG-205——实现中栏流式对话

### Story

作为作者，我希望在中栏向当前选中的 AI 提问并看到流式回答，以便在不修改小说文件的前提下获得写作辅助。

### 用户价值

用户第一次获得可用的 AI 对话，同时仍完全掌握哪些文本被发送、发送给谁以及是否复制到手稿。

### 实施范围

- 用真实对话流替换中栏占位消息；
- 显示当前 Provider、固定模型、配置/网络状态和发送按钮；
- 用户输入为空、过长、未配置或已有活动 Run 时禁止发送；
- 发送前冻结当前 Provider/Preset/模型并创建 RunId；
- 通过 provider_chat_start 启动 Rust 流；
- 监听并严格解析 provider-run-event；
- 按 sequence 合并 Text Delta，忽略重复或过期事件；
- 显示 queued/connecting/streaming/completed/failed/cancelled 状态；
- 生成期间显示停止按钮并调用 provider_chat_cancel；
- 完成后显示 Provider 返回的 Usage；缺失时显示“未提供”；
- 失败后保留用户输入和已收到文本，并提供同 Provider 重新发送入口；
- 对话正文按 S2-Q003 的确认策略保存在内存或本机状态；
- 不自动把当前项目、文件、编辑器 Selection 或搜索结果加入消息；
- 不提供文件写入、AI Apply、Workflow 或 Tool 入口；
- 关闭项目、切换项目或关闭窗口时安全取消活动 Run，不触发手稿保存。

### 安全约束

- 中栏只发送用户明确输入的文本和固定系统说明；
- 发送按钮旁显示接收方 Provider/模型；
- Prompt 和响应不进入普通日志、AppError、Telemetry 或截图证据；
- 流事件上限防止内存无限增长；
- React 渲染模型文本为纯文本/安全 Markdown，不执行 HTML、脚本、链接动作或代码；
- Provider 返回内容视为不可信数据，不能触发 Tauri Command；
- 未完成 Run 不允许 Provider 切换；
- 取消后迟到事件不能恢复 Run 或写入其他对话；
- Provider 故障不影响右栏未保存文本和 Project Lock。

### 验收条件

- [x] 已配置的任一 Provider 可以启动普通流式对话。
- [x] 发送前、中、后始终清晰显示 Provider 和固定模型。
- [x] Text Delta 按 sequence 稳定合并，重复/迟到事件安全处理。
- [x] 用户可以停止生成，Run 进入唯一 cancelled 终态。
- [x] 完成后显示 Usage 或明确“未提供”。
- [x] 未配置、无网络、Vault 不可用和活动 Run 状态正确禁用发送。
- [x] 当前项目内容不会被自动加入请求。
- [x] Provider 输出不会自动写入或修改任何 Markdown。
- [x] 模型输出不能执行 HTML、脚本、链接或 Tauri Command。
- [x] 失败后用户输入和已收到文本可见，错误保持脱敏。
- [x] 切换/关闭项目和关闭窗口不会留下继续发送的后台 Run。
- [x] Sprint 1 右栏编辑、保存、搜索和关闭保护回归通过。

### 测试与证据

- 对话输入和发送状态机组件测试；
- 流事件顺序、重复、迟到和唯一终态测试；
- 停止生成和关闭窗口取消集成测试；
- Prompt 只包含明确输入的负面断言；
- 模型恶意 HTML/脚本/链接输出渲染测试；
- 无 Canon 写入、无项目读取的命令计数断言；
- Provider 故障与右栏 dirty-state 隔离测试；
- DeepSeek/OpenAI Mock 流式 UI E2E；
- 真实 Tauri 中栏流式视觉证据。

### 完成定义

NG-205 只有在用户能清楚控制发送、停止和复制，同时模型输入输出对项目文件保持只读且无隐式上下文时才算完成。

## 11. NG-206——实现显式切换和禁止 Fallback

### Story

作为重视隐私和成本的作者，我希望明确决定每次请求由哪家 Provider 接收，以便失败时内容不会被静默转交给另一家公司。

### 用户价值

用户对数据接收方拥有可验证的控制权，不会因错误恢复或重试机制发生意外跨厂商传输。

### 实施范围

- 只允许在没有活动 Run 时切换 Provider；
- 切换前显示目标 Provider 和固定模型；
- 切换是独立的 provider_select Command，不自动发送或重放消息；
- 新 Run 启动时捕获 Provider/Preset/模型快照；
- Run 期间 UI、Retry 和取消都使用该快照；
- 当前 Provider 失败只显示失败和同 Provider 重试；
- 用户显式切换后必须再次按发送，才能把内容发给新 Provider；
- 不迁移 Provider 隐藏会话、推理、Response ID 或缓存 Key；
- 对话显示每条 Assistant 消息的 Provider/模型来源；
- 建立跨 Provider 请求计数和负面断言；
- 切换选择只写本机 SQLite，不修改 project.json 或 Markdown。

### 安全约束

- 不实现 automatic、best available、cheapest、fastest 等自动路由；
- 不在 Retry、Timeout、认证失败或模型不可用时切换 Provider；
- 不把同一 Run 的消息同时广播给两家 Provider；
- 不允许前端提交任意 Provider 字符串；
- 活动 Run 时切换请求必须被 Rust 和 UI 双重拒绝；
- Provider 选择变化不能改变 Key、Preset 或项目文件；
- 旧 Provider 的迟到事件不能显示在新 Provider Run 中。

### 验收条件

- [x] 只有 deepseek/openai 两个显式选择。
- [x] 活动 Run 期间无法切换，取消或终态后才可切换。
- [x] 切换动作本身网络请求数为 0。
- [x] 失败、Timeout、429 和认证错误均不会调用另一 Provider。
- [x] 切换后必须由用户再次按发送才产生新 Provider 请求。
- [x] 每条 Assistant 结果可见 Provider 和模型来源。
- [x] 旧 Run 迟到事件不会污染新 Run。
- [x] 选择持久化只写本机状态，不改项目。
- [x] 重启后选择恢复，但未配置 Provider 仍不能发送。
- [x] Mock 请求计数能证明每个 Run 只有一个 Provider。
- [x] 没有隐藏状态或 Response ID 跨 Provider 迁移。
- [x] 离线编辑不依赖任何 Provider 选择。

### 测试与证据

- 活动 Run 切换拒绝状态机测试；
- 切换零网络请求断言；
- DeepSeek 失败后 OpenAI 请求数为 0，反向同样；
- 用户切换并重新发送的正向 E2E；
- 迟到事件跨 Run/Provider 隔离测试；
- 选择持久化与 project.json 不变断言；
- Provider/模型来源 UI 快照。

### 完成定义

NG-206 只有在测试能证明“任何自动路径都不会改变数据接收方”，并且跨 Provider 发送一定需要新的用户动作时才算完成。

## 12. NG-207——建立密钥泄漏回归与 G2 E2E

### Story

作为产品负责人，我需要系统证明 Key 不会出现在常见状态、错误和产物中，并验证两家 Provider 的完整安全路径，以便批准 G2 Provider Ready。

### 用户价值

用户获得可重复验证的 BYOK 隐私保障，而不是仅依赖实现者承诺。

### 实施范围

- 为 DeepSeek/OpenAI 生成不同的高熵合成 Canary Key；
- 覆盖保存、替换、删除、连接测试、聊天、取消、错误和重启；
- 扫描 SQLite 导出、项目目录、App 日志、错误 JSON、测试快照、DOM、截图元数据和质量证据；
- 断言 Authorization Header、完整 Key 和可关联 Key 片段均不存在；
- 覆盖 Vault 不可用、SQLite 损坏、迁移失败和孤立引用；
- 覆盖认证失败、Rate Limit、Timeout、模型不可用、畸形响应和取消；
- 覆盖显式 Provider 切换和另一 Provider 请求数为 0；
- 覆盖无 Key、无网络时离线编辑完整可用；
- 建立 Sprint 2 Mock E2E；
- 建立可选 Live Smoke，使用用户本机 Vault 中的 Key；
- Live 证据只记录 Provider、Preset 版本、模型、脱敏结果和时间，不记录 Prompt、Response、Usage 明细或 Key；
- 将 Sprint 2 检查加入统一质量入口；
- 生成真实 Tauri G2 窗口证据，使用 Mock/合成内容而非真实手稿。

### 安全约束

- 扫描和清理只操作测试创建并验证的临时根；
- 不复制、导出或打印 Vault 中的真实 Secret；
- Live Smoke 不接受命令行 Key、环境变量 Key 或配置文件 Key；
- 测试失败保留脱敏结构证据，不保留正文、请求 Body 或响应 Body；
- Canary 泄漏视为阻断性失败，不能通过删断言或缩小扫描范围绕过；
- G2 E2E 不修改真实用户项目；
- Live Provider 不加入默认离线 CI；
- 质量门继续先失败即停止。

### 验收条件

- [x] 两个 Canary Key 在全部允许扫描目标中出现次数为 0。
- [x] SQLite Schema/导出、项目、日志、错误和 UI 快照不含 Secret。
- [x] Key 保存后不可读回，删除后 Provider 变为未配置。
- [x] 两家 Mock Contract Test 和流式 UI E2E 通过。
- [x] 当前 Provider 失败时另一 Provider 请求数为 0。
- [x] 无 Key、无网络和 Vault 不可用时 Sprint 1 离线 E2E 通过。
- [x] App State Migration、删除、损坏和对账测试通过。
- [x] Mock E2E 不依赖真实 Key或外部网络。
- [x] Live Smoke 按 S2-Q004 的批准策略执行或明确豁免。
- [x] Debug/Release 构建和真实窗口 Smoke Test 通过。
- [x] 统一质量入口包含 Sprint 2 边界、Contract、Leak 和 E2E。
- [x] G2 证据不包含 Prompt、Response、Key、用户路径或真实手稿。

### 测试与证据

- Canary 泄漏扫描报告；
- App State v1 → v2 Migration 与 Vault 对账报告；
- DeepSeek/OpenAI Mock Contract 矩阵；
- Streaming/取消/Retry/Fallback 请求计数；
- 无 Key/无网络离线编辑回归；
- 可选 Live Smoke 脱敏状态；
- Debug/Release 构建日志；
- 真实 Tauri G2 截图；
- 完整统一质量日志。

### 完成定义

NG-207 只有在自动化证据证明 Key 泄漏为 0、跨 Provider 自动请求为 0、Canon 自动写入为 0，并且 Sprint 1 离线编辑保持可用时才算完成。

## 13. 跨 Story 验收矩阵

| 能力 | NG-201 | NG-202 | NG-203 | NG-204 | NG-205 | NG-206 | NG-207 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| App State v2 | 主责 | 使用 | Preset | Fixture | 使用 | 选择 | 迁移回归 |
| Vault/Key | 主责 | 录入 | 借用 | Mock | 使用 | 隔离 | 泄漏主责 |
| Key-only UI | 状态 | 主责 | 展示 Preset | 错误态 | 发送状态 | 切换 | E2E |
| Provider 契约 | 边界 | 使用 | 主责 | 主责 | 使用 | 使用 | 汇总 |
| Streaming/取消 | 支撑 | 状态 | 协议 | 契约 | 主责 | Run 隔离 | E2E |
| 禁止 Fallback | 约束 | 显式选择 | Retry 边界 | 请求计数 | 失败态 | 主责 | 回归 |
| Canon 不写入 | 隔离 | 约束 | 无文件依赖 | 断言 | 主责 | 选择不写 | 汇总 |
| Local-first | Vault 故障隔离 | 禁用态 | 网络隔离 | 离线 Mock | 编辑回归 | 选择独立 | 主责 |
| G2 演示 | 必需 | 必需 | 必需 | 必需 | 必需 | 必需 | 汇总 |

## 14. Sprint 2 Gate G2

只有全部技术条件和产品批准满足，Sprint 2 才能结束：

- [x] NG-201～NG-207 均满足 Definition of Done。
- [x] App State Schema v1 → v2 Migration、回滚和删除边界通过。
- [x] Key 从前端保存后不可读回，SQLite/项目/日志/错误/证据中无完整 Key。
- [x] DeepSeek 与 OpenAI Mock Provider Contract Test 全部通过。
- [x] 固定 Preset、Host、协议和模型无法被用户输入覆盖。
- [x] Streaming、取消、Usage、错误和有界同 Provider Retry 通过。
- [x] 当前 Provider 失败不会触发另一 Provider 请求。
- [x] Provider 切换只发生在 Run 之间，且需要用户重新发送。
- [x] 中栏对话不自动读取项目内容、不修改 Canon。
- [x] 无 Key、无网络、Vault 不可用和 Provider 故障时离线编辑保持可用。
- [x] Sprint 1 离线 E2E、安全保存和恢复测试全部回归通过。
- [x] 密钥 Canary 泄漏数、跨 Provider自动请求数、AI Canon 写入数均为 0。
- [x] Live Smoke 按 S2-Q004 明确记录 G2 Live Waiver；未执行真实 Provider 请求。
- [x] 统一质量入口、Debug/Release Build 和真实 Tauri G2 演示通过。
- [x] 产品负责人批准进入 Sprint 3 Story Packet。

## 15. 待确认决策

### S2-Q001——Windows 密钥库后端

发现的歧义：权威架构允许“Tauri Stronghold 或操作系统凭据库”，但没有选择第一版 Windows 内部测试包的具体后端。

建议：第一版采用 Windows Credential Manager（或等价系统凭据 API），封装在纯 Rust CredentialVault 后面。理由是当前交付只面向 Windows，不需要新增 Stronghold 主密码/派生密钥生命周期，也不会向 WebView开放 Stronghold Guest 权限。未来 Android 通过同一 Trait 增加独立安全存储后端，不改变 Provider Core。

备选：采用 Tauri Stronghold，但仍只允许 Rust 窄 Command 访问，不给前端配置 stronghold:default；必须另行冻结 Vault 解锁秘密、Salt、迁移和恢复策略。

影响：该决定改变 NG-201 依赖、测试方式、数据迁移和未来 Android 后端，不可由 Story 实施者临时选择。

状态：已接受。第一版采用 Windows Credential Manager，并封装在纯 Rust CredentialVault 后面；不引入 Stronghold Guest。

### S2-Q002——Key 保存与连接测试是否分离

发现的歧义：上游只要求“输入 Key”和“连接测试”，没有定义网络不可用、Key 无效或模型暂时不可用时是否允许先保存。

建议：保存与测试分离。用户点击保存时先安全写入 Vault，状态为“已保存/未测试”；只有用户点击连接测试时才发送固定合成文本“仅返回 OK”，使用最小输出上限，并提示可能产生极少 API 用量。认证失败时保留加密 Key、标记“认证失败”，允许替换或删除；网络不可达时标记“无法连接”，离线编辑不受影响。

影响：接受后用户可以离线保存 Key，且错误分类更准确；拒绝并改为“验证成功才保存”会让断网时无法配置，并需要 Key 在一次较长网络操作中停留。

状态：已接受。保存与连接测试分离；NG-201 只实现保存、删除和状态查询，保存后为“已保存/未测试”，不发起网络请求。

### S2-Q003——Sprint 2 对话是否跨重启持久化

发现的歧义：架构允许 SQLite 保存标准化对话和 Run 元数据，但 Sprint 2 计划只要求流式对话；Sprint 3 才引入 Manager、检查点和正式工作流状态。

建议：Sprint 2 对话只保存在当前进程内存，关闭应用后清空；不创建 conversation、message 或 run 表。Sprint 3 在 Manager 状态和隐私边界一起批准后，再实现版本化持久化。当前用户可手动复制需要的结果到 Markdown。

影响：接受可显著缩小 Prompt/Response 本机隐私面和 NG-207 扫描范围；拒绝则必须在本 Packet 增加对话删除、保留期、数据库迁移、Crash Recovery 和隐私 UI，Story 规模会扩大。

状态：已接受。Sprint 2 对话只保存在当前进程内存，不创建 conversation、message 或 run 表。

### S2-Q004——真实 Provider Live Smoke 是否阻断 G2

发现的歧义：Sprint Plan 把 Live Test 描述为“开发者凭据启用的可选测试”，同时 G2 名称是 Provider Ready。只用 Mock 不能证明当前账号、固定模型和真实 Endpoint 在验收当天可用。

建议：Live Smoke 不进入默认 CI，也不要求用户交出 Key；但在 G2 产品验收前，由产品负责人在本机 Vault 中分别保存自己的 DeepSeek/OpenAI Key，各执行一次最小 Live Smoke。若当时缺少 Key、额度或网络，可明确签署一次 G2 Live Waiver；技术 Gate 仍必须通过全部 Mock/Leak/离线测试。

影响：接受后自动化始终可重复，Provider Ready 又有真实环境证据；完全取消 Live Smoke 会把模型弃用、账号权限和真实 SSE 差异推迟到更晚发现。

状态：已接受。Live Smoke 不进入默认 CI；G2 前在产品负责人本机执行，或明确记录一次 G2 Live Waiver。

## 16. 实施决策权限

以下属于 Story 内部低风险实现选择，可由实施者选择并记录：

- Rust Crate/模块内部命名，只要依赖方向清晰；
- HTTP Client 和 SSE Parser 的具体 Rust Library；
- Secret 类型与零化 Library；
- Mock Server Library 和 Fixture 目录；
- 同 Provider Retry 的精确次数与退避上限，只要有严格上限和契约测试；
- 单次消息、总对话、事件、Delta 和响应的大小上限；
- RunId、sequence 和内部事件通道实现；
- 连接状态图标、颜色、Loading 动效和中栏排版；
- Provider 错误到稳定中文消息的内部映射；
- G2 脱敏证据文件格式。

以下改变范围，必须停止并请求新决定：

- 更换或新增 Provider、模型、Base URL、协议或 Endpoint；
- 允许用户编辑任意 Provider 配置；
- 使用 ChatGPT 登录、OAuth、代理账户或云端密钥服务；
- 把 Key、Key Hash/片段或 Authorization Header写入 SQLite、项目或日志；
- 给 WebView 暴露通用 Vault、HTTP、URL、Shell、Process 或 Opener；
- 自动跨 Provider Fallback、自动模型发现或自动更换模型；
- 在用户未发送时读取项目正文或未保存编辑器文本；
- 添加 Context Manifest、Workflow Pack、Manager、Tool Calling、Proposal、Diff 或 AI Apply；
- 让 Provider 输出直接修改 Canon；
- 持久化 Prompt、Response 或对话历史；
- 把 Live Key放入 CI、命令行、环境变量、Fixture 或测试证据；
- 添加第三家 Provider、本地模型、云同步、账户、Android 或安装器；
- 并行启动多个 Story 或提前实现 Sprint 3。

## 17. Story Packet Review Gate

产品负责人已完成本 Review Gate，并确认：

1. Sprint 2 演示结果和 G2 Exit Gate 符合预期；
2. NG-201～NG-207 的范围、顺序和验收条件可接受；
3. 接受或修改 S2-Q001～S2-Q004；
4. 第一版只支持 DeepSeek 和 OpenAI 的固定 Preset；
5. Key-only、Rust 网络、不可读回、禁止 Fallback 和不写 Canon 边界可接受；
6. Sprint 2 普通对话不提前实现 Sprint 3 Workflow/AI Apply；
7. 团队继续按 1 人严格顺序执行；
8. 只授权 NG-201，完成后再逐 Story 授权，不并行启动后续 Story 或 Sprint 3。

批准结论：通过。S2-Q001～Q004 全部按建议接受；只授权 NG-201，完成并验收前不启动 NG-202 或其他后续 Story。

后续授权与验收记录：

- 产品负责人已明确批准 NG-201 验收，并授权开始 NG-202；
- NG-202 已完成技术实现与自动化验证；
- 产品负责人已一次性授权 NG-203～NG-207 按既定依赖顺序连续实施，并决定 NG-202～NG-207 在 Sprint 2 完成后统一验收；该授权不允许并行启动 Story 或提前进入 Sprint 3。
- 产品负责人已于 2026-08-22 批准 NG-202～NG-207 统一验收，并按 S2-Q004 明确记录一次 G2 Live Waiver；未执行 DeepSeek 或 OpenAI 真实请求，该批准不等于授权启动 Sprint 3。

## 18. 官方契约核对

本 Packet 在 2026-08-22 只使用以下官方资料核对已批准 Preset 和安全边界：

- [Tauri Stronghold](https://v2.tauri.app/plugin/stronghold/)——支持平台、初始化和权限模型；
- [Tauri Security](https://v2.tauri.app/security/)——WebView/IPC/应用核心信任边界；
- [OpenAI GPT-5.6 Terra](https://developers.openai.com/api/docs/models/gpt-5.6-terra)——模型 ID、Responses 和 Streaming 支持；
- [OpenAI Responses Streaming](https://platform.openai.com/docs/api-reference/responses-streaming)——SSE 事件契约；
- [DeepSeek Create Chat Completion](https://api-docs.deepseek.com/api/create-chat-completion)——deepseek-v4-pro、Chat Completions、SSE 与 Usage；
- [DeepSeek API](https://api-docs.deepseek.com/api/deepseek-api)——Bearer 认证协议。

官方资料只用于验证协议事实；产品范围仍以已批准架构和本 Packet Review Gate 为准。

## 19. 技术实施结果与 G2 Review Gate

截至 2026-08-22，产品负责人一次性授权的 NG-203～NG-207 已按依赖顺序完成；连同此前技术完成的 NG-202，均已获得统一产品验收。产品负责人同时按 S2-Q004 明确记录一次 G2 Live Waiver；该结论不授权进入 Sprint 3。

| Story | 技术状态 | 主要可复现证据 |
| --- | --- | --- |
| NG-201 | 产品已验收 | App State v2、Windows Credential Manager、不可读回与对账测试 |
| NG-202 | 产品已验收 | 双 Key-only 表单、保存前清 Buffer、确认删除和状态 UI |
| NG-203 | 产品已验收 | 固定 DeepSeek/OpenAI Preset、Rust Provider Core、显式连接测试 |
| NG-204 | 产品已验收 | 双 Provider Loopback Contract、错误/Retry/Timeout/重定向/取消矩阵 |
| NG-205 | 产品已验收 | 单活动 Run、严格流式事件、停止生成、内存对话和来源标签 |
| NG-206 | 产品已验收 | 运行中前后端切换锁、预期 Provider 绑定、同 Provider 手动重试 |
| NG-207 | 产品已验收 | 高熵 Canary、SQLite/项目/错误/DOM/证据扫描、Sprint 2 Mock E2E、真实 G2 窗口 |

统一质量入口现在按失败即停止顺序覆盖：

1. 前端格式、Lint、类型和 91 项组件/契约测试；
2. Sprint 2 静态边界、Rust Clippy 和锁定离线测试；
3. Sprint 1 无 Key/无网络编辑 E2E；
4. Tauri Debug/Release 构建与真实窗口 Smoke；
5. Sprint 2 App State、Vault、Provider Contract、Canary、DOM 和离线编辑汇总 E2E。

本地 G2 脱敏证据：

- `target/g2-evidence/tauri-g2-window.png`：真实 Release Tauri 主窗口，使用空白/合成状态，不含真实手稿；
- `target/g2-evidence/security-report.json`：`canaryLeakCount=0`、`crossProviderAutomaticRequestCount=0`、`canonAutomaticWriteCount=0`、`liveSmoke.status=waived`；
- `pnpm smoke:provider:live`：独立于默认质量门，拒绝参数和环境变量密钥，只从固定 Windows Credential Manager 目标临时借用 Secret。

产品负责人已完成 G2 外部环境决策：

- 按 S2-Q004 明确记录一次 G2 Live Waiver；
- 本次未运行 `pnpm smoke:provider:live`，未向 DeepSeek 或 OpenAI 发起真实请求；
- Waiver 不替代 Mock Contract、Leak、离线编辑、Debug/Release Build 或真实窗口检查，上述本地门禁均已通过。

NG-201～NG-207、统一产品验收、G2 Live Smoke/Waiver 决策以及全部可自动复现的 G2 条件均在交接时完成。产品负责人已明确批准进入 Sprint 3 Story Packet，G2 已通过并关闭；当时 Sprint 3 仍需先完成其 Story Packet Review Gate，NG-301 尚未授权。

## 20. G2 产品批准与 Sprint 2 关闭记录

批准日期：2026-08-22。BMAD 数据包：`review-gate-packet`。

- 产品负责人明确回复“批准 G2 验收，创建 Sprint 3 Story Packet”；
- G2 最后一项产品 Gate 已勾选，Sprint 2 正式关闭；
- G2 Live Waiver 保持有效，本次未补跑 DeepSeek/OpenAI 真实 Live Smoke；
- 已创建 [Sprint 3 Story Packet](./sprint-3-story-packet.zh-CN.md)，其交接时状态为等待 S3-Q001～S3-Q005 与 Story Packet Review Gate；
- 该批准只授权创建 Packet，不等于批准 Packet 内容或授权 NG-301；
- Sprint 3 继续按 1 人、逐 Story 明确授权、严格顺序执行。
