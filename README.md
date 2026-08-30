# NovelGainker

桌面优先、Local-first、可切换 AI Provider 的简体中文小说创作工具。

当前进度：对话式小说创作 RB0、RB-Q001～RB-Q008 与 Agent Protocol V2 的 AP-Q001～AP-Q010 已于 2026-08-30 获产品批准。新的 AM-R1 Story Packet 正在等待独立 Review Gate；NG-R101、运行时代码修改、旧产品层删除、AM-R2/AM-R3、Sprint 6 与真实 Provider Live Smoke 均未授权。

## 开发入口

- [架构](./docs/architecture.zh-CN.md)
- [对话式小说创作重基线 Plan](./docs/stories/agent-conversation-authoring-rebaseline-plan.zh-CN.md)
- [Agent Protocol V2 与 Context Mark 规格](./docs/stories/agent-protocol-v2-context-mark-review.zh-CN.md)
- [Sprint 计划](./docs/sprint-plan.zh-CN.md)
- [Sprint 0 Story Packet](./docs/stories/sprint-0-story-packet.zh-CN.md)
- [Sprint 1 Story Packet](./docs/stories/sprint-1-story-packet.zh-CN.md)
- [Sprint 2 Story Packet](./docs/stories/sprint-2-story-packet.zh-CN.md)
- [Sprint 3 Story Packet](./docs/stories/sprint-3-story-packet.zh-CN.md)
- [Sprint 4 Story Packet](./docs/stories/sprint-4-story-packet.zh-CN.md)
- [Sprint 5 Story Packet](./docs/stories/sprint-5-story-packet.zh-CN.md)
- [AM-R1 Story Packet（等待产品评审）](./docs/stories/agent-conversation-authoring-am-r1-story-packet.zh-CN.md)
- [Sprint 6 Story Packet（已暂停）](./docs/stories/sprint-6-story-packet.zh-CN.md)
- [开发环境](./docs/development.md)

```powershell
pnpm install --frozen-lockfile
pnpm test:frontend
pnpm quality
pnpm dev
```
