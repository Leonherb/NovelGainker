# NovelGainker

Windows-first、Local-first、可切换 AI Provider 的简体中文小说创作桌面应用。

当前实现为 NR-2 Host Agent Loop 基线：三栏界面左侧管理工程、文件与 Provider，中间承载工程绑定的完整自然对话及受控内联 Task 状态，右侧编辑真实 Markdown。Host 保留完整 History、按阈值维护派生 Memory，并支持 Provider-native Tool Request/Result 的有界执行循环。

NR-201～NR-206 与 NR-G3-R01～R03 已完成，NR-G3 已批准。NR3-D2-R01～R06 与 FR01～FR02 已在 NR-3 常驻 Novelist Capability Pack / Canon Read Context Story Packet Draft 2 Final 中获产品批准，当前停止于 Packet 批准 Gate；NR-301～NR-306 尚未获实施授权。生产 Tool Registry 仍为空，因此现有产品不加载 Novelist Skill、不读取 Canon 作为模型上下文、不实际执行 Tool、不由 AI 写入 Markdown，也未执行真实 Provider Live Smoke；NR-4～NR-7 Story Packet 尚未获授权形成。

## 开发入口

- [当前架构](./docs/architecture.zh-CN.md)
- [开发与质量门](./docs/development.md)
- [当前交付状态](./docs/sprint-plan.zh-CN.md)
- [NR-0 Clean Slate Story Packet](./docs/stories/novelist-agent-runtime-nr0-clean-slate-story-packet.zh-CN.md)
- [NR-G1 Remediation Final](./docs/stories/novelist-agent-runtime-nr-g1-remediation-story-packet-draft-2-final.zh-CN.md)
- [NR-G1 技术重新提交](./docs/reviews/novelist-agent-runtime-nr-g1-technical-submission.zh-CN.md)
- [NR-1 Story Packet Draft 2 Final](./docs/stories/novelist-agent-runtime-nr1-history-memory-story-packet-draft-2-final.zh-CN.md)
- [NR-G2 技术提交](./docs/reviews/novelist-agent-runtime-nr-g2-technical-submission.zh-CN.md)
- [NR-2 Story Packet Draft 2](./docs/stories/novelist-agent-runtime-nr2-host-agent-loop-story-packet-draft-2.zh-CN.md)
- [NR-G3 Remediation Draft 2](./docs/stories/novelist-agent-runtime-nrg3-remediation-story-packet-draft-2.zh-CN.md)
- [NR-G3 技术提交与批准记录](./docs/reviews/novelist-agent-runtime-nr-g3-technical-submission.zh-CN.md)
- [NR-3 Story Packet Draft 2 Final](./docs/stories/novelist-agent-runtime-nr3-capability-canon-read-story-packet-draft-2-final.zh-CN.md)

```powershell
pnpm install --frozen-lockfile
pnpm typecheck
pnpm test:frontend
pnpm quality
pnpm dev
```
