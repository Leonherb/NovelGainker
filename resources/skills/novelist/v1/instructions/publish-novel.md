# EPUB 出版预检与构建语义

## 目标

在作者明确要求预检或构建时，读取方法后使用当前 Host 注册的 `novelist.publish`，得到真实本地 EPUB 预检/产物回执。普通出版不强制额外评审或改稿。

## 工作规则

1. v2 请求精确为 schemaVersion:2、action:preflight或build、methodReceiptId、source:{messageId,contentHash}；从已送达方法回执及本轮当前用户来源目录复制真实标识。不要传路径、正文、URL或虚构确认。老 Host 未注册本工具时只能解释限制，不能虚报执行。
2. 每轮只一次出版调用；build 已包含预检，不先耗掉配额再构建。Host 冻结并复核输入，原生构建不执行外部命令、Python、图片生成或下载。
3. 书籍元数据来自 project.md 的 title、authorDisplayName、writingLanguage:zh-CN；缺失请用户通过已有编辑器补齐，不调用任意根目录写入。可选 coverImage 是 assets 内既有 PNG/JPEG；没有封面仍可生成，并说明“无封面 EPUB”。
4. 只导出作者明确确认且当前字节仍匹配 Host 定稿凭证的 final 章节中唯一 ## Draft 正文，忽略笔记。保存 draft 不等于定稿；历史 final、旧确认或修改后残留字段均不授权。不要擅自将所有草稿定稿，用户需明确决定。
5. 最多500章/8MiB原始文本，最多200张PNG/JPEG合计64MiB，单图16MiB、边长8192且像素不超过16777216。正文支持段落/标题/简单强调与代码/无嵌套列表及引用/独占行本地图片；表格、链接、原始HTML、嵌套标记等未知语法会阻断本次出版，不能偷偷删掉内容绕过。图片写法 ![说明](../assets/插图.png) 或 assets 相对路径；read_context不开放资产原始字节，出版Host负责受控读取。
6. generated 表示本地已生成；preflight_passed表示只通过预检且未生成；validation_failed 是验证失败；stale 是输入/确认已变化的旧版本。只引用安全回执中的产物ID/相对路径，绝不声称已上架或完整第三方认证。来源变化后应核对当前稿、必要时重新定稿并在新用户轮次构建。
7. 失败不能阻断正常聊天、删除原稿或否认旧产物。metadata_required需补元数据；no_final_chapters/final_confirmation_required需用户明确确认合格版本；unsupported_markdown/invalid_asset需修正对应输入；source_changed需核对变化；store_unavailable/local_validation_failed不得冒报成功。不要复述接口组合或参数错误堆栈。

## 禁止项

- 不运行外部程序、脚本、系统命令或网络请求。
- 不直接编辑生成产物。
- 仅由具体 Host 工具新建 published/{artifactId}.epub；不覆盖已有书籍，不接受用户/模型任意输出路径或系统命令。未经明确出版要求零构建。
