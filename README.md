# NovelGainker

Windows-first、Local-first、可切换 AI Provider 的简体中文小说创作桌面应用。

当前实现为 NR-1 长对话基线：三栏界面左侧管理工程、文件与 Provider，中间承载工程绑定的完整自然对话，右侧编辑真实 Markdown。Host 会在字节阈值以下直接调用一次主回复；达到阈值时先以一次内部 Compactor 增量维护派生 Memory，再调用一次主回复。两条路径均可取消、单 HTTP attempt、零重试。

NR-101～NR-106 已完成并停止在 NR-G2 技术提交，`nrG2Approved = false`。现有产品仍不加载 Novelist Skill、不读取 Canon 作为模型上下文、不调用工具、不由 AI 写入 Markdown，也未执行真实 Provider Live Smoke；未获单独批准前不进入 NR-2。

## 开发入口

- [当前架构](./docs/architecture.zh-CN.md)
- [开发与质量门](./docs/development.md)
- [当前交付状态](./docs/sprint-plan.zh-CN.md)
- [NR-0 Clean Slate Story Packet](./docs/stories/novelist-agent-runtime-nr0-clean-slate-story-packet.zh-CN.md)
- [NR-G1 Remediation Final](./docs/stories/novelist-agent-runtime-nr-g1-remediation-story-packet-draft-2-final.zh-CN.md)
- [NR-G1 技术重新提交](./docs/reviews/novelist-agent-runtime-nr-g1-technical-submission.zh-CN.md)
- [NR-1 Story Packet Draft 2 Final](./docs/stories/novelist-agent-runtime-nr1-history-memory-story-packet-draft-2-final.zh-CN.md)
- [NR-G2 技术提交](./docs/reviews/novelist-agent-runtime-nr-g2-technical-submission.zh-CN.md)

```powershell
pnpm install --frozen-lockfile
pnpm typecheck
pnpm test:frontend
pnpm quality
pnpm dev
```
