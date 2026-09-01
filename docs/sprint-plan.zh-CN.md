# NovelGainker NR-1 交付状态

更新时间：2026-09-01

## 当前结论

NR-101 → NR-106 与 NR-G2-R01 → NR-G2-R03 已按批准顺序完成。产品负责人已明确批准 NR-G2 技术提交并确认 Gate 通过，当前停止于 NR-G2：

- `NR-G2.status = passed`
- `NR-G2.reviewStatus = approved`
- `nrG2Approved = true`
- `remediationImplementationAuthorized = true`
- `realProviderLiveSmokeExecuted = false`
- `nr2Authorized = false`

当前不执行真实 Provider Live Smoke，不形成或进入 NR-2；后续动作等待产品负责人另行授权。

## NR-G2 Remediation Draft 2

| Story     | 状态      | 关闭目标                                             |
| --------- | --------- | ---------------------------------------------------- |
| NR-G2-R01 | completed | 请求级 System 不变量、双 Provider 四条序列化路径保真 |
| NR-G2-R02 | completed | 仅初始无有效 Memory 可重建；有效 Memory 禁止降级替换 |
| NR-G2-R03 | completed | Prompt/行语法对齐；顶层结构拒绝且正文不做关键词扫描  |

Packet：`docs/stories/novelist-agent-runtime-nr-g2-remediation-story-packet-draft-2.zh-CN.md`。

## 已完成 Story

| Story  | 状态      | 核心交付                                                                       |
| ------ | --------- | ------------------------------------------------------------------------------ |
| NR-101 | completed | 完整 History 事实源、工程绑定 Memory Store、Segment Cursor、源 Hash 与原子替换 |
| NR-102 | completed | 字节阈值、近期优先重建、连续覆盖、前向衔接和渐进 Backfill                      |
| NR-103 | completed | 当前 Provider/CredentialVault 下的纯文本 Compactor；可取消、单 attempt、零重试 |
| NR-104 | completed | 固定优先级 Context Assembler；压缩失败回退；当前用户消息一次且仅一次           |
| NR-105 | completed | 重开、Provider 切换、停止与关窗恢复；不新增 History/Memory UI                  |
| NR-106 | completed | 候选 Profile、10,000 消息与长消息规模证据、预算冻结、NR-G2 Manifest/Report     |

## 冻结 Profile

| 参数                        |           值 |
| --------------------------- | -----------: |
| Trigger                     |       64 KiB |
| Protected exact tail        |       24 KiB |
| Segment content             |        6 KiB |
| Compactor input             |       72 KiB |
| Backfill reserve            |        8 KiB |
| Memory summary              |       12 KiB |
| Compactor requested output  | 1,024 tokens |
| Main reply requested output | 4,096 tokens |
| Cumulative requested output | 5,120 tokens |

每次用户发送在阈值以下最多 1 次真实 HTTP attempt；触发压缩时最多 2 次。Compactor 与主回复均可取消且零重试。Skill、Tool 与 Canon 操作仍为 0。

## 产品边界

NR-1 只实现完整可见 History、派生 Memory、阈值压缩和长对话 Context。它不加载 Novelist Skill、不读取 Canon 作为模型上下文、不调用 Tool，也不由 AI 创建或更新小说 Markdown。Memory 不是 Canon，也不在中栏生成独立气泡或控制入口。

## Gate 证据

- Packet：`docs/stories/novelist-agent-runtime-nr1-history-memory-story-packet-draft-2-final.zh-CN.md`
- 技术报告：`docs/reviews/novelist-agent-runtime-nr-g2-technical-submission.zh-CN.md`
- 机器 Manifest：`target/qa/nr-106/nr-g2-manifest.json`
- 最终验收入口：`pnpm quality`

NR-G1 与 NR-G2 的历史 Manifest/机器声明保持生成时事实不变；提交后的产品批准记录在各自 Report Gate Decision 与 BMAD 当前状态中。
