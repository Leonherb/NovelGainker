# NovelGainker NR-1 规划状态

更新时间：2026-09-01

## 当前结论

NR-001 → NR-006 与 NR-G1-R01 → NR-G1-R03 已完成，NR-G1 技术提交已获产品批准。

当前保持：

- `NR-G1.status = passed`
- `nrG1Approved = true`
- 已形成 `NR-1 Story Packet Draft 1`
- `nr1ImplementationAuthorized = false`
- 未执行真实 Provider Live Smoke

NR-G1 技术 Manifest 与报告中的机器声明继续保留“技术提交生成时”的冻结状态，不重写历史 Evidence；当前产品批准记录以 BMAD 状态和报告中的 Gate Decision 为准。

## 已完成基线

| Story          | 状态      | 核心交付                                                                                        |
| -------------- | --------- | ----------------------------------------------------------------------------------------------- |
| NR-001～NR-006 | completed | 自然对话基线、Conversation Store、单次 Provider Turn、三栏 UI、关闭生命周期与首次技术提交       |
| NR-G1-R01      | completed | Event 快速通知 + Store 最终事实；listener readiness；活动 Turn 1000ms 自动对账与同步失败恢复    |
| NR-G1-R02      | completed | Project Metadata `schemaVersion 1 -> 2` 一次性 Clean Slate；保留 Canon、凭据、UI 和无关项目状态 |
| NR-G1-R03      | completed | 递归发布面、五层 24/3 IPC 闭包、上游 Evidence 派生、违规生产文件与 Report 漂移负向 Gate         |

## NR-1 Draft 1

Packet：`docs/stories/novelist-agent-runtime-nr1-history-memory-story-packet-draft-1.zh-CN.md`

提议顺序：

1. NR-101 — History / Memory 合同与派生 Store
2. NR-102 — 增量覆盖与阈值判断
3. NR-103 — 有界 Compactor Provider 模式
4. NR-104 — Context Assembler 与主回复回退
5. NR-105 — 生命周期与无新增 History UI
6. NR-106 — 规模测试、预算冻结与 NR-G2 技术提交

本 Draft 只覆盖完整可见 History、Host 增量 Memory、阈值压缩和长对话 Context。Novelist Skill、Agent Tool Loop、Canon 读取或 Markdown 自动维护均不在 NR-1。

## 当前冻结事实

- Production Commands：24
- Conversation Commands：3
- Provider Calls / Turn：NR-0 基线为 1
- Skill Calls / Turn：0
- Tool Calls / Turn：0
- Canon Operations / Turn：0
- Store reconciliation 最大间隔：1000ms
- 自动化外部请求：0
- 真实 Provider Live Smoke：未执行

这些是 NR-G1 已通过基线，不代表 NR-1 的压缩阈值或累计输出预算已经冻结。

## 下一步

评审 `NR1-D1-R01`～`NR1-D1-R04`。在明确批准 Final Packet 和连续实施顺序前：

- 不开始 NR-101；
- 不新增 Memory、Compactor 或 Context Assembler 生产代码；
- 不加载 Novelist Skill；
- 不调用 Tool 或读取/写入 Canon；
- 不执行真实 Provider Live Smoke；
- 不进入 NR-2。
