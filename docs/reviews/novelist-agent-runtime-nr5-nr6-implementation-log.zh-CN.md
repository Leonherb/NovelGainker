# NR-5～NR-6 连续实施日志

授权：[实施批准](novelist-agent-runtime-nr5-nr6-implementation-approval.zh-CN.md)。历史NR-G5提交不改写；当前批准与技术进度分别记录。未执行真实Provider测试、多用户试用或版本分发。

## NR-501 — 章节合同与Gate分层（完成）

实现：`crates/app-core/src/chapter_contract.rs`及其测试；复用validation既有有界frontmatter解析；严格章节身份/编号/目标/候选/来源/回执/定稿合同。未注册章节生产写工具，尚不能据此声称应用可以保存章节。

合同决定：

- `contentHash`、`beforeHash`、`afterHash`均为磁盘实际UTF-8字节SHA-256，包含BOM、换行及`status`字段，不规范化后做CAS；确认记录位于非Canon状态，不进入自身Hash。
- 章节仅`chapters/`内扁平Markdown，非零规范UUID、正整数章号、严格七项frontmatter、唯一有效Draft内容；单章32KiB。全量身份/编号清单必须先检查重复。
- 候选绑定当前持久用户来源与工程/会话/轮次。仅明确写并保存或接受确切已展示候选进入可提交合同；讨论、试写、歧义及放弃不写入。语义由AI判断，Hash验证只证明来源，不以离线测试冒充真实语义正确率。
- 接受上一轮更新候选保留候选身份/字节与原目标Hash，但须当前轮重新完整读取目标；不能沿用旧读取凭证，不能因刷新凭证更换候选正文。
- 定稿必须有Host持久Committed回执和用户决定来源，并绑定确切revision/afterHash。无变化读取或保存可维持仍有效的确认；历史final无确认不能自行晋升。任何改字节、Undo或恢复未决使旧确认不可用；已失效标志重开或字节回滚不复活。实际事务内的失效落盘、手动保存/adoption由NR-502接入验证。
- 新阶段协议常量8Agent/33792输出、参数与结果各64KiB、Continuation512KiB、Body768KiB独立定义；NR-4运行常量不改。本项不宣称大参数已贯通生产解析器。
- 基础结构报告固定`eligibleForEpub=false`，前端拒绝仅凭格式合格的旧true；文案明确仍需确认与版本核对。出版就绪由NR-505专用预检判断。

Gate分层：`nr-stage-state.mjs`严格分块读取当前`current_gate`、`successor_packet`和历史`nr4_packet`，拒绝重复块、重复字段、嵌套冒充和必要批准缺失。历史脚本输出明确regression scope，NR-G5旧提交入口关闭。NR-501证据与完整quality改写到独立`target/qa/nr-501/`，不覆盖冻结提交Manifest。

最终验证：11项章节合同、58项授权负例通过；完整`pnpm quality`于2026-09-05T08:45:43.452Z通过26步，含113项前端测试、Rust全套、离线编辑回归、Debug/Release构建及桌面启动。此前发现本次测试格式、关闭旧提交后的unused变量和一处Clippy写法，均已修复后从头重跑，未豁免任何检查。

证据：`target/qa/nr-501/validation.json`（SHA256 `df714c03597e847c303b136806886cd42e684e8db69d8ff57f3784bb75c5f054`）、`target/qa/nr-501/quality.json`（SHA256 `84296222606edcf503a0da57bf4a49ff2ac23b0a410ebc14a28cb464ab1db215`）；通过时releaseScopeSha256=`cac2bab4940bfad00f275dbbe30b50140151cecb7b9bb5299fa6c8d510433949`。后续回归证据不得冒充此完成快照。

## NR-502 — 章节提交与编辑保护（完成）

NR-501完整质量通过后开始NR-502。已核对既有事务协调器、Kernel、读取证据和Diff/Undo接入点；本项未质量通过前不声称章节端到端交付。

实现及验证：

- 已实现独立 NR-5 Host 章节事务入口：Host 分配新身份、完整读取与当前来源校验、单轮一章、Dirty/Revision 保护、持久回执及同调用幂等、恢复和保留原字节的 Undo。手动章节保存使用本地事件来源，不伪造聊天消息；旧 final 改字后回到 draft。
- 已加入章节承载参数的独立双 Provider 协议 profile；旧 NR-4 限额仍独立保留。DeepSeek/OpenAI 的大参数、原生续传、SSE 和实际离线 loopback HTTP 请求体测试通过；未连接真实 Provider。
- 500 次真实章节落盘压力用例通过（1229.85 秒），验证 500 上限、501 次拒绝及最早 Undo 仍保留。这是容量/事务正确性测试，不是 NR-6 交互性能达标结论。
- 已接通原字节完整章节读取、v2闭合工具参数、已展示候选来源、分页保存记录、编辑器实际保存文本回读和显式候选放弃。已保存候选不继续冒充未保存候选；放弃只处理内部凭证，不改聊天或Canon。
- 两类Provider × 新建/更新 × 保存后回复成功/失败，共8条离线原生HTTP链路通过，实际序列化Body最高256016字节；涉及完整读取、双重转义大参数、真实落盘、任务对账、Diff与Undo。普通聊天与非法工具参数不抹掉文本另做新旧profile回归；未把合成响应解释成真实模型语义正确率。
- 工程限额：NR-5 Change最多2048条/总256MiB/单记录512KiB，待确认仍10条；候选最多64份/单份256KiB，聊天/模型最近目录最多8份；每页保存记录最多100条，游标绑定工程/会话/轮次/页大小/快照，拒绝过期续页。元数据满额测试与500次真实提交是不同证据，不互相替代。
- 桌面入口已挂载NR-5章节Coordinator和当前Host容量的任务恢复；仍仅2个工具、28条IPC。NR-4历史profile及冻结提交独立保留；内置Pack升级1.2.0（9能力/37资源）。

最终验证：完整`pnpm quality`于2026-09-05T11:30:24.817Z通过27步，含119项前端测试、224项app-core测试（4个ignored条目为父测试使用的子进程夹具）、其他workspace测试、16项NR-502定向用例、8条原生离线HTTP链路、Debug/Release构建和启动。500次实际提交及501次拒绝、最早Undo本轮再次通过，app-core整套耗时1164.67秒；不是交互性能达标声明。质量开始前修复Hook lint、人物夹具schemaVersion和测试格式，未豁免检查。

完成快照：`target/qa/nr-502/quality.json` SHA256 `356d8efe7f697fa795eb2845b4dc2bea6b10f1266446763b2942fd03d36595e1`；`target/qa/nr-502/validation.json` SHA256 `b1860b662de9252e176e745850977cdd12ab20bae3af0105d235b0e809ea7f4e`；releaseScopeSha256=`93f15494f7a9f41eda5a9ded9d2828832794b8c3bc3f964e8a85ebedea3f485b`。后续NR-502回归写入独立regression-validation.json，不覆盖此完成快照。

边界：原生HTTP合成模型验证真实Host/磁盘链路，React组件另有回归；不冒充真实模型语义验证或完整原生GUI点击链。多用户试用未执行，NR-G6/NR-G7未通过。

## NR-503 — 连续性读取与回写（完成）

NR-502完整质量通过后开始NR-503。已实现 Host 将实际完整章节读取绑定到当前持久提交回执，支持 AI 提交或手动有效保存的章节维护五类资料。继续使用既有两个工具和 28 条 IPC，不增加模型可伪造的章节凭证输入。

已补齐来源版本/Hash/作用域校验、提交各边界的来源复核、Undo 后依赖过期与重开不重放、六维带引用报告的机械校验。Pack 1.3.0 扩展人物/设定/素材方法的章节读取范围；来源过期警告与资料已保存事实分别显示，不自动连带撤销。模型自然语言报告仍依靠方法与读取证据，不声称机械合同能证明模型语义或拦截任意文本。

定向测试覆盖五类资料 × AI/手动章节来源的创建/更新/无变更/Undo，以及两类 Provider × 来源有效/外改 × 最终回复成功/失败的 8 条实际离线 HTTP 链；最大实际 Body 分别为 DeepSeek 15374 / OpenAI 14774 字节。完整 `pnpm quality` 于 2026-09-05T12:47:24.617Z 通过全部 28 步，含 121 项前端测试、232 项 app-core 测试（4 个 ignored 为父测试使用的子进程夹具）、其他 workspace 测试、8 项 NR-503 定向测试、Debug/Release 构建和启动。500 次真实提交、501 次拒绝及最早 Undo 再次通过，app-core 整套耗时 1155.83 秒，不作为 NR-6 性能达标声明。

完成快照：`target/qa/nr-503/quality.json` SHA256 `af4ece90d838542481f508b2688020f5df2091c3a0d9f4586c483f153c66a2c0`；`target/qa/nr-503/validation.json` SHA256 `752d9f55bc390cf91cca14b2039254f5e5cad87c616c5c412b7da8506da5918f`；releaseScopeSha256=`f3ba557fa975d0d2ab7b04469e6b66fcd4c6d9d88fec957bed7ec2f8193fe57f`。后续本项回归单独写 regression-validation.json。未运行真实 Provider、多用户试用或完整原生 GUI 点击链，不标记 NR-G6/NR-G7 通过。

## NR-504 — 非线性多任务编排（完成）

NR-503 完整质量通过后开始 NR-504。已扩展循环、预算和原生协议历史校验，支持有界任意串行顺序、4 读/2 变更/1 发布槽位、8 Agent + 1 Compactor / 33792 tokens；末轮无工具。生产仍仅两个工具，发布执行器待 NR-505，测试最大链的发布槽位是 cfg(test) 专用夹具，不能算作 EPUB 实现。

定向验证：两 Provider × 两顺序 × 四种终态（双成功、首任务待确认、首任务失败、保存后最终流中断）共 16 条生产入口原生 HTTP 链通过；2 条最大 8+1 核心链验证真实摘要落盘并进入请求，实际 Body 最大 67729 字节；七组原生配对、错配/重复/超额反例通过。双重转义的原生序列化请求约 723 KB，单独可发送，但完整返回预留后约 856 KB 超限，真实写执行器未执行、Task 失败且零资料文件；小请求对照真实提交成功。此预留用例是实际序列化 + Host/磁盘验证，不冒充额外 HTTP 捕获。

完整 `pnpm quality` 于 2026-09-05T13:37:30.105Z 通过 29 步，含 121 项前端、237 项 app-core 测试（4 个 ignored 为父测试使用的子进程夹具）、其他 workspace 测试、5 项 NR-504 定向测试、Debug/Release 构建和启动。500 次实际提交/501 次拒绝/最早 Undo 通过，app-core 耗时 1192.32 秒；不声称交互性能达标。

完成快照：`target/qa/nr-504/quality.json` SHA256 `2d906ac8b22987e5569bc57b18a4afea4627f6ed2d188b22b8cc48dd3d2f4f12`；`target/qa/nr-504/validation.json` SHA256 `87d5d82abc25085d3a13744ca7ecec091181546a5ab8b63e201fc33dd9940762`；releaseScopeSha256=`613634b416667d3bb3b2d2adc05b62aa33b221bfcd1949dede1ffbcc1383f319`。后续回归写入独立 regression-validation.json。仍无真实 Provider、多用户或 NR-G6/NR-G7 通过声明。

## NR-505 — 本地出版与独立评审（完成）

NR-504 完整质量通过后开始 NR-505。已挂载纯本地 Rust EPUB 预检/构建、novelist.publish、publication_list/detail 和聊天内渐进展开回执。当前 30 IPC，Pack 1.4.0 / 9 能力 / 37 资源；使用现有 project.md 的 authorDisplayName 等字段，只由当前有效定稿版本形成出版输入，不开放根目录 AI 写入。

当前定向证据：15 项 Rust 测试通过，包括 8 条双 Provider 常规实际原生链及 2 条 8 Agent + 1 Compactor 的真实出版最大链；后者不是 NR-504 的 publication stand-in。独立 .NET ZIP/XML 检查通过，新增前端覆盖合并后共 129 项测试通过。持久记录必填 nullable、输入总量/依据、256 条/13 页、1 GiB 含孤立暂存配额、不覆盖旧产物、外改 stale、恢复不重放均有反例。未执行真实 Provider、多用户或完整 EPUB 第三方认证。

完整 quality 在 2026-09-05T15:37:13.388Z～2026-09-05T16:01:43.604Z 通过全部 30 步：129 项前端、252 项 app-core（4 个 ignored 为父测试调用的子进程夹具）、其余 workspace、15 项本 Story 定向测试、10 条实际出版原生 loopback、独立 .NET ZIP/XML、Debug/Release 构建及启动。500 次真实章节提交/501 次拒绝/最早 Undo 再次通过，app-core 整套耗时 1253.35 秒，不是交互性能达标声明。

完成快照：target/qa/nr-505/quality.json SHA256 856b2d077986d10f7b1e1fda04c59f43f3cd6ba28a58a1a4b62d47ee384fdd1a；validation.json SHA256 0ecf2048929a14871377da3b467ec553a50aebdabec66eb1ec4198cb61b73ea2；二者 releaseScopeSha256 均为 8f3d1b92125a6882bc8abfc85606cb257ce61d4127026a5b92ea5acc78586573。完成时合成 EPUB 副本 completed-structure-fixture.epub SHA256 63151feddff7e41c72fd0a9d0d55a419bac398d638f65e5af09cccb8e61df8df；后续回归只写独立 regression-validation.json。

质量前修正新测试终态枚举、脚本子进程哈希依赖及旧双工具数量假设；原双工具断言保留并分别检查唯一新增出版工具。首次完整 quality 因 Hook 名称命中历史 History 面板禁用规则停下，改为准确的出版记录名称后从头重跑通过；不放宽规则、不改写冻结 NR-504/G5 证据。工程约束见 novelist-agent-runtime-nr505-engineering-contract.zh-CN.md。

## NR-506（完成）

NR-505 全质量通过后进入 12 章以上应用级闭环及 NR-G6 技术证据。G6 技术尚未通过；沿用已批准的技术通过后自动进入 NR-6 方式，不提前通过 NR-G7，不运行真实 Provider/多用户或分发版本。

NR-506 定向进展：实际桌面 command 源码通过 cfg(test) 桥接进入 Tauri MockRuntime，调用创建工程/凭据配置状态/聊天启动/事件/轮询/资料回执/出版/重开/Undo 的原始 IPC handlers 和真实后台 worker。仅替代窗口、文件选择结果、合成 vault 与 localhost HTTP，不增加生产 endpoint 或通用写入通道。双 Provider 格式完成讨论→人物/设定→十二章写入并定稿→章节来源素材→显式评审→EPUB→重开→Undo/stale，回执与字节一致；非原生 WebView 点击链，也非真实模型语义验证。

回归暴露并修复 Windows Task 轮询读取与原子替换竞争（OS 5），添加工程内弱引用锁协调完整 Task 读取/更新，不跨 Provider 或工具执行；高频快照与 100 项三状态更新、双协议十二章连续四轮复跑通过。另修正夹具跨 Turn 重复工具 ID 和 DTO/闭合风险枚举，保留生产防重复校验。补齐双协议零文本/部分文本/工具参数失败/待确认放弃/已保存后最终 503 的实际 IPC 故障矩阵。临时数字错误诊断已移除。

完整 quality 于 2026-09-05T16:58:50.758Z～2026-09-05T17:21:58.899Z 通过 31 步：132 项前端、257 项 app-core（4 个 ignored 为父测试调用的夹具）、其他 workspace、5 项 NR-506 定向、Debug/Release 构建与启动全部通过。500 次实际提交/501 次拒绝/最早 Undo 通过，app-core 1139.91 秒，不声称 NR-6 性能合格。首次完整运行因旧 NR-006 静态规则漏认 Runtime 泛型停止；精确认别后额外保持唯一声明/唯一调用检查，从头重跑通过。

冻结 quality SHA256 098ca628049e23eb684692bb42968f7489f28fd4cfc3a90f89ae1a0044683541；validation SHA256 0518b7b083f5a5f3303da08f554e0e76b31b6ca2750e69b06bf564382730c264；共同 releaseScopeSha256 bfdc16cee6bafe591ae9cba81e139bdf965be970dc04521ea115c076eb0cdaf6。NR-G6 报告 SHA256 bdf59cb8982cc4f783b5e569133265dce7a189153d9de49eda7d1f997364e1d8，Manifest SHA256 166e2ef11c69230329ee3eba07d478098cfd40adb1db3cb3f15ddf24b83325d1。初次 nrG6Approved=false，技术通过后依用户已批准的连续方式进入 NR-601；不冒充新的人工 Gate 决定。后续回归写独立 regression-validation.json。

## NR-601（完成）

开始会话目录与 History/Memory/Task 分域。旧默认会话身份/路径、共享 Canon、单活动 Turn 和跨会话事件隔离必须一起验证。真实多用户测试仍未执行，未分发，最终 G7 未通过。

NR-601 定向进展：34 项严格闭合 IPC，默认路径/身份增量登记、32 非归档/256 总目录、三类总容量与生成预留、原子目录/CAS/幂等、逐层 reparse 拒绝均已实现。14 项 Rust 专项及 139 项前端测试通过；双原生 loopback 在运行中切换后仍将 History/Task/事件/修改记录绑定原非默认会话，原会话 Diff/Undo 可验证。补齐同工程重开别名不得误恢复活动轮次、重复生成拒绝、跨工程停止拒绝；不增加真实 Provider 调用。

完整 quality 尚未通过：第一次因旧 NR-005 静态规则只认可 App 直接挂载 ConversationView 停止，已增加受 NR-G6 授权约束的包装层及 keyed 原组件检查；第二次独立测试文件中的合成工具名触发 NR-206 发布扫描，夹具改为复用实际只读工具定义，扫描规则保持不变。第三次在长回归期间发现同工程重开别名风险后主动停止，修复并专项验证后将从头重跑。以上不构成 Story 完成或 Gate 通过。

最终完整 quality 于 2026-09-05T18:45:39.182Z～19:08:43.466Z 通过全部 32 步。139 项前端、271 项 app-core（4 个 ignored 为父测试使用的子进程夹具）、其余 workspace、14 项 NR-601 定向、44 项授权负例、Debug/Release 构建与启动均通过。500 次实际提交/501 次拒绝/最早 Undo 通过，app-core 1140.07 秒，不作为 NR-6 性能达标结论。

冻结 target/qa/nr-601/quality.json SHA256 9f617a5518518de20ca66d87597ddde50f31aa1c1a302668c8cb73cc8be1d773；validation.json SHA256 fb22fd0dd3455a2ed56252ad626ceb55881ddad525e6bcc1da0376a75e831c3e；releaseScopeSha256 均为 e2a16e5275d2155de4464ac63cc6e534d25d5a0a571d8099003e577e3883eba9。后续只写 regression-validation.json。NR-G6 冻结报告与 Manifest 哈希未变；未声称真实 Provider、干净 Windows 安装、多用户或 NR-G7 通过。

## NR-602（完成）

NR-601 全质量通过后进入闭合结构操作及显式本地导出。继续保留原消息身份、来源回执和共享 Canon；不自动重放 Provider，不永久删历史。

NR-602 定向进展：37 项 IPC 五层严格闭合，新增 branch/clear/archive/restore 封闭命令与两步原生保存对话框/一次性 token 导出。清空的源会话与请求在确认丢失后仍固定，32 满额原子替换、256 总目录、准备孤儿精确复用/篡改拒绝、归档只读/恢复、独立实例禁止恢复其他 writer、worker 生命周期持有 lease，以及 Markdown/JSON 可见字段导出已实现。导出禁止工程内/重解析/已有文件/过期来源，准备与提交共用单份有界槽，未提交凭证关闭失效，不暴露绝对路径或替换字节。

14 项 Rust/实际 IPC 定向及 34 项新阶段授权负例通过；7 项会话界面测试（4 项新增）、3 项新客户端严格 DTO 测试通过。全量质量尚未完成，不进入 NR-603。保存对话框选择由夹具代替；独立 registry + 真实 writer lease 不是两套已安装应用的原生点击证据。现有 NR-601 14 项回归仍通过，原冻结证据不变。

第一次全质量已通过 146 项前端及前段 Gate 回归，在长 Rust 回归期间主动停止：复核发现清空成功但确认丢失后，原会话条目不在当前非归档页，界面暂时把未知状态当可发送。Host 原已拒绝归档写入；现补充原会话待核对选项、保守只读提示及精确 UI 断言。另明确本地导出在任何文件系统读取前拒绝 UNC/设备命名空间，并验证提交中的单份容量占用不能由过期绕过。将从头重跑完整质量，不计首次为通过。

最终完整 quality 于 2026-09-05T20:17:32.810Z～20:40:39.555Z 通过全部 33 步：146 项前端、285 项 app-core（4 个 ignored 为子进程夹具）、其他 workspace、14 项 NR-602 定向、34 项新授权负例、Debug/Release 构建及启动均通过。500 次真实章节提交/501 次拒绝/最早 Undo 通过，app-core 耗时 1142.80 秒，不代表交互性能验收。

冻结 target/qa/nr-602/quality.json SHA256 ea1421aa835117bc8ea9fb8aa8c1877f24fe24c03fe562d8b31c096b38b75001；validation.json SHA256 5b510cf6f0406db3a86435ac3562aa609df5a43f9fc498e3ef164c0b0e2b7f06；共同 releaseScopeSha256 4eef862248334e3c09b321e011f467b75ed38f5578d0b3d0d83b34984996a9d1。后续仅写 regression-validation.json。未执行真实 Provider、多用户、原生保存对话框点击或干净安装；不提前通过 NR-G7。

## NR-603（完成）

NR-602 全量通过后开始保守显式重试、持久 attempt 身份和支持矩阵。默认仍无自动 Provider 重试，事件补拉不发模型请求，失败请求占原 Turn 预算；工具/可见正文/结果不明的轮次不重放。真实原生 Cursor 未验证，保持 unsupported。

NR-603 定向进展：38 项 IPC 五层闭合；原用户消息/Turn 不重复、一次 CAS 重试、持久 8 Agent + 1 Compactor / 33792 预算、跨尝试压缩跳过、工具派发前记录、重开未知不重放、原 Provider/model 配置核对、旧尝试事件/倒退快照拒绝及同一运行锁内的取消身份校验已实现。9 项 Rust/实际双原生 loopback 定向与 34 项新授权负例通过，NR-601/602 的 14+14 项回归通过。NR-005 的 45 项 UI/生命周期测试、NR-006 的 108 发布文件/38 当前命令闭包、TypeScript/ESLint/Clippy 通过；全量质量仍须完成。首轮前端回归因旧停止断言缺少新 attempt 参数失败，已同步断言，不弱化 DTO。

完整 quality 于 2026-09-05T21:13:05.567Z～21:36:46.420Z 通过全部 34 步：151 项前端、294 项 app-core（4 个 ignored 为父测试使用的子进程夹具）、其余 workspace、9 项 NR-603 定向、34 项新授权负例、Debug/Release 构建及启动通过。500 次实际提交/501 次拒绝/最早 Undo 通过，app-core 1151.67 秒，不作为 NR-605 性能结论。

冻结 target/qa/nr-603/quality.json SHA256 5ed92d6df85204412a92b98d7c087ea6ee6563842f58502f3f6d095460ffe42c；validation.json SHA256 6234cc4780eb1eb94dd3c1087ccdfaf7578ed6a93a0d47aa7c36f1f0cde3163d；共同 releaseScopeSha256 1f3539bff3ee95d654cf6864630344802b3260fbe9f2573c11c7d20fbe38e8f0。完成时 Debug EXE SHA256 479742944bc8f0b607e3d3dec152c929740c310be17e94e135fe18694235cf6c，Release EXE SHA256 0ec7b479c90bfc7951891a7a9b6e800ee50a4f2792db8b20c8a52f4b4f507b02。后续只写 regression-validation / regression-quality，不覆盖完成证据。

## NR-604 入口（批次未确认） / NR-605～606（未实施）

累计完成 9/12。依 Story Packet 的 NR-604 进入条件，NR-603 质量前置已满足，但产品负责人试用批次尚未确认、参与者/Provider 分工与干净 Windows 环境未提供。已只读核对后续最小 Host 回执、合成工程、旧版本生成接口及发放要求，尚未实施 NR-604 工具详情/测试包/安装升级验证，不把程序准备或离线结果当作真实用户通过。详见 nr604-entry-readiness 检查单。连续实施授权仍有效，不新增中间产品 Gate；收到批次安排后按原范围推进。没有分发、真实 Key 调用或最终 NR-G7 批准。
