# 候选保存失败与额度耗尽后回复中断修复

日期：2026-09-14。基线`0b8b6b6`加本地补丁，包含此前工具流解析修复；不变更Story/Gate完成状态。

## 真实记录

截图对应轮次两次read_context成功；随后propose_change依次为candidate_invalid、target_invalid、budget_exceeded，均在执行前校验阶段失败。终态为provider.invalid_request，五次Agent请求，未出现该轮Committed保存证据。助手自述“world的kind/version错误”并非校验器给出的精确诊断：旧实现只有通用提示，原始候选未持久化，不能据此确认是哪行真实出错。

源码可确认收尾缺陷：当前tools已移除写工具后，注册表仍允许查到该工具；超额调用被写成失败Task并加入原生历史，使历史第三次mutation超过严格上限。continue_after_tool返回本地provider.invalid_request，模型没有机会生成最终解释。另有with_tool_definitions([])直接返回原请求、未实际清空工具的缺陷。

## 本轮修复

1. NR-5五类资料显式create、targetId/readReceiptId/expectedSourceHash均null时，由Host规范固定字段id=new、kind、schemaVersion、writingLanguage=zh-CN及BOM/CRLF/末尾LF。不猜标题、正文或其他创意字段；不更改目标引用。此后仍完整验证方法、来源、语法、大小及事务。Host在实际新建时分配最终身份和路径。
2. 规范化不覆盖update、upsert、章节或已展示章节候选；重复字段、嵌套固定字段、锚点及畸形头部不进行修补。缺失标题等真实内容错误仍失败，不能虚构内容使其通过。
3. 候选/目标校验失败返回最多4项、2048字节的argumentIssues，指出操作索引、固定字段、校验码、行号和修正要求。取自Host校验，不回显模型值、未知键、原始候选、实际文件路径或凭据。只用于Provider纠正；错误仍是失败，不当作保存证据。
4. 对已从当前tools移除的注册工具，不派发、不构造超额原生历史。保留已完成工具及提交回执，使用原轮剩余额度请求无工具文字收尾。正常清空工具定义，不增加预算，不替模型虚构保存成功。若模型继续无视无工具模式、取消或Provider故障，仍如实结束。

这与此前流式空分片/多调用修复属于不同层次。通用网络自动恢复、阶段持续执行以及G09完整Host装配方案仍未全部实现。当前补丁不能保证每个模型候选都能保存。

## 验证

新增完整离线链路：错误固定元数据经Host规范后实际保存；缺失标题返回精确字段、模型修正后实际保存；格式错误→目标错误→第三次超额调用不执行→正常未保存解释。断言Task数量、真实Markdown及Committed回执，不以模型自述判定成功。保留此前工具流解析七场景回归。

新建资料覆盖五类kind及缺失固定字段，正文内容不变、规范化幂等；负例覆盖伪造目标引用、更新/upsert、章节、重复字段、锚点、嵌套值及代码围栏，不绕过原校验。测试仅使用隔离合成工程及回环服务，无真实Provider或凭据读取，用户稿件未改。

首次默认并发的工作区测试有一项NR-504回环HTTP夹具读取请求头失败，保留在rust-workspace.log；随后单项复核及四线程工作区结果见最终检查记录。没有放宽超时或校验器。

本次不声明完整pnpm quality通过或Story/Gate完成；未重复运行与此修复无关的500次提交压力测试。最终命令、日志和构建指纹位于target/qa/candidate-save-fix-2026-09-14/。

最终检查通过：NR-504单项复核；Rust workspace 318项通过、4项既有忽略入口、1项明确排除压力测试；Rust格式、Clippy、生产边界和NR-603专项通过。TypeScript/Vite及Tauri Release构建、隐藏启动主窗口检查均通过。

Release：`target/release/novelgainker-desktop.exe`，19911168字节；SHA256：`24889ea28a121d3c6b9d9e8f24eaaa63e25c8e44e45c8ef927d60d73aa64bd30`。源码范围指纹：`db713b5e75710ef63902128246379ecc80da09a734ef67326f3c94b7d430bce8`。此次EXE替代上次同路径构建，旧构建哈希仅保留在此前记录中。用户可在原会话重测保存，不必删除任何小说或聊天；尚未执行真实Provider验收。
