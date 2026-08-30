import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const root = process.cwd();
const storyIds = [
  "ng-r201",
  "ng-r202",
  "ng-r203",
  "ng-r204",
  "ng-r205",
  "ng-r206",
  "ng-r207",
  "ng-r208",
];
const storyEvidence = [];
for (const storyId of storyIds) {
  const source = await readFile(
    path.join(root, "target", "qa", storyId, `${storyId}-evidence.json`),
    "utf8",
  );
  const parsed = JSON.parse(source);
  if (parsed.passed !== true) throw new Error(`${storyId} evidence is not passing`);
  storyEvidence.push({
    story: storyId.toUpperCase(),
    sha256: createHash("sha256").update(source).digest("hex"),
  });
}

const scale = JSON.parse(
  await readFile(path.join(root, "target", "qa", "ng-r208", "ng-r208-evidence.json"), "utf8"),
);
if (
  scale.fixture.chineseCharacterCount < 500_000 ||
  scale.fixture.writeScenarioCount < 72 ||
  scale.correctness.truthHits !== scale.fixture.truthQueryCount ||
  scale.boundaries.agentCanonWriteOutsideApprovedScope !== 0 ||
  scale.boundaries.partialCommitCount !== 0 ||
  scale.boundaries.humanEditOverwriteCount !== 0 ||
  scale.boundaries.externalNetworkCalls !== 0 ||
  scale.boundaries.realCredentialReads !== 0 ||
  scale.budgetObservation.compositionCalls !== 0 ||
  scale.budgetObservation.segmentCount !== 0 ||
  scale.lockedGar2CanonProfile.maxCompositionSegments !== 0
) {
  throw new Error("NG-R208 scale or isolation threshold drifted");
}
const headroomPairs = [
  [scale.budgetObservation.providerCallsP99, scale.lockedGar2CanonProfile.maxProviderCalls],
  [
    scale.budgetObservation.providerInputTokensP99,
    scale.lockedGar2CanonProfile.maxTotalProviderInputTokens,
  ],
  [
    scale.budgetObservation.providerOutputTokensP99,
    scale.lockedGar2CanonProfile.maxTotalProviderOutputTokens,
  ],
  [
    scale.budgetObservation.contextResultBytesP99,
    scale.lockedGar2CanonProfile.maxAccumulatedContextResultBytes,
  ],
  [scale.budgetObservation.canonWritesP99, scale.lockedGar2CanonProfile.maxCanonWrites],
];
if (headroomPairs.some(([observed, maximum]) => Math.ceil(observed * 1.25) > maximum)) {
  throw new Error("GAR2 locked budget lacks the required 25% p99 headroom");
}

const retirementSource = await readFile(
  path.join(root, "target", "qa", "ng-r208", "gar2-retirement-manifest.json"),
  "utf8",
);
const retirement = JSON.parse(retirementSource);
if (
  retirement.passed !== true ||
  retirement.oldAgentIpcReachable !== false ||
  retirement.items.some((item) => !["deleted", "retained_kernel"].includes(item.disposition))
) {
  throw new Error("NG-R208 retirement manifest is incomplete");
}

const contractSet = JSON.parse(
  await readFile(
    path.join(root, "resources", "contracts", "agent-v2", "v2", "contract-set.json"),
    "utf8",
  ),
);
if (contractSet.resources.length !== 6) throw new Error("GAR2 contract resource count drifted");
const commandSource = await readFile(
  path.join(root, "apps", "desktop", "src-tauri", "src", "command_manifest.rs"),
  "utf8",
);
const commands = [...commandSource.matchAll(/"([a-z][a-z0-9_]*)"/gu)].map((match) => match[1]);
const agentCommands = commands.filter((command) => command.startsWith("agent_"));
if (commands.length !== 35 || agentCommands.length !== 14) {
  throw new Error("GAR2 production command surface drifted");
}

const directory = path.join(root, "target", "qa", "gar2");
await mkdir(directory, { recursive: true });
const manifest = {
  schemaVersion: 1,
  gate: "GAR2",
  submittedAt: new Date().toISOString(),
  storyEvidence,
  agentV2ContractSetSha256: contractSet.contractSetSha256,
  agentV2ContractResourceCount: contractSet.resources.length,
  productionCommandCount: commands.length,
  productionAgentIpcCount: agentCommands.length,
  dualProviderLoopback: ["deepseek", "openai"],
  scale: {
    chineseCharacterCount: scale.fixture.chineseCharacterCount,
    writeScenarioCount: scale.fixture.writeScenarioCount,
    truthQueryCount: scale.fixture.truthQueryCount,
    agentCanonWriteCount: scale.boundaries.agentCanonWriteCount,
    queryP99Ms: scale.performance.queryP99Ms,
    commitP99Ms: scale.performance.commitP99Ms,
  },
  lockedGar2CanonProfile: scale.lockedGar2CanonProfile,
  compositionCalls: 0,
  segmentCount: 0,
  agentCanonWriteOutsideApprovedScope: 0,
  externalNetworkCalls: 0,
  realCredentialReads: 0,
  realProviderLiveSmokeExecuted: false,
  technicalSubmissionReady: true,
  gar2Approved: false,
  productApprovalRequired: true,
  amR3Authorized: false,
  sprint6Authorized: false,
};
await writeFile(
  path.join(directory, "gar2-manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
);
const report = `# GAR2 技术提交评审报告

状态：**NG-R201～NG-R208 技术证据完成，等待产品评审；gar2Approved=false。**

- Conversation → Anchor → Canon/Chapter Commit 已在三栏界面、Host 协议、冲突决议、WritePlan、事务、Undo 与 Context 刷新上闭环。
- 当前生产面精确为 35 条 Command，其中 14 条 Agent Protocol V2 IPC；旧 Agent IPC 不可达。
- 50 万字以上、72 个写入场景与双 Provider 确定性 Loopback 已通过；预算均保留至少 25% p99 余量。
- Canon 组合调用与分段均为 0；批准范围外写入、部分提交、覆盖人工编辑、外网调用与真实凭据读取均为 0。
- 本提交不自行批准 GAR2，不授权 AM-R3、Sprint 6 或真实 Provider Live Smoke。
`;
await writeFile(path.join(directory, "gar2-review-report.md"), report);
console.log("[e2e:gar2] PASS technical submission generated; gar2Approved=false");
