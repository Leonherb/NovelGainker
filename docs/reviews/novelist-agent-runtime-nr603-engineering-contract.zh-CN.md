# NR-603 工程合同：保守显式重试与状态对账

状态：完成。完整 34 步 quality 于 2026-09-05T21:36:46.420Z 通过，冻结哈希见实施日志。前置 NR-602 于 2026-09-05T20:40:39.555Z 通过 33 步；连续实施授权来自现有批准文档。本合同不授权真实 Provider、分发或最终 NR-G7。

## 支持矩阵

| 行为 | 本应用支持状态 | 明确边界 |
| --- | --- | --- |
| UI/Event 对账 | 支持，本地 Store 为终态真值 | 不请求 Provider；按 conversation/turn/attempt/sequence 去重，旧尝试和倒退快照拒绝 |
| 用户手动重试 | 有条件支持一次 | 新物理请求、同一用户消息/Turn；不是网络断点续传 |
| OpenAI 服务端 Cursor 恢复 | unsupported / 未实测 | 当前同步 Responses 请求不建立 background 流恢复合同 |
| DeepSeek 服务端 Cursor 恢复 | unsupported / 未实测 | 当前 chat/completions 适配器未实现有证据支持的流恢复接口 |
| 自动重试 / 工具后整轮重放 | 不支持 | 不借用连接检查 MAX_RETRIES，不增加调用额度 |

官方依据：OpenAI 的后台流文档描述 response ID 与 starting_after/sequence 的恢复条件；这不是当前同步调用的自动能力。只据当前适配器标 unsupported，不断言所有 OpenAI 协议都不能恢复。[OpenAI Background mode](https://developers.openai.com/api/docs/guides/background)。DeepSeek 多轮 Chat 文档要求每次提交消息上下文，当前 SSE 返回结束标记；未据这些文档推导出服务端 Cursor 续流支持。[DeepSeek 多轮对话](https://api-docs.deepseek.com/guides/multi_round_chat/)、[Chat Completion API](https://api-docs.deepseek.com/api/create-chat-completion/)。没有新增 background/服务端存储或 opaque continuation 持久化。

## 冻结数据与执行边界

- ConversationTurn 增加可缺省但严格封闭的 retryState：attemptId(UUID)、attemptNumber(1～2)、agentRequests(0～8)、compactorRequests(0～1)、requestedOutputTokens(累计≤33792)、toolDispatched(boolean)、outcome(running/eligible/blocked)。固定七字段，序列化后计入配额，仍受原 16 MiB History、10,000 消息及聚合配额约束；不另开数据库。缺省为旧/不明记录，不赋重试资格。分支复制清除该字段。
- 所有生产启动先持久化用户消息/running，再初始化身份，绑定物理项目运行锁，最后启动 worker。每次 Agent/Compactor 在 HTTP 前持久计数，失败不返还预算；无法记账则不请求。跨手动重试只有一个 Compactor，已经用过即跳过；原八 Agent 与工具后续空间不重置。
- 仅最新 Turn、首次尝试、已知失败终态、零输出字符（空白也不算零）、未派发任何工具、剩余预算足够，且错误为 timeout/unreachable/rate_limited，才 eligible。这表示 Host 能证明没有应用工具副作用，不表示远端模型没有计费。协议不明、鉴权失败、取消、重开恢复的 running、已经派发/提交的结果都 blocked。
- begin_retry 在唯一运行预留内以 expectedAttemptId 比较；先持久化 attempt 2 再请求。只移除已核实的 Host 零正文占位，不新增/改写用户消息，不复制 Task/Change。原 provider/model 必须仍有匹配配置，不换成当前选择的另一个模型。重复/错身份/旧轮次/归档/只读/活动运行均拒绝。
- 恰好增加 conversation_turn_retry，当前总 38 IPC；request 仅 projectSessionId/conversationSessionId/turnId/expectedAttemptId，deny_unknown_fields。五层闭合须同时检查前置 NR-602 冻结 quality/validation 哈希及两份状态的当前授权。未接收 Provider Cursor/原始请求/路径/替换字节。
- start/event 返回 Host attemptId/attemptNumber；同尝试按单调 sequence 去重，终态后迟到文本拒绝。已知 attempt 2 不接受 attempt 1 快照；新尝试事件先于确认时待 Store 对账。停止携带会话/attempt 身份并在活动运行锁内比较；旧接口未携带身份最多停止第一次尝试，不能停止重试。关闭应用仍取消当前工作，不增加关闭等待。
- UI 保留普通聊天形态，显示“用原消息重试一次”的含义和限制；提交中禁止重复点击。确认丢失只补拉 Store，不重发。不可重放时可把原问题放入草稿，用户修改/发送形成新 Turn，不自动发送或覆盖已有草稿。

## 验证及未验证

定向覆盖持久 CAS/累计预算/重开未知/严格字段、双原生 loopback 零正文失败→显式重试、部分正文与已保存后失败不可重放、旧停止身份隔离、实际 Compactor 不重复，以及 UI 确认丢失/草稿保留/事件去重。测试全部是合成工程/隔离 vault/127.0.0.1，不能声称真实 Provider 兼容或原生 UI 点击已验证。独立人工评审未发生，工程复核为 implementer self_review。

真实多用户、干净 Windows 安装、旧版本工程升级、长篇语义和最终 build 复测仍属后续 Story；未取证不得 completed。原 Gate/Manifest 保持冻结；完整 pnpm quality 必须通过后才能进入 NR-604。
