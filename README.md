# NovelGainker

Windows-first、Local-first、可切换 AI Provider 的简体中文小说创作桌面应用。

当前实现为 NR-0 自然对话基线：三栏界面左侧管理工程、文件与 Provider，中间只承载工程绑定的自然对话，右侧编辑真实 Markdown。每个用户 Turn 最多调用一次 Provider；对话持久化、流式显示、停止、异常终态与 Store 自动对账已实现。

NR-G1 技术提交已获产品批准，当前正在评审 `NR-1 Story Packet Draft 1`，尚未授权 NR-1 实施。现有产品仍不加载 Novelist Skill、不读取 Canon 作为模型上下文、不调用工具、不由 AI 写入 Markdown，也未执行真实 Provider Live Smoke。

## 开发入口

- [当前架构](./docs/architecture.zh-CN.md)
- [开发与质量门](./docs/development.md)
- [当前交付状态](./docs/sprint-plan.zh-CN.md)
- [NR-0 Clean Slate Story Packet](./docs/stories/novelist-agent-runtime-nr0-clean-slate-story-packet.zh-CN.md)
- [NR-G1 Remediation Final](./docs/stories/novelist-agent-runtime-nr-g1-remediation-story-packet-draft-2-final.zh-CN.md)
- [NR-G1 技术重新提交](./docs/reviews/novelist-agent-runtime-nr-g1-technical-submission.zh-CN.md)
- [NR-1 Story Packet Draft 1](./docs/stories/novelist-agent-runtime-nr1-history-memory-story-packet-draft-1.zh-CN.md)

```powershell
pnpm install --frozen-lockfile
pnpm typecheck
pnpm test:frontend
pnpm quality
pnpm dev
```
