import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const root = process.cwd();
const stories = ["ng-r101", "ng-r102", "ng-r103", "ng-r104", "ng-r105", "ng-r106"];
const evidence = [];
for (const story of stories) {
  const source = await readFile(
    path.join(root, "target", "qa", story, `${story}-evidence.json`),
    "utf8",
  );
  const parsed = JSON.parse(source);
  if (parsed.passed !== true) throw new Error(`${story} evidence is not passing`);
  evidence.push({
    story: story.toUpperCase(),
    sha256: createHash("sha256").update(source).digest("hex"),
  });
}
const retirementSource = await readFile(
  path.join(root, "target", "qa", "ng-r106", "v1-retirement-manifest.json"),
  "utf8",
);
const retirement = JSON.parse(retirementSource);
if (retirement.passed !== true || retirement.items.some((item) => item.disposition !== "deleted")) {
  throw new Error("NG-R106 retirement manifest is incomplete");
}
const contractSet = JSON.parse(
  await readFile(
    path.join(root, "resources", "contracts", "agent-v2", "v2", "contract-set.json"),
    "utf8",
  ),
);
const commandSource = await readFile(
  path.join(root, "apps", "desktop", "src-tauri", "src", "command_manifest.rs"),
  "utf8",
);
const commands = [...commandSource.matchAll(/"([a-z][a-z0-9_]*)"/gu)].map((match) => match[1]);
if (commands.length !== 35) throw new Error("post-GAR1 production command count drifted");
const currentAgentCommands = commands.filter((command) => command.startsWith("agent_"));
if (currentAgentCommands.length !== 14) throw new Error("post-GAR1 Agent IPC count drifted");
const directory = path.join(root, "target", "qa", "gar1");
await mkdir(directory, { recursive: true });
const manifest = {
  schemaVersion: 1,
  gate: "GAR1",
  submittedAt: new Date().toISOString(),
  storyEvidence: evidence,
  agentV2ContractSetSha256: contractSet.contractSetSha256,
  gar1BaselineProductionCommandCount: 28,
  gar1BaselineProductionAgentIpcCount: 7,
  currentProductionCommandCount: commands.length,
  currentProductionAgentIpcCount: currentAgentCommands.length,
  agentCanonWriteCount: 0,
  dualProviderLoopback: ["deepseek", "openai"],
  realProviderLiveSmokeExecuted: false,
  technicalSubmissionReady: true,
  gar1Approved: false,
  productApprovalRequired: true,
  subsequentAmR2ImplementationPresent: true,
  amR3Authorized: false,
};
await writeFile(
  path.join(directory, "gar1-manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
);
const report = `# GAR1 技术提交评审报告

状态：**技术证据完成，等待产品评审；gar1Approved=false。**

- NG-R101～NG-R106 的机器可读证据均通过并锁定 Hash。
- GAR1 历史基线为 28 条 Command / 7 条 Agent IPC；当前经已批准 AM-R2 实施后为 35 / 14，本报告不把当前面误报为 GAR1 基线。
- Conversation、Control、Context、Canon 与 Persistence 已分面；AM-R1 Agent Canon 写入为 0。
- 完整 Novelist-compatible bundled Skill Pack、项目内 History/Memory、同步分支决议与 50 万字 Context 规模门已通过。
- DeepSeek/OpenAI 仅执行确定性 Loopback；未读取真实凭据，未执行真实 Provider Live Smoke。
- 旧 AM1 产品层与 AM2 产品 Wrapper 已按 Retirement Manifest 退休；写入、事务、Undo 与 Canon/连续性负向回归保留在安全 Kernel。
- 本历史提交本身不批准 GAR1；后续 AM-R2 授权不由本清单反向推导。本报告不授权 AM-R3、Sprint 6 或真实网络验证。
`;
await writeFile(path.join(directory, "gar1-review-report.md"), report);
console.log("[e2e:gar1] PASS technical submission generated; gar1Approved=false");
