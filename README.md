# NovelGainker

桌面优先、Local-first、可切换 AI Provider 的简体中文小说创作工具。

当前进度：G5、GA0 与 GA1 已获产品批准；AM2 的 NG-A201～NG-A206 与 GA2-R001～GA2-R004 均已完成，revision 5 完整质量门与 GA2 技术提交通过。当前停在 GA2 独立产品验收点；`ga2Approved=false`，Sprint 6 与真实 Provider Live Smoke 未授权。

## 开发入口

- [架构](./docs/architecture.zh-CN.md)
- [统一 Agent 架构重审与批准记录](./docs/architecture-unified-novelist-agent-review.zh-CN.md)
- [Sprint 计划](./docs/sprint-plan.zh-CN.md)
- [Sprint 0 Story Packet](./docs/stories/sprint-0-story-packet.zh-CN.md)
- [Sprint 1 Story Packet](./docs/stories/sprint-1-story-packet.zh-CN.md)
- [Sprint 2 Story Packet](./docs/stories/sprint-2-story-packet.zh-CN.md)
- [Sprint 3 Story Packet](./docs/stories/sprint-3-story-packet.zh-CN.md)
- [Sprint 4 Story Packet](./docs/stories/sprint-4-story-packet.zh-CN.md)
- [Sprint 5 Story Packet](./docs/stories/sprint-5-story-packet.zh-CN.md)
- [Agent Migration AM0 Story Packet（GA0 已关闭）](./docs/stories/agent-migration-am0-story-packet.zh-CN.md)
- [Agent Migration AM1 Story Packet（GA1 已关闭）](./docs/stories/agent-migration-am1-story-packet.zh-CN.md)
- [Agent Migration AM2 Story Packet（GA2 待产品验收）](./docs/stories/agent-migration-am2-story-packet.zh-CN.md)
- [Sprint 6 Story Packet（已暂停）](./docs/stories/sprint-6-story-packet.zh-CN.md)
- [开发环境](./docs/development.md)

```powershell
pnpm install --frozen-lockfile
pnpm test:frontend
pnpm quality
pnpm dev
```
