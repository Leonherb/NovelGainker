# 防止 Git 行尾转换破坏受锁定资源

- **ID:** STORY-STARTUP-002
- **Epic:** EPIC-STARTUP-RELIABILITY
- **Priority:** Should Have
- **Story Points:** 3
- **Status:** Not Started

## User Story

作为一名在 Windows 上启动开发环境的贡献者，我希望受哈希锁保护的资源在克隆、安装和启动后保持字节一致，以便 `pnpm dev` 不会因 Git 自动行尾转换而失败。

## Acceptance Criteria

- [ ] `.gitattributes` 明确规定受锁定资源使用 LF 行尾和 UTF-8 无 BOM 编码。
- [ ] 开发启动前或 CI 中会验证这些资源的哈希与行尾，不一致时给出明确修复命令。
- [ ] 新克隆的 Windows 工作区在 `core.autocrlf=true` 环境中也能完成资源验证，或在首次启动前得到阻断性提示。
- [ ] 修复过程不改写业务源文件，不触及项目 Markdown 内容。
- [ ] 现有资源锁与合约检查继续通过。

## Technical Notes

### Implementation Approach

为锁定资源补充 Git 属性和可重复的预检脚本；将字节差异与运行时资源验证错误映射到具体文件集合。

### Files/Modules Affected

- `.gitattributes` - 受锁定资源的行尾策略。
- `scripts/` - 安装或启动前资源预检。
- `resources/` 相关锁文件 - 仅用于验证，不进行无关内容改写。

## Dependencies

- **Blocked by:** Sprint 6 暂停；需要用户授权后实现。
- **Blocks:** Windows 开发环境的可重复启动。

## Testing Requirements

- [ ] 在 CRLF 自动转换模拟环境中执行预检。
- [ ] 在正确 LF 工作区中执行 `pnpm dev` 和资源锁检查。

## Definition of Done

- [ ] 所有验收标准通过。
- [ ] Windows 新克隆工作区的启动验证通过。
- [ ] 资源锁检查无新增差异。
