# 让本机状态库故障可诊断且可安全恢复

- **ID:** STORY-STARTUP-001
- **Epic:** EPIC-STARTUP-RELIABILITY
- **Priority:** Must Have
- **Story Points:** 5
- **Status:** Archived / Unplanned（不属于当前 NR-1；重新启用需单独批准）

## User Story

作为一名 NovelGainker 用户，我希望在本机状态库无法使用时看到可操作的故障原因和安全恢复入口，以便恢复界面偏好与 Provider 配置引用，而不会误伤项目 Markdown 文件或 Windows 凭据。

## Acceptance Criteria

- [ ] 状态库打开、迁移、读取、写入和记录校验失败时，应用保留稳定且不含敏感信息的失败类别。
- [ ] `app_state.unavailable` 向用户显示具体恢复建议，而不是将所有原因折叠为同一文案。
- [ ] 用户可在执行前确认备份状态库，并在确认后重建本机状态库。
- [ ] 重建操作不删除项目目录中的 Markdown 文件，也不读取、显示或删除 Provider Key。
- [ ] 重启后应用可重新加载新建状态库；Provider 凭据引用缺失时给出重新关联提示。
- [ ] 为损坏数据库、无写权限、迁移失败和无效历史记录增加自动化测试。

## Technical Notes

### Implementation Approach

为 `AppStateRepository` 保留受控的失败类别与只读诊断摘要；提供显式备份/重建命令，不在自动恢复路径中删除任何状态文件。

### Historical Module Notes

该提案引用的是 NR-0 Clean Slate 之前的模块名，不能作为当前实施清单。若未来重新批准，必须先基于现行 `app_state_current.rs`、`commands_current.rs` 与当前 UI 重新形成 Story Packet。

### Security Considerations

诊断输出不得包含 SQLite 原始数据、项目内容、Provider Key 或系统凭据内容。

## Dependencies

- **Blocked by:** 无当前 Sprint 排期；只有在现行 NR 基线上重新确认问题并单独批准后才可重开。
- **Blocks:** 本机偏好与 Provider 配置的可靠恢复。

## Testing Requirements

- [ ] 损坏数据库时仍可手动打开和编辑项目。
- [ ] 备份后重建不会修改项目 Markdown 文件。
- [ ] Windows 凭据不出现在日志、诊断或恢复产物中。

## Definition of Done

- [ ] 所有验收标准通过。
- [ ] 新增测试通过，且质量门禁通过。
- [ ] 文档说明状态库的备份与恢复边界。
