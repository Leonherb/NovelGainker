# NR-601 会话分域工程合同（完整 quality 已通过）

本项沿已批准连续实施进入；NR-G6 技术证据已冻结，最终 G7 未批准。本文件记录用户委托范围内参数，不授权真实 Provider、对外分发或云同步。

- 每工程最多 32 个非归档会话，含归档最多 256 项目录，目录 JSON 最多 1 MiB；名称 60 个字符 / 180 UTF-8 字节，禁止控制字符。分页 32 项，游标绑定工程、状态和目录快照。
- 每会话沿用 History 16 MiB / 10000 消息、Task 4 MiB / 10000 项、Memory 32 KiB。整个工程含归档的 History 512 MiB、Task 128 MiB、Memory 8 MiB；临时/孤立文件同样计入容量检查，不自动驱逐记录。三个有界状态目录各最多 1024 个条目（含子目录），写入前额外预留临时条目；容量阈值针对持久内容及已存在的暂存文件，原子替换期间还需要操作系统提供暂存空间，磁盘不足时保留旧文件而不承诺写入成功。超限拒绝需新增资源的操作，既有安全读取不因此关闭。
- 开始生成前持久预留当前会话剩余 History/Task/Memory 上限，并绑定 Host 生成的 reservationId、conversationSessionId 和后续 Turn ID；一个工程仅一项生成预留。其他会话的新增操作不得占用这份预留。结束后精确释放；重开只对账和终止中断，不重放 Provider 或工具。容量仍受既有单条文本/序列化限制，不承诺无限生成。
- 旧默认 conversation/session.json、memory/state.json、tasks/session.json 和原始会话 ID 保持；仅目录增量登记。新增会话才使用各目录 sessions/UUID.json，不复制、删除或重编码旧历史。损坏目录不能悄悄重建身份；不存在的已登记新会话不能伪装成空会话。
- 目录和容量变更受工程内短时锁及 CAS；创建采用稳定请求 ID 防重复。会话文件先准备，再原子提交目录；未提交文件保持孤立且占容量，不先删除旧数据。路径来自 Host UUID 和闭合存储类别，逐层拒绝链接/reparse/越界。
- 后台 worker 捕获不可变会话绑定；History/Memory/Task 和变更来源均用该绑定，不能随当前视图切换。共享 Canon、编辑器及写入协调保持工程级；事件按工程会话、Conversation Session、Turn 关联。
- 活动生成、停止和恢复互斥以物理工程 coordination key 匹配，而非仅以一次打开产生的 projectSessionId 匹配；同工程重复打开不能将仍活动的原轮次误作崩溃恢复，其他工程不能借 Turn ID 停止本轮。事件仍保留其实际投递作用域。
- 只增加 conversation_session_list/create/rename/activate 四项封闭 IPC，当前阶段 34 项；NR-602 的 branch/clear/export 尚未挂载。会话选择在现有聊天区域，不增加独立 History/Task 中心；切换不调用 Provider，不重放历史。
- 验收必须覆盖身份/路径保留、跨域拒绝、CAS/重复操作、运行中切换、两 Provider 合成链、容量/损坏/故障原子性、UI 操作及完整 quality。本文件不是这些验证已经通过的声明。

完成证据：2026-09-05T19:08:43.466Z 完整 quality 32 步通过，139 项前端、271 项 app-core、14 项定向、44 项授权负例及 Debug/Release 构建和启动通过。quality SHA256 9f617a5518518de20ca66d87597ddde50f31aa1c1a302668c8cb73cc8be1d773；validation SHA256 fb22fd0dd3455a2ed56252ad626ceb55881ddad525e6bcc1da0376a75e831c3e；共同源码范围 e2a16e5275d2155de4464ac63cc6e534d25d5a0a571d8099003e577e3883eba9。原生 GUI 点击、干净 Windows 安装及真实 Provider/多用户不在这些通过声明内。
