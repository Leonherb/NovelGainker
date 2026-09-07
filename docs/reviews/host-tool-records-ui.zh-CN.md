# 对话内工具记录（2026-09-08）

用户明确要求：工具使用由 Gainker 自行识别、在对话显示并留痕供复查，不依赖模型回复。本修改落实这项独立用户请求，不声明 NR-604 或任何 Gate 完成。

## 行为与边界

- 对应轮次下显示独立的「Gainker 工具记录」区，逐条保留等待、执行中、完成、失败及中止状态。调用之间按记录创建时间排序，不再错误地用单条记录的 revision 选出一个代表全部操作。
- 每条操作使用原生可键盘展开的 details/summary。详情提供 Host 工具名、版本、记录创建/更新时间、记录编号及错误码的中文说明。写入及出版是否实际完成，继续以既有文件/EPUB 回执为准。
- 实时 TaskActivity 事件和会话加载使用同一投影；重开会话直接读取已有 TaskRecord，即使没有 AI 正文也能显示记录。只保留有界工具元数据，不增加原始参数、正文、Provider 请求或密钥记录。
- TaskActivity 在原六字段基础上增加 toolName、toolVersion、startedAtMs、updatedAtMs、errorCode（显式 null）。Rust 映射、生产 TypeScript 类型、严格解析及测试同步更新；继续使用现有事件/会话加载命令，38 个 IPC 的命令、Handler、权限和 Capability 列表未增加。
- 既有 Task 存储格式不变，不修改用户历史。记录绑定原会话和轮次，原有 revision 去重、后台对账和用户向上阅读时停止自动滚动的行为保留。

## 实际验证

- 156 项前端测试通过，覆盖实时状态、多个工具、错误详情、无 AI 正文、只读会话重开复查、无 Provider 重发，以及严格 DTO 的非法输入拒绝。
- 7 项 Task Store 测试通过，验证恢复后的工具身份、时间和中断原因一致，以及投影不含原始参数/摘要内容。
- TypeScript、ESLint、app-core 全 targets Clippy、修改的前端文件 Prettier 和 diff 空白检查通过。
- 通过 loopback 合成预览实际展开错误记录并检查浏览器截图，工具名称、状态、原因和记录编号可见。
- Release 无安装包构建通过；包含之前的参数反馈与会话滚动修复。没有中断用户正在运行的旧应用，没有真实 Provider 调用。
- `pnpm quality` 仍停在仓库既有 29 个无关文件的 Prettier 问题，未改写无关文件或放宽检查；不声称完整质量 Gate 通过。
