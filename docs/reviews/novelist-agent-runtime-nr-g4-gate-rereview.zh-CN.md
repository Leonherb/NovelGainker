# NR-G4 Gate 重新评审

## 0. 评审状态

- Gate：`NR-G4`
- 评审对象：已批准的 NR-3 Story Packet 与当前 NR-301～NR-306 实现
- 技术评审：**通过**
- 产品批准：**已批准**
- 批准日期：`2026-09-04`
- `nrG4Approved = true`
- 真实 Provider Live Smoke：**未执行、未授权**
- `NR-4～NR-7`：**未进入、未形成 Story Packet**

本次评审直接回到 NR-3 原始产品合同。当前实现中的有效行为与回归统一属于 NR-301～NR-306，不再拆分新的 Gate 后 Story、补丁报告或机器声明层级。

## 1. 评审依据

仅使用以下三个活动依据：

1. `docs/stories/novelist-agent-runtime-nr3-capability-canon-read-story-packet-draft-2-final.zh-CN.md`；
2. `docs/reviews/novelist-agent-runtime-nr-g4-technical-submission.zh-CN.md`；
3. `target/qa/nr-306/nr-g4-manifest.json`。

评审范围是用户可观察行为、安全边界和批准的资源预算，不把内部提交顺序、临时状态副本或额外证明机制提升为产品要求。

## 2. Gate 合同复核

| 合同 | 当前证据 | 结论 |
| --- | --- | --- |
| 自然聊天是默认路径 | 普通请求只执行一次主 Agent；Host 无关键词 Router、无用户可见 Skill 选择器 | pass |
| Agent 自主决定是否读取上下文 | 双 Provider 请求均获得相同能力目录和唯一 `novelist.read_context` Tool；Host 不预选 Capability | pass |
| Novelist Pack 自包含 | 9 个 Capability、36 个 Resource、来源与 Hash 校验、运行时外部加载为 0 | pass |
| Tool 合同封闭 | Provider 可见名称固定为 `novelist.read_context`，版本独立为 `1`，生产 Registry 精确为 1 个 Tool | pass |
| Provider Schema 可实际使用 | 唯一 Wire DTO、递归 Strict Schema、双 Adapter 请求体字段一致均有测试覆盖 | pass |
| Canon 只读且有界 | 固定 Allowlist、最多 8 个操作/8 个唯一文件、单文件 4 MiB、Canon 写入为 0 | pass |
| Context Bundle 有界可继续 | 24 KiB 实际 JSON 上限、最大转义组合、Read/Search/List/Resource 连续分页与快照变化反例通过 | pass |
| Tool 错误不破坏聊天 | 可配对的 malformed、非对象和超限参数生成失败 Task，并由 Attempt 2 继续自然回复 | pass |
| 请求总预算受控 | Tool Result 24 KiB、Continuation 48 KiB、Agent Loop Reserve 40 KiB、Provider Body 96 KiB | pass |
| 发布面未扩权 | 新 IPC 为 0、Host 关键词 Router 为 0、外部自动请求为 0 | pass |

## 3. 重新执行的证据

### 3.1 NR-306

`pnpm e2e:nr306`：PASS。

- Capability Pack：2/2；
- Canon Context：20/20；
- Provider Core：18/18；
- Conversation Runtime：19/19；
- Memory：11/11；
- Provider Evidence：仅 `127.0.0.1` Loopback。

### 3.2 完整质量门

`pnpm quality`：PASS。

- 前端：18 个测试文件、104 项测试通过；
- Rust Workspace：144 项通过、4 项按设计忽略；
- Debug/Release 构建：PASS；
- Debug/Release 桌面窗口 Smoke：PASS；
- Canon/Recovery Kernel：PASS；
- 真实 Provider 请求：0。

### 3.3 当前 Manifest 摘要

- Result：`pass`；
- Payload SHA-256：`3122bbfeb82545cfc22f43aed855e4f395b7587fa94c7f824f213864279bdc32`；
- Release Scope SHA-256：`4a84eb90227c375548acdf56bc9c36d57c3287a4fd04061d89e153ead218c1a7`；
- `canon_context.rs`：`302e7ce87333102ba931dd4d9f237157e377e0d2653d0e290331bbf241626df2`；
- `provider_core.rs`：`4974fac24a5da7f69ea813aff5340dd770092bf7c9f25df032ef102b5c6e007d`；
- `conversation_runtime.rs`：`777e0d63a69454eeb03ea5c9fee7c9273c7257ed472077d89ca696463a3e941c`。

## 4. Findings

本轮没有发现需要阻断 NR-G4 的实现缺陷，也不形成新的补救项。

仍有一个明确的非阻断边界：真实 DeepSeek/OpenAI 对 Tool Schema 与 Tool Call 行为的线上兼容性没有执行 Live Smoke。这不是当前授权范围，技术评审不把离线 Loopback 证据描述为真实 Provider 验证。

## 5. Gate 决定

产品负责人已批准 NR-G4 技术提交，确认 **NR-G4 通过**。

当前正式状态：

- `NR-G4.status = passed`；
- `NR-G4.reviewStatus = approved`；
- `nrG4Approved = true`。

流程停止于已通过的 NR-G4。该批准不授权真实 Provider Live Smoke，也不授权进入或形成 NR-4～NR-7 Story Packet；后续动作等待产品负责人另行授权。
