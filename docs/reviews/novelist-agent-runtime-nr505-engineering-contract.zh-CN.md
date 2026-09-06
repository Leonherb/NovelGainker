# NR-505 原生出版工程合同（已完成；后续回归单独记录）

本文件冻结用户已委托的范围内工程参数；不代表 Story 通过，不扩展 Provider、分发或云服务授权。

- 原生 Rust 构建 EPUB，不执行 Python/命令、不下载资源。书名、作者使用真实工程字段 `title` / `authorDisplayName`，语言 `writingLanguage: zh-CN`；缺失请通过已有编辑器修改 `project.md`。可选 `coverImage` 限受控 `assets/` PNG/JPEG，不自动生成封面。
- 最多 500 章、8 MiB 原始章节文本、200 个图片 / 64 MiB 图片；单张最多 16 MiB、8192×8192 且不超过 16,777,216 像素。只选择有效 Host 定稿确认绑定当前字节的 final 章。草稿不导出并提示；伪造/失效 final 不能被悄悄忽略。
- 输入一次冻结，构建只消费快照；ZIP 流式写入，输出上限 112 MiB，渲染/目录等额外临时数据上限 8 MiB。出版自有缓冲区逻辑预算 192 MiB，不包含既有 History/Change/Task 等其他模块，不声称整个应用 RSS 小于此值。结束重检输入及确认，变化返回已过期，不能冒称当前稿。
- Markdown 首版明确支持段落、一级至六级标题、简单无嵌套强调/粗体/行内代码、无嵌套列表、引用、围栏代码和独占行的本地图片。拒绝表格、脚注、原始 HTML、链接、嵌套标记等未支持语法，返回封闭错误类别（当前不提供行号定位）；绝不静默剥掉正文。只导出唯一 `## Draft` 区段，章节其他笔记不导出。
- 图片按签名、扩展名/MIME、尺寸和有界结构验证，不执行主动内容；PNG 另校验各块 CRC。不把 JPEG 结构验证称为完整像素解码。
- Host UUID 产物仅 `published/{artifactId}.epub`；原子无覆盖提升本次临时文件，旧产物不动。派生回执限 256 条 / 每条 512 KiB，目录总量（含中断暂存）1 GiB；目录满明确拒绝，不自动删除旧书。Provider 只接收摘要与 opaque ID，不返回完整 500 章清单或任意导出路径。
- 预检与构建共用每 Turn 一次 publication 配额，build 自带预检。调用须当前用户来源和实际送达的 publish-novel 方法回执。模型语义判断不替换为 Host 关键词路由。只新增 publication_list/detail 两个封闭 IPC。
- 工具/产物的“已生成、验证失败、已过期”独立于聊天终态、资料保存和上架状态；普通出版不强制 analytical-review。真实外部 EPUB 阅读器、完整第三方认证及真实 Provider 语义结果不得由本地结构测试冒充。

## 格式与固定依赖

采用 [W3C EPUB 3.3](https://www.w3.org/TR/2026/REC-epub-33-20260113/) 的容器、包、导航结构及 [PKWARE ZIP 规范](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT)：首条 mimetype 不压缩、无额外字段，固定安全条目；OPF/导航/spine 顺序一致并记录 UTC 修改时间。

以下版本均已存在本机 Cargo 缓存和锁文件；仅离线解析，不下载：quick-xml 0.41.0（MIT，XML 结构验证）、crc32fast 1.5.0（MIT OR Apache-2.0，ZIP/PNG CRC）、chrono 0.4.45（MIT OR Apache-2.0，UTC 元数据）。ZIP 仅实现有界 Stored 输出子集，不引入通用任意 ZIP 解包通道。
