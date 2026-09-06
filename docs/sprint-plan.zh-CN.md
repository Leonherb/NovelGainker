# NovelGainker 当前交付状态 — NR-G5 已批准，NR-5～NR-6 联合评审

更新日期：2026-09-05。

## 授权与停止点

用户已批准NR-G5，NR-401→406已实施并分别通过质量门。用户明确“将云同步从整个plan中去除，我并没有这个需求”；因此总路线与后续交付仅至NR-6 / NR-G7，不保留云阶段、条件路线或云服务待办。后续12项Story已自评，R01～R05仍按各自授权边界处理。

NR-G1～NR-G5 已批准，不重开旧Gate，不恢复 Pre-NR-4。R04前三项产品行为已接受，R05真实验证方式已确定为多用户直接试用，见[测试方案](testing/novelist-agent-runtime-nr6-multi-user-test-plan.zh-CN.md)。尚未发放或测试，参与者/费用/覆盖安排须发放前落实；不授权助手自动Live。R01～R03及NR-6未确认工程边界保留待决定。本次最小只读Live放弃与风险接受保留；正式章节来源仍待NR-5实现。历史Story/Gate属于当时冻结快照。

补漏裁决：用户已接受NG-F01～NG-F05，现已纳入原Story和测试单：版本绑定定稿、完整创作链、内联工具证据、32个非归档会话、安装升级及最终版本复验。归档仍受总容量/目录约束；不重问已批准名额，剩余工程边界与R01～R03整包决定仍待落实。测试单31项含开发者检查，不要求每人全跑；尚未实施/测试。

最新实施授权：用户已同意[NR-5～NR-6连续实施](reviews/novelist-agent-runtime-nr5-nr6-implementation-approval.zh-CN.md)，不再等待R01～R03。当前NR-501实施中，完成数仍0；每项完整quality，NR-G6技术通过后直接进入NR-6。多用户试用及最终NR-G7仍待实际证据/用户参与，不自动Live。

## NR-4 状态

| Story | 状态 | 核心交付 |
| --- | --- | --- |
| NR-401 | completed | 五类 Mutation、来源/完整读取证据、预算及封闭合同 |
| NR-402 | completed | Host 持久快照、事务、回执与恢复 |
| NR-403 | completed | 自主选方法、内置投影 1.1.0、有界读后写与双 Provider |
| NR-404 | completed | 桌面挂载、编辑保护、真实 Host 结果、Diff、本地确认 |
| NR-405 | completed | CAS Undo、逆事务恢复、冲突/配额与外部修改保护 |
| NR-406 | completed | 五目录事务、最大调用链、失败保文、最终质量与 NR-G5 证据 |

NR-401～406 已分别通过完整质量门。最终 `pnpm quality` 于 2026-09-05 06:27 +08:00 完成，25 步全部 exit 0；NR-G5 后续产品批准见[独立批准记录](reviews/novelist-agent-runtime-nr-g5-product-approval.zh-CN.md)。

## 后续12项Story的联合评审

| 阶段 | Story | 当前状态 | 实施依赖 / 退出Gate |
| --- | --- | --- | --- |
| NR-5-D1 | NR-501～506 | 设计已自评，可提交产品决定 | R01～R03 / NR-G6 |
| NR-6-D1 | NR-601～606 | R04产品行为已接受，R05改多用户测试；未实施 | NR-G6及剩余工程边界；604/605多用户覆盖与证据 / NR-G7 |

入口：[联合Story Packet](stories/novelist-agent-runtime-nr5-nr6-story-packet-draft-1.zh-CN.md)；[逐项评审与发现](reviews/novelist-agent-runtime-nr5-nr6-story-packet-draft-1-review.zh-CN.md)。此次仅文档/状态更新，未重新运行完整质量门，不把NR-G5历史结果冒称后续阶段验收。

当前产品裁决读 BMAD `current_gate`，后续范围读 `successor_packet`。旧 `gate`/`nr4_packet` 是显式标注的历史检查兼容快照，NR-501 将检查器改为分层读取；不能把旧 `nrG5Approved=false` 当当前未批准。

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

- Scope：review-gate-packet；Windows desktop；Level 4；Codex；范围与代码事实高置信。
- Current phase：NR-G5产品已批准，NR-5～6共12项Story评审完成。
- Recommended next move：裁决R01～R03以释放NR-501→506；后续落实多用户测试批次。
- Why：R04产品行为已接受，R05多用户方式已确定；新预算/出版与剩余工程边界、测试安排仍须确认，无云同步。
- Route-outs：BMAD协调阶段；专用plannotator不可用，本轮主代理自评，不声称独立评审。
- What not to do：不代批NR-G6/G7，不把规划标为已实现，不执行真实Live/云端上传，不恢复Pre-NR-4。
