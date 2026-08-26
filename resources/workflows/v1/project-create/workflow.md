# 创建项目

## 输入与输出

- 接收严格的 ProjectCreateInputV1，标题必填；作者显示名、类型和一句话前提可选。
- 只生成确定性的 ProjectScaffoldPreviewV1，不调用 AI，不读取或要求 API Key。
- 写作语言固定为简体中文（zh-CN）。

## 本地边界

- Rust 生成 Project ID、目标路径、固定目录、技术元数据、文件字节和 Preview Hash。
- 用户输入不能指定路径、ID、Frontmatter、工具、Provider、模型或任意 Markdown 文件。
- 目录扫描、表单填写和完整脚手架预览均为零写入。
- 预览列出 11 个目录、4 个初始文件、现有无关项提示和冲突。
- 技术元数据不得包含作者、类型、一句话前提、用户路径、API Key 或正文。

## 最终确认

1. 用户审查完整脚手架预览。
2. 用户执行一次 create_project_scaffold 最终确认。
3. Rust 重新扫描所选根目录并核对 Preview Hash。
4. 仅在根目录快照、输入、模板、版本和计划字节全部未变化时，使用 add-only 事务创建新项。
5. 任一目标已存在、类型不符、链接、快照变化或写入失败时，不覆盖；回滚只清理本事务创建的项。

本工作流只使用内部 project.scan 与 project.create 能力，不进入 Provider、Vault、聊天或 AI Workflow Run。
