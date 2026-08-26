# NovelGainker

桌面优先、Local-first、可切换 AI Provider 的简体中文小说创作工具。

当前进度：Sprint 5 Story Packet Review Gate 已通过；NG-501 Context Builder v2 已完成技术实现并通过全量质量门，等待产品验收；NG-502 尚未授权。

## 开发入口

- [架构](./docs/architecture.zh-CN.md)
- [Sprint 计划](./docs/sprint-plan.zh-CN.md)
- [Sprint 0 Story Packet](./docs/stories/sprint-0-story-packet.zh-CN.md)
- [Sprint 1 Story Packet](./docs/stories/sprint-1-story-packet.zh-CN.md)
- [Sprint 2 Story Packet](./docs/stories/sprint-2-story-packet.zh-CN.md)
- [Sprint 3 Story Packet](./docs/stories/sprint-3-story-packet.zh-CN.md)
- [Sprint 4 Story Packet](./docs/stories/sprint-4-story-packet.zh-CN.md)
- [Sprint 5 Story Packet](./docs/stories/sprint-5-story-packet.zh-CN.md)
- [开发环境](./docs/development.md)

~~~powershell
pnpm install --frozen-lockfile
pnpm test:frontend
pnpm quality
pnpm dev
~~~
