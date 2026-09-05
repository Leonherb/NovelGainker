# NovelGainker 当前交付状态 — NR-4

更新日期：2026-09-05。

## 授权与停止点

用户已批准 NR-4-D1 / R01～R04，按 NR-401 → NR-406 顺序实施，每 Story 通过 `pnpm quality`，在 NR-G5 技术提交停止等待产品批准。本次最小只读 Live Smoke 明确放弃、兼容性风险接受；NR-6 完整验证保留。正式章节独立来源延至 NR-5。

NR-G1～NR-G4 已批准；NR-G4 不重开，不恢复 Pre-NR-4。NR-5～NR-7 尚未形成或实施。本文件描述现行事实；历史 Story/Gate 的“当时未实施”属于冻结快照。

## NR-4 状态

| Story | 状态 | 核心交付 |
| --- | --- | --- |
| NR-401 | completed | 五类 Mutation、来源/完整读取证据、预算及封闭合同 |
| NR-402 | completed | Host 持久快照、事务、回执与恢复 |
| NR-403 | completed | 自主选方法、内置投影 1.1.0、有界读后写与双 Provider |
| NR-404 | completed | 桌面挂载、编辑保护、真实 Host 结果、Diff、本地确认 |
| NR-405 | completed | CAS Undo、逆事务恢复、冲突/配额与外部修改保护 |
| NR-406 | completed | 五目录事务、最大调用链、失败保文、最终质量与 NR-G5 证据 |

NR-401～406 已分别通过完整质量门。最终 `pnpm quality` 于 2026-09-05 06:27 +08:00 完成，25 步全部 exit 0；NR-G5 已技术提交，产品尚未批准。

## 当前生产边界

- 内置九能力 / 37 资源，应用投影 1.1.0；上游 novelist 0.2.0 来源不变。
- 写权限会话两个工具：read_context / propose_change；只读会话保留只读回退，普通聊天可零工具。
- 五目录：characters/world/materials/plot/outlines；最多四目标、候选合计 8 KiB。
- 最多两读一变更、4 Agent + 1 Compactor、累计 17,408 请求输出 tokens；实际请求体 256 KiB、Continuation 160 KiB。
- 当前 28 条 IPC，其中 Conversation 三条；本地变更四条不接受任意路径/正文。
- AI 建议不自动变成 Canon；来源语义由模型判断，Host 校验引用、Hash、事务与权限。
- Diff/确认/Undo 来自持久 Host 数据，不以模型“已完成”作为保存依据。
- 外部修改无法恢复时保留证据与 Canon、阻止进一步写入；只读工具和聊天可继续。
- 只有手工未保存稿能阻断关窗；待确认 AI 草稿不阻断。

## 证据与历史记录

当前实施记录：[NR-4 实施与验证](reviews/novelist-agent-runtime-nr4-implementation-log.zh-CN.md)。Gate 交接：[NR-G5 技术报告](reviews/novelist-agent-runtime-nr-g5-technical-submission.zh-CN.md)。当前验证入口：`pnpm e2e:nr406`；完整门禁：`pnpm quality`。

`target/qa/nr-406/validation.json` 是定向验证，`quality.json` 只在完整门禁通过后生成，`nr-g5-manifest.json` 仅在两者匹配后形成技术提交；均不代表产品 Gate 批准。

NR-G1～G4 的技术报告及原机器声明保持冻结。NR-4 进入后的旧合同回归写 `nr-g1/2/3/4-regression-manifest.json`，分别位于 nr-006/106/206/306 下，当前 IPC/投影另列，不覆盖技术快照。

NR-0～3 及 NR-G1～G3 Remediation 已完成，NR-301～306 在 NR-G4 获产品批准。DeepSeek 空人物检查修复保留在运行时基线，没有独立 Pre-NR-4 计划。

## 已接受风险与未开放内容

真实 Provider Tool/Skill 兼容性和语义质量未正式验证；全部自动化使用合成工程与 Loopback。机械回执和脚本化响应不能证明真实模型能正确区分只读、假设或接受建议。正式章节写入/独立来源、图像生成、出版和 NR-6 长篇验证不在本次完成声明内。

## BMAD Routing Brief

- Scope：brownfield-resume-packet；Windows desktop；Level 4；Codex；证据置信度高。
- Current phase：NR-4 implementation 完成，NR-G5 产品评审边界。
- Recommended next move：review gate，等待用户决定是否批准 NR-G5。
- Why：范围已批准，401～406 已通过逐项质量门，技术报告与同次质量证据已匹配；剩余决定是产品 Gate 批准，而非继续实施。
- Route-outs：BMAD 管阶段；ui-ux-pro-max 指导内联结果与保护交互；neat-freak 同步现行文档；无新增独立任务。
- What not to do：不批准 NR-G5，不进入 NR-5，不执行真实 Live Smoke，不恢复 Pre-NR-4。
