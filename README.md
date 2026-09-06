# NovelGainker

Windows-first、Local-first、可切换 AI Provider 的简体中文小说创作桌面应用。

当前实现进入 NR-4：三栏左侧管理工程、文件与 Provider，中间承载完整自然对话和 Host 保存结果，右侧编辑真实 Markdown。应用自包含 Novelist 九能力、37 资源；AI 可按需读取 Canon，并受约束地维护人物、世界/设定、素材、剧情与大纲，支持 Diff、本地确认和可恢复 Undo。

NR-401～406 已分别通过完整质量门，[NR-G5 已获产品批准](docs/reviews/novelist-agent-runtime-nr-g5-product-approval.zh-CN.md)。[NR-5～NR-6 联合 Story](docs/stories/novelist-agent-runtime-nr5-nr6-story-packet-draft-1.zh-CN.md) 已形成并自评，尚未实施；后续仅12项Story，止于NR-G7。云同步不是用户需求，已从整个计划移除。正式章节独立来源延至 NR-5。本次最小只读 Live Smoke 已明确放弃，真实 Provider 工具兼容性和语义质量未正式验证，NR-6 完整验证保留；不把模型的“已完成”当作实际保存证据。

## 开发入口

- [当前产品报告](./docs/product-current-state-report.zh-CN.md)
- [NR-4 实施与验证记录](./docs/reviews/novelist-agent-runtime-nr4-implementation-log.zh-CN.md)

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
