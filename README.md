# NovelGainker

Windows、本地工程、可切换AI Provider的简体中文小说创作桌面应用。

当前源码支持自然对话、内置小说方法、受约束的资料/单章写入、实际保存回执、Diff/Undo、多会话及本地EPUB。它采用单轮有界Agent执行；分阶段自主创作和有限自动恢复是2026-09-09已完成策划整理的新目标，不代表已实现。

## 产品和方案

- [目标产品策划案（17项决定已确认）](docs/product-spec.zh-CN.md)
- [当前产品与实现状态](docs/product-current-state-report.zh-CN.md)
- [目标技术方案（方向已确认，开始分片实现）](docs/technical-design.zh-CN.md)
- [从零测试重置与首个实现切片](docs/reviews/implementation-slice-a-2026-09-10.zh-CN.md)
- [当前架构](docs/architecture.zh-CN.md)
- [全工程梳理与决策记录](docs/reviews/project-reframe-2026-09-09.zh-CN.md)
- [技术专项问题与确认记录](docs/reviews/technical-design-alignment-2026-09-09.zh-CN.md)
- [15项技术缺口处理方案（全部已确认，待实现验证）](docs/reviews/technical-gap-closure-proposal-2026-09-10.zh-CN.md)
- [交付与验收状态](docs/sprint-plan.zh-CN.md)

## 开发入口

见[开发与质量门](docs/development.md)。安装依赖后，Rust验证使用`--locked --offline`。

```powershell
pnpm install --frozen-lockfile
pnpm typecheck
pnpm test:frontend
pnpm quality
pnpm dev
```

自动化验证使用隔离合成工程和loopback Provider。真实用户试用有独立验收要求，见[测试方案](docs/testing/novelist-agent-runtime-nr6-multi-user-test-plan.zh-CN.md)。模型说“已完成”不代替实际文件提交证据。产品不包含工程云同步。
