# NovelGainker Sprint 1 Story Packet

> 状态：NG-101～NG-107 已实施，G1 已由产品负责人批准，Sprint 1 已关闭；Sprint 2 Story Packet 已创建  
> 日期：2026-08-22  
> BMAD 数据包：execution-packet  
> Sprint：1——离线本地编辑  
> 范围：NG-101～NG-107  
> 团队：1 人，严格顺序实施  
> 上游计划：[sprint-plan.zh-CN.md](../sprint-plan.zh-CN.md)（已批准）  
> 上游架构：[architecture.zh-CN.md](../architecture.zh-CN.md)（已批准）  
> 前置 Gate：[sprint-0-story-packet.zh-CN.md](./sprint-0-story-packet.zh-CN.md)（G0 已通过）

## 1. Sprint 目标

在完全没有网络、没有 API Key、没有 AI Provider 的情况下，让用户可以在 Windows 11 x64 上安全创建或打开一个 NovelGainker Markdown 项目，浏览文件树，直接编辑并可靠保存 Markdown，关闭应用后重新打开仍不丢失内容。

Sprint 1 的核心不是“让文本框写进文件”，而是建立不会破坏用户手稿的本地项目基础：

- Markdown 与项目资源始终是唯一权威来源；
- 所有文件能力由 Rust 和带类型 Tauri Command 执行；
- React 只提交项目句柄、项目相对路径、源文件 Hash 和新文本；
- 保存必须经过路径沙箱、外部修改检查和原子替换；
- 格式错误允许保存，但必须显示诊断，并为后续 AI Apply、Final 和 EPUB 留下阻断状态；
- 最近项目、布局偏好、搜索索引和恢复信息只能是可删除、可重建的本机状态；
- Sprint 1 完成后，用户即使永远不配置 AI，也能把 NovelGainker 当作可靠的离线 Markdown 编辑器使用。

## 2. Sprint 1 可演示结果

G1 Review 应能按以下顺序复现：

1. 断开外部网络并启动 Windows 内部开发构建；
2. 选择一个空目录，确认后创建 NovelGainker 原生项目；
3. 查看 project.md、AGENTS.md、标准源目录、materials/000.story-ledger.md 和 .novelgainker/project.json；
4. 打开一个 Markdown 文件，在右栏直接修改，看到明确的未保存状态；
5. 使用保存按钮或 Ctrl+S 原子保存，关闭并重新打开应用，确认内容仍在；
6. 保存一个不符合严格契约的 Markdown，确认文件仍安全保存，同时 UI 显示结构诊断；
7. 用外部编辑器修改当前文件，确认 NovelGainker 阻止静默覆盖并要求用户重新加载或另存内容；
8. 再次打开同一项目，确认第二个 Writer 被拒绝并进入只读状态；
9. 在完全离线状态下搜索项目文本，搜索结果只定位文件和片段，不修改 Canon；
10. 折叠侧栏、调整栏宽和选择当前文件，重启后确认本机偏好恢复；
11. 删除 App 本机状态后重新打开项目，确认 Markdown 项目仍完整可读；
12. 运行统一质量入口和离线 E2E，全部通过。

## 3. 范围边界

### 3.1 本 Sprint 包含

- Windows 原生目录选择；
- NovelGainker v1 项目 Scaffold；
- 已有项目扫描、导入预检与冲突说明；
- Rust Project Root 绑定和不透明项目会话句柄；
- 白名单文件树和 Markdown 源文本读取；
- 右栏直接编辑、未保存状态、显式保存和离开确认；
- 路径标准化、项目根边界、符号链接与 Reparse Point 防护；
- 同目录临时文件、Flush 和 Windows 原子替换；
- 源文件 Hash、外部修改检测和冲突恢复；
- 单 Writer Project Lock、第二实例只读；
- 脱离项目正文的事务恢复日志；
- Markdown/Frontmatter 本地校验；
- 完全离线的项目内全文搜索；
- 最近项目、栏宽、折叠状态和当前相对文件等本机偏好；
- Windows 文件系统负面测试和 Sprint 1 离线 E2E。

### 3.2 明确不做

- 不接入 DeepSeek、OpenAI 或任何网络请求；
- 不保存、读取或测试真实 API Key；
- 不实现 Vault、Provider Profile、Streaming、Token 用量或对话历史；
- 不实现 AI 提案、Diff、确认 Apply、Workflow Pack 或 Manager；
- 不实现 EPUB、图片生成、云同步、Git 管理或 Android；
- 不自动保存，不在后台定时写手稿；
- 不递归删除用户文件，不提供通用文件浏览器或任意路径 API；
- 不加载 Novelist Plugin Cache，不打包 SKILL.md、Python 或通用 Shell；
- 不把 SQLite、索引、最近项目或恢复日志变成隐藏 Canon。

## 4. 关键契约

### 4.1 项目格式

新项目创建以下权威结构：

~~~text
project.md
AGENTS.md
characters/
materials/
  000.story-ledger.md
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
~~~

project.json 使用固定的 v1 字段：

~~~json
{
  "schemaVersion": 1,
  "projectId": "uuid",
  "workflowPackVersion": "1.0.0",
  "defaultProviderId": "openai",
  "writingLanguage": "zh-CN",
  "strictChapterMarkup": true
}
~~~

defaultProviderId 只是可移植的非敏感预设提示，不表示 Provider 已配置、已选中或允许联网。该文件不得包含 API Key、Token、Provider Response ID、设备用户名或绝对路径。

没有 project.json 的现有 Markdown 项目允许先执行只读扫描。只有用户看到拟创建的元数据和冲突清单并确认后，App 才能写入 .novelgainker/project.json。

### 4.2 Tauri 与路径边界

- 目录选择产生 Rust 持有的不透明 ProjectSessionId；
- 前端后续只能提交 ProjectSessionId 和项目相对路径；
- Command 不接受任意绝对路径、可执行文件、Shell、URL 或 Provider 参数；
- Rust 每次读写都重新验证根目录、路径组件和最终解析结果；
- 只允许已批准目录中的 Markdown 源文件以及受控的 project.json；
- published/ 视为生成目录，Sprint 1 不允许通过编辑器把它当作源文件修改；
- 新增 Command 必须登记输入、输出、读写分类、Capability、错误码和负面测试。

### 4.3 手动保存契约

- 用户按保存按钮或 Ctrl+S 即表示确认保存自己的手动编辑；
- 保存请求必须携带读取时的源 Hash；
- Hash 已变化时停止写入，不提供“仍然覆盖”快捷按钮；
- 格式错误不阻止原子保存，但保存后显示错误诊断；
- 诊断状态会阻止未来 AI Apply、Final 标记和 EPUB 发布，但 Sprint 1 不实现这些入口；
- 保存成功后才更新编辑器基线和已保存状态；
- 保存失败必须保留原文件和编辑器内未保存文本。

## 5. 依赖与执行顺序

~~~mermaid
flowchart LR
    G0[G0 已通过] --> NG101[NG-101 项目创建与打开]
    NG101 --> NG102[NG-102 文件树与编辑器]
    NG102 --> NG103[NG-103 路径沙箱与原子保存]
    NG103 --> NG104[NG-104 Lock、冲突与恢复]
    NG102 --> NG105[NG-105 校验与搜索]
    NG101 --> NG106[NG-106 本机偏好]
    NG104 --> NG107[NG-107 离线 E2E]
    NG105 --> NG107
    NG106 --> NG107
    NG107 --> G1[G1 Offline Editor]
~~~

单人团队按以下顺序执行：

1. NG-101；
2. NG-102；
3. NG-103；
4. NG-104；
5. NG-105；
6. NG-106；
7. NG-107；
8. G1 综合验收。

NG-106 在上游计划中只列 NG-002 依赖；本 Packet 补充 NG-101 为有效依赖，因为最近项目和当前相对文件必须建立在真实项目会话之上。这是依赖细化，不改变 Story 范围。

## 6. NG-101——实现项目创建与打开

### Story

作为中文小说作者，我希望选择一个本地目录创建或打开 NovelGainker 项目，以便开始离线管理自己的 Markdown 手稿。

### 用户价值

用户获得一个可移植、自解释、无需数据库或云服务也能读取的小说项目。

### 实施范围

- 使用受限原生目录选择能力，不让前端输入任意路径；
- 创建前扫描目标目录并生成创建计划；
- 空目录可创建完整 v1 Scaffold；
- 非空目录必须展示现有文件、拟创建文件和冲突；
- 任何现有同名文件都不得静默覆盖；
- 生成 projectId、schemaVersion 1、writingLanguage zh-CN 和严格标记默认值；
- 打开已有项目时验证目录、元数据和基础结构；
- 没有 project.json 时只读推导默认值，经确认后才创建元数据；
- Rust 返回不透明项目会话、显示名称、能力状态和相对文件摘要；
- 登记固定 project_create、project_open、project_scan 和 project_close Command 及最小 Capability。

### 安全约束

- 不读取所选根目录以外的文件；
- 创建失败时清理仅由本次事务新建的空目录或文件，不删除原有内容；
- 错误 UI 不显示 Rust Debug、环境变量或非用户主动选择的机器路径；
- project.json 不包含本机绝对路径或秘密；
- 不创建 Git 仓库，不复制 Novelist Plugin 资源。

### 验收条件

- [x] 空目录可以生成完整 v1 Scaffold。
- [x] 创建后 project.json 满足固定 Schema 和 zh-CN 默认值。
- [x] 非空目录先显示计划和冲突，未确认时写入数为 0。
- [x] 任一目标文件已存在时不会被覆盖或截断。
- [x] 可打开带 project.json 的有效项目。
- [x] 无 project.json 的 Markdown 项目可以只读预检，并经确认补充元数据。
- [x] 无效 schemaVersion、损坏 JSON 和不支持结构显示稳定中文错误。
- [x] 前端只取得项目会话 ID 和展示信息，不获得通用文件系统能力。
- [x] Capability 只增加本 Story 明确登记的项目 Command。
- [x] 全流程不访问网络、Key、SQLite Provider 表或 Vault。

### 测试与证据

- Scaffold Golden Test；
- 空目录、非空目录、同名冲突和中途失败事务测试；
- project.json Schema/敏感字段负面测试；
- Tauri Command 契约与 Capability 快照；
- Windows 临时目录集成测试；
- 真实 Tauri 创建和打开截图。

### 完成定义

NG-101 只有在已有文件永不被静默覆盖、创建失败不会破坏目标目录，并且项目脱离 App 状态仍然自解释时才算完成。

## 7. NG-102——实现项目文件树和直接 Markdown 编辑

### Story

作为作者，我希望在右栏选择项目 Markdown 文件并直接修改源文本，以便不依赖 AI 完成日常写作。

### 用户价值

用户可以把 NovelGainker 当作真正的离线 Markdown 编辑器，而不只是项目查看器。

### 实施范围

- 从 Rust 返回的白名单项目索引渲染文件树；
- 只显示权威源目录中的允许文件和受控元数据；
- 选择 Markdown 后读取 UTF-8 文本、相对路径、内容 Hash 和修改时间；
- 用右栏当前编辑器替换 Sprint 0 临时缓冲区；
- 明确显示当前文件、只读状态、已保存、未保存、保存中和保存失败；
- 文件切换、关闭项目或关闭窗口前保护未保存文本；
- 提供保存按钮和 Ctrl+S 入口，但在 NG-103 安全保存完成前不得使用临时不安全写入；
- 大文件采用有上限的读取与 UI 提示，限制值由实现记录并测试；
- 右栏继续直接显示 Markdown 源文本，不增加预览/编辑标签。

### 安全约束

- 前端不能提交项目绝对路径；
- 不能读取隐藏秘密文件、.git、App 状态目录或项目外链接；
- published/ 不作为可编辑源；
- 不把正文写入浏览器 Local Storage、日志或错误诊断；
- 未保存文本只存在当前进程内存，直到 NG-103 原子保存成功。

### 验收条件

- [x] 文件树只显示允许的项目相对路径。
- [x] 选择 Markdown 后可直接编辑真实源文本。
- [x] 当前文件和已保存/未保存状态始终清晰。
- [x] 切换文件、关闭项目和关闭窗口不会静默丢弃未保存文本。
- [x] 保存入口在安全写入服务不可用时明确禁用，不存在临时直接覆盖。
- [x] 当前文件内容不进入 Local Storage、日志或 AppError。
- [x] published/、.git 和未知隐藏目录不能通过编辑器打开。
- [x] 窄窗口下文件树仍以右侧抽屉工作，中栏不被挤压。
- [x] 键盘可以选择文件、聚焦编辑器并触发保存入口。
- [x] Sprint 0 三栏、健康检查和中文错误回归通过。

### 测试与证据

- 文件树白名单和排序单元测试；
- 编辑器 dirty-state 状态机测试；
- 切换、关闭和窗口退出保护组件测试；
- 大文件边界和非 UTF-8 错误测试；
- 无 Local Storage、无正文日志负面断言；
- 宽屏和窄屏真实窗口视觉 QA。

### 完成定义

NG-102 不允许为了“先能保存”而引入非原子直接写入；真正落盘由 NG-103 激活。

## 8. NG-103——实现路径沙箱与原子保存

### Story

作为作者，我希望保存失败或恶意路径都不能损坏手稿，以便放心把 NovelGainker 用于真实小说项目。

### 用户价值

用户的手动修改可以可靠落盘，同时原文件在失败、冲突或越界请求下保持完整。

### 实施范围

- 建立 Rust Scoped Project Filesystem；
- 验证 ProjectSessionId、相对路径、允许目录和扩展名；
- 拒绝绝对路径、空路径、点路径、上级路径和保留设备名；
- 检测符号链接、Junction 和 Reparse Point 越过项目根；
- 保存请求携带源 Hash，新 Hash 不一致时拒绝写入；
- 在目标同目录创建唯一临时文件；
- 写入 UTF-8、Flush 文件内容，并使用 Windows 原子替换；
- 成功后返回新 Hash、修改时间和诊断触发标记；
- 失败时保留旧文件及前端未保存文本；
- 只清理由本次事务创建且已验证的临时文件；
- 激活 NG-102 保存按钮和 Ctrl+S。

### 安全约束

- 不支持递归删除、任意复制、任意重命名或项目外另存；
- 不接受模型、URL、Shell 或用户拼接的绝对目标；
- 原子替换实现必须有 Windows 实际文件系统测试；
- 测试故障注入不得使用用户真实项目；
- 无论格式是否有效，都先保护文件完整性，再返回本地诊断。

### 验收条件

- [x] 合法 Markdown 可通过同目录临时文件原子替换。
- [x] 保存成功返回新 Hash，编辑器变为已保存。
- [x] 绝对路径、..、符号链接、Junction 和 Reparse Point 越界全部被拒绝。
- [x] 源 Hash 过期时目标文件保持不变。
- [x] 写入、Flush 或替换阶段失败时旧文件不丢失、不截断。
- [x] 临时文件命名不可预测且只在项目内目标目录创建。
- [x] 无效严格 Markdown 仍可安全保存，并返回诊断状态。
- [x] 普通错误 UI 只显示项目相对路径和稳定中文恢复建议。
- [x] 保存实现不使用 Shell、外部 CLI 或 Python。
- [x] Windows 真实文件系统负面矩阵通过。

### 测试与证据

- 路径解析 Property Test；
- Windows 绝对路径、UNC、盘符、设备名、Junction、Symlink 和 Reparse Point Fixture；
- 写入/Flush/Replace 故障注入测试；
- 原文件 Hash 前后断言；
- 非法 Markdown 允许保存回归；
- Tauri 保存 Command 端到端测试。

### 完成定义

任何保存失败都必须满足二选一：旧文件保持原样，或新文件完整成为目标文件；不允许出现截断、半写或不明状态。

## 9. NG-104——实现 Project Lock、外部修改检测与恢复日志

### Story

作为作者，我希望多个窗口、外部编辑器或异常退出不会静默覆盖我的手稿，以便在复杂使用场景中仍能安全恢复。

### 用户价值

用户能明确知道谁拥有写权限、文件是否已被外部修改，以及异常事务需要如何恢复。

### 实施范围

- 每个项目建立单 Writer 进程级 Lock；
- 第二个进程或窗口打开同一项目时进入只读状态；
- 通过 File Watcher 和内容 Hash 检测外部修改；
- 当前文件外部变化时显示重新加载、保留内存文本并复制内容的恢复选择；
- 不提供跳过 Hash 检查的强制覆盖按钮；
- 在 App 状态目录保存脱敏事务日志，不写入手稿正文；
- 日志只记录事务 ID、项目 ID、相对路径、阶段、Hash 和临时文件标识；
- 启动或重新打开项目前扫描未完成事务；
- 根据实际文件和 Hash 确定完成、回滚、清理或需要用户处理；
- 恢复完成前禁止新的写事务。

### 安全约束

- Lock 和恢复日志不得保存手稿正文；
- 崩溃恢复只操作日志明确登记且仍位于项目根内的文件；
- 不自动重试含糊写入；
- 不通过 PID 猜测直接删除任意 Lock；必须验证所有权和进程状态；
- 不实现跨设备同步或网络锁。

### 验收条件

- [x] 同一项目同一时间只有一个 Writer。
- [x] 第二实例只能只读，不会获得保存能力。
- [x] 外部修改会使当前源 Hash 失效并阻止覆盖。
- [x] 用户可以重新加载磁盘版本，同时保留或复制内存文本。
- [x] 事务日志不包含正文、绝对路径、用户名或秘密。
- [x] 启动时能识别未完成的写事务。
- [x] 可确定恢复的事务自动恢复，并显示中文结果。
- [x] 含糊状态停止写入并要求用户处理，不猜测成功。
- [x] 恢复完成前不能开始新保存。
- [x] Crash、进程终止和外部修改测试不会破坏原文件。

### 测试与证据

- 双进程/双窗口 Lock 集成测试；
- File Watcher 与 Hash 竞态测试；
- 事务阶段 Crash Fixture；
- 恢复日志 Schema 和敏感字段负面测试；
- 只读第二实例组件测试；
- Windows 实际外部编辑演示。

### 完成定义

NG-104 完成后，任何并发或异常写入都必须可归类为已保存、已回滚或需要恢复，不能静默覆盖或处于未知可继续写状态。

## 10. NG-105——实现 Markdown/Frontmatter 校验和本地搜索

### Story

作为作者，我希望保存后立即看到结构问题，并能离线搜索整个项目，以便发现和定位内容而不依赖 AI。

### 用户价值

用户可以在保留写作自由的同时获得确定性诊断和快速本地导航。

### 实施范围

- 建立或扩展 novelgainker-validation Rust Crate；
- 校验 UTF-8、YAML Frontmatter、必需字段和章节严格标记；
- 诊断包含稳定 code、severity、相对路径、行列、中文消息和恢复建议；
- 保存前诊断只作提示，保存后诊断绑定新文件 Hash；
- 格式错误不阻止手动保存；
- 明确标记该文件暂不允许未来 AI Apply、Final 或 EPUB；
- 在允许的 Markdown 源目录执行离线全文搜索；
- 中文和英文文本至少支持确定性字面/大小写无关匹配；
- 返回有上限的相对路径、行号和短片段；
- 搜索结果只导航和选择候选，不修改文件或 Canon；
- 索引若存在必须可从 Markdown 全量重建。

### 安全约束

- 搜索不扫描项目根外、.git、published 生成物或隐藏秘密；
- 查询和命中正文不进入日志或 AppError；
- 搜索不发网络请求，不进入 AI Context；
- 解析恶意 Frontmatter 时有大小、深度和耗时边界；
- 索引损坏只能导致重建，不能阻止直接打开项目文件。

### 验收条件

- [x] 有效 Markdown/Frontmatter 返回无阻断诊断。
- [x] 格式错误仍可保存，并显示稳定中文诊断。
- [x] 诊断绑定内容 Hash，文件变化后旧诊断标为过期。
- [x] 章节严格标记错误被识别。
- [x] 本地搜索在断网状态可返回文件、行号和短片段。
- [x] 搜索结果只导航，不写文件、不改变 Canon。
- [x] 查询、正文和片段不进入日志或错误诊断。
- [x] 恶意或超大 Frontmatter 不导致无限解析或崩溃。
- [x] 删除索引后可从 Markdown 重建。
- [x] Provider、Key 和 AI 发送入口保持禁用。

### 测试与证据

- Validator Fixture/Golden Test；
- Frontmatter 语法、缺字段、严格章节标记和恶意深度测试；
- 中英文搜索 Fixture；
- 索引删除/损坏重建测试；
- 查询与正文隐私负面断言；
- 完全离线搜索组件和集成测试。

### 完成定义

校验和搜索都只能解释或定位权威 Markdown；任何缓存或诊断历史不得成为内容来源。

## 11. NG-106——持久化本机 UI 偏好

### Story

作为作者，我希望重启应用后恢复最近项目和工作区布局，以便继续上次的离线写作。

### 用户价值

应用具有连续使用体验，同时项目仍不依赖单台电脑的状态数据。

### 实施范围

- 建立版本化的 App State Repository；
- 保存最近项目、窗口级栏宽、折叠状态和每项目当前相对文件；
- 本机可以保存用户主动选择的最近项目绝对路径，但该路径永不写入项目；
- 项目状态以 projectId 和相对文件关联；
- 启动时清理不存在或不可访问的最近项目项；
- 偏好损坏或版本不兼容时使用安全默认值；
- 删除 App 状态后可以重新选择项目并完整读取；
- 不保存 Markdown 正文、搜索查询、API Key、Provider 响应或错误堆栈；
- 若 S1-Q001 通过，使用最小 SQLite，只包含 Sprint 1 可重建本机状态；Sprint 2 在此基础上扩展 Provider/Vault 引用。

### 安全约束

- SQLite 或替代状态文件不属于项目、不进入用户仓库；
- 状态数据库损坏不能阻止手动打开项目；
- 当前文件只保存项目相对路径；
- UI 偏好写入不能与手稿保存共用事务或影响 Canon；
- 不提前创建密钥、Provider、对话或 Token 表。

### 验收条件

- [x] 最近项目在重启后可见。
- [x] 左右栏折叠、栏宽和 API 卡折叠状态可恢复。
- [x] 每项目当前相对文件可恢复。
- [x] 项目目录绝对路径只存在本机状态，不进入 project.json。
- [x] 删除 App 状态后仍可手动打开并读取项目。
- [x] 状态损坏时使用默认值并显示安全中文提示。
- [x] App 状态不包含 Markdown 正文、查询、Key 或 Provider 数据。
- [x] 状态 Schema 有版本和迁移边界。
- [x] 偏好保存失败不影响手稿保存。
- [x] Sprint 0 临时状态测试更新为真实偏好边界测试。

### 测试与证据

- App State Repository 单元测试；
- 重启恢复组件/E2E；
- 数据库或状态文件删除、损坏和版本变化测试；
- 项目元数据无绝对路径断言；
- 状态导出敏感字段负面扫描；
- 手稿保存与偏好保存故障隔离测试。

### 完成定义

本机状态可以被整个删除而不损坏项目、不丢失已保存手稿，也不影响项目在另一台机器上的可读性。

## 12. NG-107——建立 Sprint 1 离线 E2E

### Story

作为产品负责人，我需要在禁止网络和没有 Key 的环境中验证完整编辑路径，以便确认 G1 是真正可用的离线编辑器。

### 用户价值

用户获得一条可重复证明的创建、编辑、保存、恢复和重开路径。

### 实施范围

- 建立隔离的 Windows 临时项目 Fixture；
- 在外部网络被禁止的测试条件下运行；
- 覆盖创建、扫描、打开、文件树、编辑、保存、关闭和重开；
- 覆盖无效 Markdown 允许保存并显示诊断；
- 覆盖路径越界、外部修改、第二 Writer、Crash Recovery；
- 覆盖本地搜索和 UI 偏好重启恢复；
- 删除 App 状态后重新选择并打开项目；
- 断言没有延迟、排队或重连后自动发出的 AI 请求；
- 将 Sprint 1 检查加入统一质量入口；
- 真实 Tauri 窗口完成 G1 演示并保留证据。

### 安全约束

- E2E 只操作明确创建并验证位于测试临时根内的 Fixture；
- 清理前解析并验证绝对目标仍在测试根内；
- 不使用真实用户项目、真实 Key 或外部网络；
- 不通过跳过检查、固定成功 Mock 或降低 Capability 完成测试；
- 测试失败必须保留足够脱敏证据，不保留正文或机器用户名。

### 验收条件

- [x] 无网络、无 Key 条件下完整编辑路径通过。
- [x] 保存后关闭并重开，内容 Hash 与预期一致。
- [x] 无效 Markdown 安全保存且诊断可见。
- [x] 所有路径越界请求被拒绝。
- [x] 外部修改与第二 Writer 不会导致覆盖。
- [x] Crash Recovery 后项目处于确定状态。
- [x] 本地搜索和偏好恢复通过。
- [x] 删除 App 状态后项目仍可打开和读取。
- [x] 测试期间外部网络请求数为 0，后续也无排队请求。
- [x] 统一质量入口、Debug 启动、Release Build 和 Windows Smoke Test 全部通过。

### 测试与证据

- 离线 E2E 日志；
- 关键阶段源文件 Hash 清单；
- Windows 路径攻击矩阵；
- Lock、外部修改和 Crash Recovery 报告；
- App 状态删除重建报告；
- 真实 Tauri G1 截图；
- 完整统一质量日志。

### 完成定义

NG-107 只有在一台不配置 Provider、没有 Key、禁止外部网络的 Windows 11 x64 环境中，可以从空目录完成创建到重开的完整路径时才算完成。

## 13. 跨 Story 验收矩阵

| 能力 | NG-101 | NG-102 | NG-103 | NG-104 | NG-105 | NG-106 | NG-107 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 项目创建/打开 | 主责 | 使用 | 边界 | Lock | 校验 | 最近项目 | E2E |
| Markdown 文件树/编辑 | 支撑 | 主责 | 安全保存 | 冲突 | 诊断/搜索 | 当前文件 | E2E |
| 路径与文件安全 | 初始 Root | 只读白名单 | 主责 | 恢复 | 扫描边界 | 状态隔离 | 攻击矩阵 |
| Local-first | 约束 | 使用 | 使用 | 恢复 | 搜索 | 偏好 | 主责 |
| Canon 独立于 App 状态 | 项目格式 | 正文 | 原子写入 | 日志隔离 | 索引可重建 | 主责 | 删除状态 |
| 无 API/Key/网络 | 约束 | 约束 | 约束 | 约束 | 约束 | 约束 | 回归 |
| G1 演示 | 必需 | 必需 | 必需 | 必需 | 必需 | 必需 | 汇总 |

## 14. Sprint 1 Gate G1

只有全部条件满足，Sprint 1 才能结束：

- [x] NG-101～NG-107 均满足 Definition of Done。
- [x] Windows 11 x64 离线 E2E 全部通过。
- [x] 新建和已有 Markdown 项目均可打开。
- [x] 用户可以直接编辑、显式保存、关闭并重开且内容不丢失。
- [x] 保存失败不会导致原文件丢失、截断或未知状态。
- [x] 路径越界、符号链接、Reparse Point 和并发修改负面测试通过。
- [x] 第二 Writer 不能静默写入。
- [x] 手动格式错误允许原子保存，同时诊断与后续阻断状态正确。
- [x] 搜索和校验完全离线，不修改 Canon。
- [x] 删除 App 本机状态后项目仍可打开和读取。
- [x] 没有真实 API Key、Provider 请求、Vault、AI Apply 或 Python/Shell。
- [x] 统一质量入口和真实 Tauri G1 演示通过。
- [x] 产品负责人批准进入 Sprint 2 Story Packet。

## 15. 待确认决策

### S1-Q001——Sprint 1 是否建立最小 SQLite

发现的歧义：G1 明确要求删除 App SQLite 后项目仍可打开，但追踪表把完整 SQLite/Vault/Provider 能力列在 Sprint 2。

建议：Sprint 1 在 NG-106 建立最小、无秘密、可删除的 SQLite App State，只保存最近项目、UI 偏好以及可重建索引元数据；恢复日志仍按事务安全选择文件或表。Sprint 2 的 NG-201 扩展 Provider 状态并接入加密 Vault。

影响：接受后可直接验证 G1 的数据库删除边界；拒绝则 Sprint 1 使用版本化本机状态文件，并把 G1 文案改为“删除 App 本机状态”。

状态：已接受。NG-106 使用最小、无秘密、可删除的 SQLite App State；不提前创建 Provider、Vault 或 Key 表。

### S1-Q002——第二实例的 Lock 体验

建议：Sprint 1 第二实例只读，不提供“请求转移 Lock”。用户关闭 Writer 后可重新打开获得写权限。Lock 转移会引入跨窗口协调和未保存内容处置，首版离线编辑不需要承担。

状态：已接受。第二实例只读，不实现 Lock 转移。

### S1-Q003——保存方式

建议：Sprint 1 只提供显式保存按钮和 Ctrl+S，不做自动保存或定时保存。关闭、切换文件和切换项目时提示未保存内容。

状态：已接受。仅显式保存按钮与 Ctrl+S，不自动保存。

### S1-Q004——编码与换行

建议：新文件使用 UTF-8 无 BOM 和 LF；已有 UTF-8 文件保存时保留检测到的 BOM/无 BOM 与 LF/CRLF。非 UTF-8 文件不自动转码，以只读错误状态打开并提示用户先备份转换。

状态：已接受，并按补充方案执行：已有 UTF-8 文件同时保留 BOM 状态和换行风格。

## 16. 实施决策权限

以下属于 Story 内部低风险实现选择，可由实施者选择并记录：

- Rust Crate 内部模块命名；
- 安全文件句柄和 Watcher 的具体库；
- Windows 原子替换调用的封装方式；
- 搜索结果上限和单文件大小上限，只要有文档与边界测试；
- UI dirty-state 图标、颜色和布局细节；
- 事务日志的文件或表结构，只要不含正文且可恢复；
- 测试临时目录和故障注入接口。

以下改变范围，必须停止并请求新决定：

- 自动保存、后台写入或静默覆盖；
- 允许用户跳过 Hash 冲突强制覆盖；
- 允许项目外路径、任意扩展名或通用文件 API；
- 把 Markdown 正文变成 SQLite Canon；
- 在项目中保存绝对路径、Key 或本机秘密；
- 引入 Provider、网络请求、Vault 或真实 API Key；
- 添加 Shell、Process、Python、Novelist Plugin Runtime；
- 自动创建 Git 仓库、云同步或 Android；
- 提前实现 Sprint 2 及以后的能力。

## 17. Story Packet Review Gate

开始 NG-101 前，需要产品负责人确认：

1. Sprint 1 的演示结果和 G1 Exit Gate 符合预期；
2. NG-101～NG-107 的范围、顺序和验收条件可接受；
3. 接受或修改 S1-Q001～S1-Q004；
4. Sprint 1 保持完全离线，不接入 Provider、Key 或 Vault；
5. 团队继续按 1 人严格顺序执行；
6. 只授权 NG-101，完成后再逐 Story 授权，不并行启动后续 Story 或 Sprint 2。

批准结论：通过。产品负责人于 2026-08-21 接受 S1-Q001～S1-Q004，Q004 按补充方案执行；仅授权开始 NG-101，NG-102～NG-107 仍未授权。

## 18. NG-101 实施与验证记录

执行日期：2026-08-21。状态：产品负责人已验收；NG-102 已授权，NG-103～NG-107 尚未授权。

### 已交付

- Rust 侧原生目录选择，绝对路径只保存在 ProjectRegistry，前端仅得到一次性 candidateId 和 sessionId；
- 创建/打开只登记 project_scan、project_create、project_open、project_close 四个 Story Command；
- v1 Scaffold、固定 project.json、旧 Markdown 项目元数据补充确认、损坏/不支持元数据拒绝；
- 创建前重新预检，所有新文件使用 create_new；故障时只反向清理本事务新建项，不递归删除项目根；
- 右栏提供目录选择、创建/打开计划、冲突清单、显式确认、项目会话与关闭入口；
- 文件浏览和正文保存能力显式保持关闭，未提前实施 NG-102/NG-103；
- 未引入网络、Provider、API Key、SQLite、Vault、Shell、Process、Git 或 Android。

### 自动化证据

- pnpm quality：10 个阶段全部通过；
- 前端：8 个 Test File、32 项测试通过；
- Rust：15 项测试通过，其中覆盖 Scaffold Golden、非空目录只读预检、同名竞态拒绝、中途失败回滚、有效项目打开、旧项目确认导入、损坏/不支持/敏感字段元数据拒绝；
- Capability 快照：仅 allow-health-check 与四个 NG-101 项目权限；
- Tauri Release 内部构建成功，Windows 隐藏窗口 Smoke 成功；
- 真实 Release 窗口视觉证据：target/qa/ng-101-tauri-window.png（1294×838 逻辑窗口，按 125% DPI 捕获为 1618×1048）。

### 验收边界

- 系统 UI Automation 未暴露 WebView 内按钮，因此没有使用坐标或键盘猜测自动操作原生目录对话框；
- 产品验收可现场点击“选择目录创建”和“打开现有项目”各一次；底层目录与事务行为已由 Windows 临时目录测试覆盖；
- Q004 在 NG-101 已验证新 Scaffold 为 UTF-8 无 BOM、LF；已有文件的 BOM/换行保留与非 UTF-8 只读行为由 NG-102/NG-103 按已批准补充方案实现。

## 19. NG-102 实施与验证记录

执行日期：2026-08-21。状态：产品负责人已验收；NG-103 已授权，NG-104～NG-107 尚未授权。

### 已交付

- Rust `workspace` 服务只索引项目根可见 `.md`、`characters/`、`materials/`、`macguffins/`、`plot/`、`outlines/`、`world/`、`style/`、`chapters/`，以及只读的 `.novelgainker/project.json`；
- 文件树和读取命令只接收不透明 `sessionId` 与项目相对路径，前端不持有项目根绝对路径；
- 拒绝绝对路径、空路径、`.`、`..`、反斜杠、盘符/冒号、隐藏路径组件、未知目录、`published/`、`.git/`、Symlink、Junction/Reparse Point 和根外 Canonical Path；
- 索引上限为 5000 个节点、16 层目录；Markdown 单文件读取上限为 4 MiB，受控元数据上限为 32 KiB；
- 只接受 UTF-8；读取时记录 SHA-256、修改时间、字节数、UTF-8 BOM 状态与 LF/CRLF/混合换行状态；编辑器正文不包含 BOM；非 UTF-8 返回稳定只读错误，不猜测转码；
- 右栏以真实项目文件树和直接 Markdown 源编辑器替换 Sprint 0 临时草稿；显示当前相对路径、已保存、未保存和只读状态；
- 编辑缓冲区只存在 React/Rust 当前进程内存，不进入 Local Storage、Session Storage、IndexedDB、日志或 AppError；右栏折叠只隐藏、不卸载组件；
- 切换文件、关闭项目和关闭窗口遇到 dirty buffer 时必须显式选择“继续编辑”或“放弃修改并继续”；窗口保护使用 Tauri close-request 事件；
- 保存按钮保持禁用，Ctrl+S 只显示 NG-103 边界提示；Capability、Command Manifest 和前端均不存在 `project_save_file`；
- Tauri Capability 仅增加 `project_list_files`、`project_read_file`、关闭事件 listen/unlisten 和主动 close 所需的最小权限；未引入通用 FS、网络、Provider、API Key、SQLite、Vault、Shell、Process、Git 或 Android。

### 自动化证据

- `pnpm quality` 全部阶段通过：Prettier、ESLint、TypeScript、React、NG-102 边界、Rustfmt、Clippy、Rust Test、Tauri Release 内部构建和 Windows Smoke；
- 前端：9 个 Test File、39 项测试通过；覆盖严格 IPC DTO、真实源缓冲区、dirty-state、文件切换、项目关闭、窗口关闭、只读元数据、受控读取错误、右栏折叠保持挂载和 Sprint 0 回归；
- Rust：21 项工作区测试通过，其中 `app-core` 18 项；覆盖白名单与排序、隐藏/`published` 排除、UTF-8 BOM、LF/CRLF/混合换行、非 UTF-8、4 MiB 超限、路径拒绝、受控元数据只读及 Windows 链接拒绝；
- 静态边界门通过：七个 App Command、精确 Capability 快照、只读 `workspace` 生产服务、无保存命令、无浏览器持久存储、无网络/密钥/进程能力；
- Tauri Release 内部构建成功：`target/release/novelgainker-desktop.exe`；Windows Smoke 成功创建真实主窗口后关闭，无遗留测试进程；
- 宽屏真实窗口视觉证据：`target/qa/ng-102-wide-window.png`（窗口逻辑矩形 1294×838，125% DPI 捕获 1618×1048），三栏、项目空态、直接源编辑器和禁用保存完整可见；
- 窄屏真实窗口视觉证据：`target/qa/ng-102-narrow-window.png`（窗口逻辑矩形 914×790，125% DPI 捕获 1142×988），中栏保持、右侧文件抽屉覆盖显示。

### 验收边界与后续

- NG-102 的“已保存”表示当前缓冲区等于已读取的磁盘版本；本 Story 不存在保存中或保存失败事务，因为所有写入入口仍被禁用；NG-103 激活原子保存后补齐这两个运行态；
- Q004 在本 Story 已完成读取侧检测：UTF-8 BOM/无 BOM 与换行风格进入 DTO，非 UTF-8 受控拒绝；真正写回时保留原编码标记和换行风格属于 NG-103；
- 原生目录选择器仍不使用坐标或键盘猜测自动化；产品验收可选择一个临时 Markdown 项目，现场检查文件树、手动编辑和放弃修改提示；底层读取矩阵已由真实 Windows 临时目录测试覆盖；
- 产品负责人已批准 NG-102 验收并单独授权 NG-103；NG-104～NG-107 未授权。

## 20. NG-103 实施与验证记录

执行日期：2026-08-21。状态：产品负责人已验收；NG-104 已授权，NG-105～NG-107 尚未授权。

### 已交付

- `app-core::workspace` 新增唯一受控保存入口：只接受已登记 `ProjectSession`、项目相对路径、UTF-8 正文和读取时 SHA-256；`.novelgainker/project.json` 继续只读；
- 保存路径复用白名单和逐组件 Reparse 检查，并补充 UNC、尾随空格/点、`CON`、`PRN`、`AUX`、`NUL`、`CLOCK$`、`COM1`～`COM9`、`LPT1`～`LPT9` 拒绝；
- 写入前校验源 Hash，临时文件完成同步后再次解析目标并复核 Hash；任何过期基线都返回 `file.source_changed`，不提供强制覆盖；
- 临时文件只在目标同目录以 `.目标名.novelgainker-{UUID}.tmp` 和 `create_new(true)` 创建；写入后执行 `flush()` 与 `sync_all()`；事务失败的 Drop Guard 只删除本次已验证临时文件；
- Windows 替换使用 `MoveFileExW` 的 `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`；不先删除、截断或重命名原文件，不开放通用复制、重命名、删除、Shell、CLI 或 Python；
- 保存成功返回新 SHA-256、修改时间、字节数、BOM、换行和 `diagnosticState: pending`；完整 Markdown/Frontmatter 诊断仍由 NG-105 实现，当前不做半套校验器；
- 已有 UTF-8 BOM/无 BOM 与标准 LF/CRLF 写回时保留；非 UTF-8 仍拒绝；历史混合换行文件因浏览器编辑值会归一化，首次保存明确统一为 LF；
- Tauri 只新增 `project_save_file`，App Command 总数为八个；Capability 不包含通用 FS；项目根绝对路径仍只在 Rust Registry 内；
- 保存按钮和 Ctrl+S 走同一事务；UI 展示保存中、已保存、保存失败和中文恢复建议；保存失败或 Hash 冲突时编辑器正文保持 dirty 且可继续复制/修改；
- 手动内容即使不符合未来严格 Markdown 契约也允许安全保存；UI 明确显示结构诊断待 NG-105 启用。

### 自动化证据

- `pnpm quality` 十个阶段全部通过：Prettier、ESLint、TypeScript、React、NG-103 边界、Rustfmt、Clippy、Rust Test、Tauri Release 内部构建和 Windows Smoke；
- 前端：9 个 Test File、43 项测试通过；覆盖严格保存 DTO、按钮、Ctrl+S、保存中防重入、成功后 Hash 基线、Hash 冲突、dirty 草稿保留和中文稳定错误；
- Rust：29 项工作区测试通过，其中 `app-core` 25 项、Tauri 3 项、Domain 1 项；
- Windows 文件系统证据覆盖：真实 Junction 越界、Symlink、UNC/盘符/设备名矩阵、UTF-8 BOM + CRLF、混合换行归一化、过期 Hash、写入/Flush/Replace 故障注入、目标文件占用导致的真实原子替换失败、UUID 临时文件及事务后零残留；
- Tauri 保存适配器测试从真实 ProjectRegistry 会话写入 Markdown，并同时断言磁盘正文与返回新 Hash；
- 静态边界门通过：八个精确 App Command、无绝对路径 DTO、无通用 FS、无递归删除/任意复制/目录创建、无浏览器持久草稿、无网络/Provider/API Key/SQLite/Vault/Process；
- Tauri Release 内部构建成功：`target/release/novelgainker-desktop.exe`；Windows Smoke 创建真实主窗口后安全关闭。

### 验收边界与后续

- 当前 Hash 校验覆盖“读取后到保存时已变化”的冲突；持续 Watcher、单 Writer Lock、跨窗口协调和崩溃恢复日志属于 NG-104，未提前实现；
- 当前只返回诊断已触发且待处理的稳定状态；完整结构诊断、搜索以及对未来 AI Apply/Final/EPUB 的阻断属于 NG-105 及后续 Story；
- 产品验收可打开临时项目，修改 Markdown 后分别用保存按钮和 Ctrl+S 落盘，再用外部编辑器制造 Hash 冲突，确认旧磁盘内容与内存草稿均不被静默覆盖；
- 产品负责人已批准 NG-103 验收并单独授权 NG-104；NG-105～NG-107 未授权。

## 21. NG-104 实施与验证记录

执行日期：2026-08-21。状态：产品负责人已验收；NG-105～NG-107 尚未授权。

### 已交付

- `app-core::coordination` 使用 `Local\NovelGainker.Project.{项目根哈希}` Windows Named Mutex 实现 OS 持有的单 Writer Lease；第二个进程或窗口只获得只读会话，不使用 PID、锁文件、陈旧锁猜测或锁转移；进程异常终止后由 Windows 自动释放 Mutex；
- `ProjectSession` 新增 `writer/readOnly`、恢复状态、恢复计数和可写 Capability；只读会话可以浏览和读取，但 Tauri 保存适配器会稳定返回 `project.writer_required`；
- 新增 `project_watch_file`，只接收不透明 Session ID、相对路径和预期 SHA-256；当前文件由前端每秒进行一次受控 Hash Watch，能够区分 `unchanged/modified/missing`，不依赖修改时间猜测；
- 外部变化后编辑器保持内存文本并阻断保存和 Ctrl+S；中文界面提供“重新载入磁盘版本”“保留内存文本”“复制内存文本”三项操作，不存在 force overwrite；
- `app-core::recovery` 在 Tauri App Data 下使用版本化 JSON journal；精确字段仅为 `schemaVersion`、事务 ID、项目 ID、相对路径、阶段、源/替换 Hash 和临时文件 ID，不保存正文、绝对路径、用户名、Prompt 或秘密；未知字段、超限、链接和越界状态均拒绝；
- 可恢复保存依次持久化 `prepared -> temporarySynced -> replacing -> committed`；启动或重新打开 Writer 会话时扫描未完成事务，按磁盘 Hash 确定提交完成、回滚/清理或 `required`；含糊证据保留且所有新保存返回 `project.recovery_required`；
- 恢复扫描只删除 journal 精确登记的项目同目录临时文件、精确命名的 journal 与验证过的孤立 journal-update 临时文件；不递归删除项目、不触碰未登记文件；
- App Command 白名单从八条增加为九条，仅新增 `project_watch_file`；仍无通用 FS、Shell、Process、网络、Provider、API Key、SQLite 或 Vault 能力；AI 与密钥继续保持未启用。

### 自动化证据

- `pnpm quality` 十个阶段全部通过：Prettier、ESLint、TypeScript、React、NG-104 边界、Rustfmt、Clippy、Rust Test、Tauri Release 内部构建和 Windows Smoke；
- 前端：9 个 Test File、50 项测试通过；覆盖右侧文件栏调宽、严格 Watcher DTO、外部修改三项恢复操作、冲突保存阻断、剪贴板复制、磁盘重载、第二实例只读、恢复歧义只读和三类稳定中文错误；
- Rust：工作区 38 项常规测试通过；另有 2 个 ignored 子进程夹具由父测试定向启动并实际执行；其中 `app-core` 33 项通过、Tauri 4 项、Domain 1 项；
- Windows 双进程证据：父进程持有 Named Mutex 时子进程确认只能只读；父进程释放后子进程取得 Mutex 并异常终止，随后父进程再次成功取得，证明不依赖 PID 清理；
- Crash 恢复证据：真实子进程创建并同步正文临时文件与 `temporarySynced` journal 后 `abort`；父进程重开扫描后保持原正文、删除精确临时文件并归类为 `recovered`；
- 恢复矩阵覆盖目标已替换后的提交完成、源文件未替换时的回滚清理、第三方内容造成的 `required`、未知 journal 字段拒绝、精确字段集合与敏感内容负面断言；
- Windows 外部编辑证据覆盖同一修改时间窗口内连续三次磁盘写入和文件删除，Watcher 始终以内容 Hash 正确识别；React 测试验证冲突出现后不能保存；
- 静态边界门通过：九条精确 App Command、OS Named Mutex、无 PID/lockfile、严格 journal Schema、App Data 根、无正文或绝对路径日志、无 force overwrite、无网络/Provider/API Key/SQLite/Vault/Process；
- Tauri Release 内部构建成功：`target/release/novelgainker-desktop.exe`；Windows Smoke 创建真实主窗口后安全关闭。

### 验收反馈修正：右侧文件栏可调宽

- 验收反馈指出右侧文件栏被固定在 350px，长文件名、文件树和 Markdown 编辑区域可视范围不足；宽屏布局现已改为内存态可调宽列，不写入浏览器存储；
- 用户可拖动右侧栏左边缘向左放大；最大宽度根据当前工作区、左栏和中间对话区最小宽度实时计算，窗口缩小时自动收敛，不把中栏挤出可用范围；
- 分隔条支持键盘：方向键调整、Home 恢复默认；双击也可恢复默认宽度；900px 及以下继续使用互斥抽屉，不显示拖动条；
- 新增 React 布局测试，覆盖方向键放大/缩小、Home 复位、真实 Pointer 拖动到 500px/600px、拖动状态和窄屏隐藏；前端总计更新为 50 项测试通过；
- 修正后重新执行 `pnpm quality`，十个阶段全部通过；前端 50 项测试、Rust 工作区测试、NG-104 静态边界、Release 内部构建与真实 Windows 主窗口 Smoke 均通过；
- 修正版已覆盖 `target/release/novelgainker-desktop.exe`，可直接进行产品验收。
### 验收边界与后续

- NG-104 的 Watcher 只监视当前已打开文件，并以一秒轮询调用受控 Hash 命令；递归项目监视与本地搜索属于 NG-105，不在本 Story 内；
- 第二实例固定为只读；关闭 Writer 后需要重新打开项目才能竞争新的 Lock，不实现运行中锁转移；
- `recoveryStatus: required` 只提供安全阻断和中文人工处理建议；本 Story 不猜测解决含糊手稿，也不提供绕过日志或 Hash 的按钮；
- 产品验收建议：打开同一项目两次确认第二实例只读；在 Writer 中修改未保存正文后用外部编辑器改动当前 Markdown，分别检查复制、保留、重载三项操作；
- NG-105 尚未授权；本次实施没有加入 Markdown/Frontmatter 校验、搜索、AI、Provider 或 API Key。
- 产品负责人于 2026-08-21 批准 NG-104 验收；NG-105～NG-107 仍未授权。

## 22. NG-105 实施与验证记录

执行日期：2026-08-21。状态：已实施并通过自动化验证，待产品负责人验收；NG-106～NG-107 尚未授权。

### 已冻结实现口径

- 新增独立 novelgainker-validation Rust Crate；除 Scaffold 约定文件 AGENTS.md 外，权威源 Markdown 要求安全 YAML Frontmatter，并包含非空 title 与固定 writingLanguage: zh-CN；
- chapters/ 文件额外要求且只允许一个严格的 ## Draft，该区段必须有非空正文；
- Frontmatter 第一版采用确定性安全子集：支持简单 key: value、两空格缩进映射和缩进列表；锚点、别名、标签、流式结构、多行标量、奇数缩进和超限层级返回稳定诊断，不猜测复杂 YAML 语义；
- Frontmatter 上限固定为 32 KiB、256 行、单行 1024 字节和 8 层；文件编辑上限继续为 4 MiB；
- NG-105 不创建持久化搜索索引。每次搜索从当前 Canon Markdown 受控重建/扫描，因此不存在缓存成为内容来源的问题，也不提前占用 NG-106 的 SQLite 范围；
- 搜索上限固定为 128 个查询字符、500 个文件、32 MiB 扫描量、100 个结果和每个结果 160 个正文字符（界面允许首尾省略号）；中文使用确定性字面匹配，英文可选择 ASCII 大小写敏感或不敏感；
- AGENTS.md 仍可被安全搜索，但不作为待出版源执行 Frontmatter 门禁；.novelgainker/project.json、.git、published、隐藏目录、链接、Reparse Point 和项目根外路径均不进入搜索正文。

### 已交付

- ValidationReport 包含相对路径、SHA-256、稳定 code、severity、行列、固定中文消息、恢复建议，以及未来 AI Apply、Final、EPUB 三项资格；
- 保存按钮和 Ctrl+S 先调用受控 project_validate_draft 生成只读提示；无论是否存在格式诊断，随后仍进入原有 Writer/Recovery/Hash/原子替换保存链；
- 原子替换成功后，Rust 对实际写入字节立即生成 diagnosticState: ready 报告；报告 Hash 必须与保存结果的新 Hash 完全一致；
- 用户继续修改草稿或 Watcher 发现外部变化后，旧报告明确显示“诊断已过期”，三项未来动作均暂停；再次保存后才恢复为新 Hash 的结果；
- 右栏新增结构诊断面板，可查看诊断字段、恢复建议和未来动作资格；点击诊断只选择当前编辑器对应行，不自动修复或写入；
- 新增 project_search，只接受不透明 Session ID、查询和大小写选项；Rust 复用受控文件树与读取服务进行每次全量扫描；
- 搜索结果只返回相对路径、行列和短片段；点击结果只读取并选择目标行，脏草稿仍经过既有放弃确认，不存在搜索写入、自动 Canon 化或 AI 发送；
- App Command 白名单从九条增加为十一条，仅新增 project_validate_draft 与 project_search；仍无通用 FS、Shell、Process、网络、Provider、API Key、SQLite 或 Vault 能力。

### 安全与恢复证据

- 查询、命中正文和草稿不写入 AppError、控制台、恢复 Journal 或本机状态；异常只映射为稳定代码和集中式简体中文恢复文案；
- 非 UTF-8 文件会被搜索安全跳过并计入文件/字节预算；跳过文件不能绕过 500 文件或 32 MiB 上限；
- 搜索每次直接从 Canon 重建；测试放入损坏的伪索引后搜索仍正常，删除伪索引后结果不变，随后修改 Canon 会在下一次搜索立即反映；
- 格式错误的章节仍完成原子保存，同时返回 frontmatter.missing 等稳定诊断并将三项未来动作设为不可用；
- 恶意 Frontmatter Fixture 覆盖超大单行/总量、超深缩进、锚点和非法结构；解析有固定循环与容量边界，不递归追随 YAML 引用；
- 静态边界验证十一条精确命令、查询脱敏、搜索无写入、诊断纯函数、无网络/SQLite/Provider/Key/Process，并继续保留 NG-101～NG-104 的路径、锁、Watcher 和恢复约束。

### 自动化证据

- pnpm quality 十个阶段全部通过：Prettier、ESLint、TypeScript、React、NG-105 边界、Rustfmt、Clippy、Rust Test、Tauri Release 内部构建和 Windows Smoke；
- 前端：12 个 Test File、59 项测试通过；覆盖严格诊断/搜索 DTO、保存 Hash 绑定、搜索只导航、未来动作资格、旧诊断过期、中文错误脱敏，以及 NG-101～NG-104 全部既有交互回归；
- Rust：46 项常规测试通过；另有 2 个 ignored 子进程夹具由 NG-104 父测试定向实际执行；其中 app-core 36 项、Tauri 4 项、Domain 1 项、Validation 5 项；
- Validator Golden/Fixture 覆盖有效中文源、有效章节、缺失/未闭合 Frontmatter、重复/缺失字段、错误写作语言、严格章节标题、空正文、非法 UTF-8、恶意大小/深度和危险 YAML 特性；
- 搜索 Fixture 覆盖中英文确定性匹配、大小写选项、相对路径/行列/片段、结果上限、隐藏和生成物排除、非 UTF-8 跳过、Canon 修改即时重建，以及损坏/删除伪索引；
- Tauri Release 内部构建成功：target/release/novelgainker-desktop.exe；Windows Smoke 创建真实主窗口后安全关闭。

### 产品验收建议

1. 新建或打开项目，选择一个合法章节并保存，确认显示“结构检查通过”和新 Hash 前八位；
2. 删除 ## Draft 或移除 Frontmatter 后保存，确认正文仍落盘，同时显示稳定中文诊断和三项“暂停”；
3. 在诊断后继续编辑，确认旧结果显示“诊断已过期”；再次保存后结果更新；
4. 用中文和不同大小写英文搜索，点击结果确认只跳转到文件与行，磁盘文件内容不变化；
5. 保持电脑断网且不配置 Key，重复搜索和保存，确认功能可用且中栏 AI 发送入口仍禁用。

### 验收边界与后续

- 本 Story 不实现 YAML 自动修复、后台保存、连续性语义审查、AI Apply、Final 或 EPUB 执行，只提供这些未来动作的资格标记；
- 本 Story 不保存搜索查询、诊断历史或持久化索引；最近项目、栏宽/折叠状态、当前文件与最小 SQLite 仍属于 NG-106；
- 产品负责人尚未批准 NG-105 验收；NG-106～NG-107 未授权，不在本次实施范围内。

## 23. NG-105 验收闭环与 NG-106 实施记录

执行日期：2026-08-22。状态：产品负责人授权 NG-106、NG-107 并要求完成 Sprint 1；该推进授权同时闭环 NG-105 产品验收。NG-106 已完成并纳入 G1 技术验收。

### 已交付

- app-core::app_state 使用 App Data 下固定文件 app-state-v1.sqlite3，Schema v1 只包含 ui_preferences、recent_projects 和 project_files；
- 保存左/右栏折叠、API 卡折叠、右栏宽度、最近 12 个项目和每项目当前相对文件；右栏宽度边界为 278～1600px；
- 最近项目绝对路径只存在 Rust 持有的本机数据库，前端重开只提交不透明 projectId，project.json 不增加机器路径；
- 启动时清理不存在、不可访问或链接/Reparse Point 的最近项目；损坏或更新版本的数据库降级为安全默认值，不阻断手动项目访问和手稿保存；
- SQLite 偏好事务与 Canon 保存事务完全分离；删除整个 App State 后，用户仍可重新选择、打开和读取项目；
- App Command 从十一条增加为十五条，只新增 app_state_load、app_state_save_ui、app_state_set_current_file 和 project_open_recent；
- 前端启动后恢复布局和最近项目；当前文件只以相对路径恢复。App State 保存失败显示中文提示，但不会覆盖编辑器状态或阻断手稿保存；
- 数据库不包含 Markdown 正文、查询、诊断历史、Provider、Key、Vault、对话、Token 或 Response。

### 自动化证据

- App State Rust 测试覆盖跨重启恢复、损坏/新版本降级、删除状态后 Canon 独立，以及精确 Schema、敏感字段和最近项目数量边界；
- Tauri 测试覆盖最近项目重开、当前相对文件恢复，以及偏好数据库故障不影响手稿保存；
- 前端测试覆盖严格 App State DTO、精确 Command 参数、错误归一化、布局恢复、损坏状态默认值、最近项目重开与当前文件恢复；
- pnpm quality:boundaries 验证十五条精确 Command/Capability、三张允许表、路径本机隔离和无 Provider/Key/Vault/正文状态。

## 24. NG-107 与 Sprint 1 Gate G1 技术验收记录

执行日期：2026-08-22。BMAD 数据包：review-gate-packet。结论：NG-107 完成，G1 所有技术条件通过；等待产品负责人批准进入 Sprint 2 Story Packet。

### 离线 E2E

- 新增 crates/app-core/tests/sprint1_offline_e2e.rs，只在已验证的系统临时根下创建唯一 Fixture，清理前再次核对规范化父目录和固定前缀；
- 从空目录完成扫描、创建、读取、手动编辑、无效 Markdown 原子保存与诊断、外部修改冲突、有效保存、本地搜索、关闭、重开和 Hash 一致性验证；
- 覆盖单 Writer、当前文件/布局恢复、删除 App State 后 Canon 仍可读，以及绝对路径、上级路径、.git、published、设备名和路径穿越攻击矩阵；
- pnpm e2e:sprint1 会移除 Provider/Key/Token 环境变量，启用 Cargo 离线模式，把代理指向不可用本机端口，并先执行静态无网络/无危险 Capability 检查；
- 同一入口定向执行异常进程遗留临时文件恢复与跨进程 Mutex 终止恢复测试；外部网络请求数为 0，Provider Key 数为 0，排队 AI 请求数为 0；
- 脱敏 Hash/检查证据写入 target/g1-evidence/offline-e2e.json，不包含正文、Fixture 路径、机器用户名或 Key。

### 统一质量门

- pnpm quality 的 13 个阶段全部通过：Prettier、ESLint、TypeScript、React、Sprint 1 边界、rustfmt、Clippy、Rust 全工作区、离线 E2E、Tauri Debug Build、Debug Smoke、Tauri Release Build、Release Smoke；
- 前端 13 个 Test File、65 项测试通过；
- Rust 53 项常规测试通过；另有 2 个 ignored 子进程 Fixture，由父测试定向启动并实际验证异常终止路径；
- Debug 与 Release novelgainker-desktop.exe 均在 15 秒内创建真实 Windows 主窗口并按精确 PID 关闭；
- Release 窗口证据为 target/g1-evidence/tauri-g1-window.png，1294×838、129299 bytes，采样包含 50 种颜色；SHA-256 为 d7b7f04b4e8f077401668a9dd60f158511f49dfd285e44ab5f5d87ad85b86c43；
- 质量门曾实际阻断 React Effect 同步状态更新、Debug Command 顺序断言和证据 Hash 工具兼容问题；修复后均从头重跑通过，没有跳过或降级检查。

### G1 结论与边界

- NG-101～NG-107 的 Definition of Done 和 G1 技术条件均已满足；
- Sprint 1 仍没有真实 API Key、Provider 网络请求、Vault、AI Apply、Workflow Pack、Python、Shell 或 Process；
- 本技术验收记录生成时 Sprint 2 尚未开始，也未创建 Sprint 2 Story Packet；
- 当时 G1 仅剩“产品负责人批准进入 Sprint 2 Story Packet”一项，必须由产品负责人明确批准，不能由实现者代签。

## 25. G1 产品批准与 Sprint 1 关闭记录

批准日期：2026-08-22。BMAD 数据包：review-gate-packet。

- 产品负责人明确回复“批准 G1 验收，创建 Sprint 2 Story Packet”；
- G1 最后一项产品 Gate 已勾选，Sprint 1 正式关闭；
- 已创建 [Sprint 2 Story Packet](./sprint-2-story-packet.zh-CN.md)，状态为等待 Story Packet Review Gate；
- 该批准只授权创建 Sprint 2 Packet，不等于授权 NG-201 或任何 Provider、Key、Vault 实现；
- Sprint 2 继续按 1 人、逐 Story 明确授权、严格顺序执行。

