# NR-G5 产品批准记录

日期：2026-09-05。决定者：用户。原始决定：“批准NR-G5，我想你帮我将后续的story都一起跑完，评审NR-5到NR-7的 story”。

- NR-G5：passed / product_approved / `nrG5Approved=true`。
- NR-4 实施已包含于当前提交 `0f61a9aaafb6384669e4a8e2b75bdf0becf5bb88`；本轮不修改其生产实现。
- 原 [NR-G5 技术提交](novelist-agent-runtime-nr-g5-technical-submission.zh-CN.md) 与 Manifest 中 false 为批准前冻结快照，不能改成 true 冒称技术提交时已经产品批准；现行批准以本记录及 BMAD `current_gate` 为准。
- NR-4 的最小只读 Live Smoke waiver 和未验证真实兼容性风险保留；不是取消 NR-6 完整真实验证。
- 用户明确要求提前集中评审 NR-5～NR-7，允许一次形成18项 Story；并表达连续完成后续内容的意愿。
- 新Packet的具体预算/出版/Session边界尚待联合决定；本轮没有把这句话变成真实凭据/网络/云端部署授权或后续 Gate 的预先批准。
- 后续范围已被用户纠正为仅NR-5/NR-6：明确无云同步需求，整个计划删除该部分，不保留服务选择或条件阶段。最初18项是形成时记录，不是现行交付承诺。
- 不重开 NR-G4/G5，不恢复 Pre-NR-4；当前交付为 [联合 Story Packet](../stories/novelist-agent-runtime-nr5-nr6-story-packet-draft-1.zh-CN.md) 与联合评审。

## 机器状态映射

`current_gate` 表示最新产品决定；BMAD 旧 `gate`、`nr4_packet.nrG5Approved=false` 是现有历史检查器读取的技术提交兼容快照，不是当前产品裁决。字段旁注明 scope，避免下一个执行者误读。

NR-501 必须把这些旧检查器改为明确读取历史基线与当前批准两个独立来源，并做批准撤销反例；不能一直靠全文搜索某个 true/false 判定阶段。此次只更新文档/状态，没有改测试断言来让未实现的阶段“通过”。
