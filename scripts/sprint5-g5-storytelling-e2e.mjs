import { createHash, randomBytes } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) throw new Error("pnpm must run the NG-507/G5 E2E entry.");

const qualitySequenceMode = process.argv.includes("--quality-sequence");
const qualityStartedAt = process.env.NOVELGAINKER_QUALITY_STARTED_AT;
if (qualitySequenceMode && !qualityStartedAt)
  throw new Error("G5 quality-sequence mode requires the parent quality start marker.");

const generatedAt = qualitySequenceMode ? qualityStartedAt : new Date().toISOString();
const privateEnvironmentName =
  /(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu;
const testEnvironment = {
  ...process.env,
  CARGO_NET_OFFLINE: "true",
  HTTP_PROXY: "http://127.0.0.1:9",
  HTTPS_PROXY: "http://127.0.0.1:9",
  ALL_PROXY: "http://127.0.0.1:9",
  NO_PROXY: "127.0.0.1,localhost",
};
for (const name of Object.keys(testEnvironment))
  if (privateEnvironmentName.test(name)) delete testEnvironment[name];

const canary = "NG507" + randomBytes(20).toString("hex") + randomBytes(20).toString("base64url");
testEnvironment.NG507_TEST_CANARY = canary;
const expectedPackHash = "1658685537a9fe57b441928816e0fd2af770109e9b9a12a985204774496246a5";
const evidenceRoot = resolve("target", "g5-evidence");
const screenshotPath = join(evidenceRoot, "tauri-g5-window.png");
mkdirSync(evidenceRoot, { recursive: true });

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function runPnpm(name, args) {
  console.log("[ng507-g5] START " + name);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) throw new Error(name + " failed");
  console.log("[ng507-g5] PASS " + name);
}

function runPnpmCaptured(name, args) {
  console.log("[ng507-g5] START " + name);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    encoding: "utf8",
  });
  const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  if (result.error || result.status !== 0) {
    const sanitized = output.replaceAll(canary, "[REDACTED]");
    console.error(sanitized);
    throw new Error(name + " failed");
  }
  assert(!output.includes(canary), name + " leaked the credential Canary.");
  console.log("[ng507-g5] PASS " + name);
  return output;
}

function evidenceLines(output, marker) {
  return output
    .split(/\r?\n/u)
    .filter((line) => line.includes(marker))
    .map((line) => JSON.parse(line.slice(line.indexOf(marker) + marker.length)));
}

function assertHash(value, label) {
  assert(/^[0-9a-f]{64}$/u.test(value), label + " is not SHA-256.");
}

function writeJson(name, value) {
  const text = JSON.stringify(value, null, 2) + "\n";
  assertRedacted(text, name);
  const path = join(evidenceRoot, name);
  writeFileSync(path, text, "utf8");
  return path;
}

function assertRedacted(text, label) {
  assert(!text.includes(canary), label + " contains the credential Canary.");
  assert(!/[A-Za-z]:[/\\]/u.test(text), label + " contains an absolute Windows path.");
  assert(
    !/Authorization|Bearer\s+|apiKey|accessToken|authToken|credentialRef|promptBody|responseBody|contextBody|proposalBody|chapterBody|ledgerBody/iu.test(
      text,
    ),
    label + " contains a credential or private-body marker.",
  );
}

function assertPng(path) {
  assert(existsSync(path) && statSync(path).isFile(), "G5 desktop screenshot is missing.");
  const bytes = readFileSync(path);
  const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  assert(bytes.length >= 4096, "G5 desktop screenshot is too small.");
  assert(bytes.subarray(0, 8).equals(signature), "G5 desktop screenshot is not PNG.");
  assert(!bytes.includes(Buffer.from(canary, "utf8")), "G5 screenshot contains the Canary.");
  return bytes;
}

runPnpm("least-privilege and fixed-Pack boundary", ["quality:boundaries"]);
const rustOutput = runPnpmCaptured("dual-Provider Storytelling transaction matrix", [
  "exec",
  "cargo",
  "test",
  "-p",
  "app-core",
  "ng_507",
  "--locked",
  "--offline",
  "--",
  "--nocapture",
]);
runPnpm("strict Continuity/Apply/Lifecycle UI path", [
  "--filter",
  "@novelgainker/desktop",
  "exec",
  "vitest",
  "run",
  "src/lib/tauri/story-continuity.test.ts",
  "src/lib/tauri/story-chapter-patch.test.ts",
  "src/lib/tauri/story-lifecycle.test.ts",
  "src/components/StoryContextBuilder.test.tsx",
]);
if (!qualitySequenceMode) runPnpm("release desktop build", ["build:test"]);
runPnpm("redacted desktop capture", ["smoke:g5"]);

const providerEvidence = evidenceLines(rustOutput, "NG507_EVIDENCE:");
const negativeEvidence = evidenceLines(rustOutput, "NG507_NEGATIVE:");
assert(providerEvidence.length === 2, "G5 requires exactly two Provider success records.");
assert(negativeEvidence.length === 2, "G5 requires exactly two Provider negative records.");
providerEvidence.sort((left, right) => left.providerId.localeCompare(right.providerId));
negativeEvidence.sort((left, right) => left.providerId.localeCompare(right.providerId));
assert(
  providerEvidence.map((item) => item.providerId).join(",") === "deepseek,openai",
  "G5 Provider success matrix is incomplete.",
);
assert(
  negativeEvidence.map((item) => item.providerId).join(",") === "deepseek,openai",
  "G5 Provider negative matrix is incomplete.",
);

const sharedFields = [
  "stateMachineVersion",
  "stateSequence",
  "manifestHash",
  "requestHash",
  "outputHash",
  "createdChapterHash",
  "manualChapterHash",
  "revisedChapterHash",
  "continuityUnknownCount",
  "unknownsConfirmed",
  "providerRequestCount",
  "automaticProjectWriteCount",
  "statusConfirmationCount",
  "ledgerConfirmationCount",
  "staleOverwriteCount",
];
for (const field of sharedFields)
  assert(
    JSON.stringify(providerEvidence[0][field]) === JSON.stringify(providerEvidence[1][field]),
    "Provider-neutral state machine mismatch: " + field,
  );
for (const item of providerEvidence) {
  for (const field of [
    "manifestHash",
    "requestHash",
    "outputHash",
    "createProposalHash",
    "reviseProposalHash",
    "createdChapterHash",
    "manualChapterHash",
    "revisedChapterHash",
    "acceptedChapterHash",
    "ledgerHash",
  ])
    assertHash(item[field], item.providerId + "." + field);
  assert(item.automaticProjectWriteCount === 0, "Automatic project write detected.");
  assert(item.staleOverwriteCount === 0, "Manual edit was overwritten by stale Proposal.");
  assert(item.statusConfirmationCount === 1, "Status confirmation contract changed.");
  assert(item.ledgerConfirmationCount === 1, "Ledger confirmation contract changed.");
  assert(item.continuityUnknownCount === 2, "Unknown results were reclassified.");
  assert(item.unknownsConfirmed === true, "Author confirmation for unknown results is missing.");
}
for (const item of negativeEvidence) {
  for (const field of [
    "cancelledProjectWriteCount",
    "truncatedOutputWriteCount",
    "invalidJsonWriteCount",
    "providerErrorWriteCount",
    "sourceStaleWriteCount",
  ])
    assert(item[field] === 0, item.providerId + "." + field + " must be zero.");
  assert(item.chapterExists === false, "Negative path created a chapter.");
  assert(item.ledgerUnchanged === true, "Negative path changed the Ledger.");
}

const providerReportNames = {
  deepseek: "deepseek-storytelling-loopback-report.json",
  openai: "openai-storytelling-loopback-report.json",
};
const providerReports = providerEvidence.map((item) =>
  writeJson(providerReportNames[item.providerId], {
    schemaVersion: 1,
    story: "NG-507",
    generatedAt,
    mode: "offline-loopback",
    ...item,
    liveRequestCount: 0,
  }),
);
const chapterReportPath = writeJson("chapter-transaction-report.json", {
  schemaVersion: 1,
  story: "NG-507",
  generatedAt,
  createAndRevisePassed: true,
  manualEditProtectedFromStaleProposal: true,
  providers: providerEvidence.map((item) => ({
    providerId: item.providerId,
    createdChapterHash: item.createdChapterHash,
    manualChapterHash: item.manualChapterHash,
    revisedChapterHash: item.revisedChapterHash,
    acceptedChapterHash: item.acceptedChapterHash,
    staleOverwriteCount: item.staleOverwriteCount,
  })),
});
const ledgerReportPath = writeJson("ledger-transaction-report.json", {
  schemaVersion: 1,
  story: "NG-507",
  generatedAt,
  independentTransactions: true,
  providers: providerEvidence.map((item) => ({
    providerId: item.providerId,
    acceptedChapterHash: item.acceptedChapterHash,
    ledgerHash: item.ledgerHash,
    statusConfirmationCount: item.statusConfirmationCount,
    ledgerConfirmationCount: item.ledgerConfirmationCount,
  })),
});
const cancellationReportPath = writeJson("cancellation-and-recovery-report.json", {
  schemaVersion: 1,
  story: "NG-507",
  generatedAt,
  providers: negativeEvidence,
  recoveryRequiredCount: 0,
  backgroundRetryCount: 0,
});
const liveReportPath = writeJson("live-smoke-report.json", {
  schemaVersion: 1,
  story: "NG-507",
  generatedAt,
  status: "not-authorized",
  blocking: false,
  providerRequestCount: 0,
  projectWriteCount: 0,
});

const screenshotBytes = assertPng(screenshotPath);
const first = providerEvidence[0];
const reviewText = `# G5 Review Report\n\n- 状态：NG-501～NG-507 均已获产品验收；G5 Review Gate 技术复核完成，等待产品批准。\n- 模式：DeepSeek/OpenAI 离线 Loopback；Live Smoke 未获授权且不阻断。\n- Workflow Pack Hash：${expectedPackHash}\n- Context Manifest Hash：${first.manifestHash}\n- Request Hash：${first.requestHash}\n- 结构化 Output Hash：${first.outputHash}\n- 创建章节 Hash：${first.createdChapterHash}\n- 手动编辑章节 Hash：${first.manualChapterHash}\n- 修订章节 Hash：${first.revisedChapterHash}\n- unknown：保留 2 项原分类，作者显式确认后解锁；未伪装为 passed。\n- 写入边界：取消、截断、解析失败、Provider 错误、来源过期均为零项目写入。\n- 事务边界：章节正文、accepted 状态、Story Ledger 分离；状态与 Ledger 各一次确认。\n- 评审结论：全部 G5 阻断项均有证据，建议产品批准 G5；本报告不代替产品批准。\n`;
assertRedacted(reviewText, "g5-review-report.md");
const reviewPath = join(evidenceRoot, "g5-review-report.md");
writeFileSync(reviewPath, reviewText, "utf8");

const artifactPaths = [
  ...providerReports,
  chapterReportPath,
  ledgerReportPath,
  cancellationReportPath,
  liveReportPath,
  reviewPath,
  screenshotPath,
];
const manifestPath = writeJson("g5-manifest.json", {
  schemaVersion: 1,
  gate: "G5",
  story: "NG-507",
  generatedAt,
  status: "review-complete-product-approval-pending",
  workflowPackHash: expectedPackHash,
  contextManifestHash: first.manifestHash,
  requestHash: first.requestHash,
  outputHash: first.outputHash,
  chapterHash: first.revisedChapterHash,
  ledgerHashes: providerEvidence.map((item) => ({
    providerId: item.providerId,
    sha256: item.ledgerHash,
  })),
  liveSmoke: { status: "not-authorized", blocking: false },
  artifacts: artifactPaths.map((path) => ({
    name: path.slice(evidenceRoot.length + 1).replaceAll("\\", "/"),
    byteLength: statSync(path).size,
    sha256: sha256(readFileSync(path)),
  })),
  screenshotSha256: sha256(screenshotBytes),
});

for (const path of [...artifactPaths, manifestPath]) {
  const bytes = readFileSync(path);
  assert(!bytes.includes(Buffer.from(canary, "utf8")), "G5 artifact contains the Canary.");
  if (path.endsWith(".json") || path.endsWith(".md")) assertRedacted(bytes.toString("utf8"), path);
}

console.log(
  "[ng507-g5] PASS providers=2 stateMachine=shared cancelWrites=0 staleOverwrites=0 live=not-authorized",
);
console.log("[ng507-g5] EVIDENCE target/g5-evidence/g5-manifest.json");
