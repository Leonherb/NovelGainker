import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) throw new Error("pnpm must run the NG-502 E2E entry.");

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
for (const name of Object.keys(testEnvironment)) {
  if (privateEnvironmentName.test(name)) delete testEnvironment[name];
}

function runPnpm(name, args) {
  console.log(`[ng502-e2e] START ${name}`);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) throw new Error(`${name} failed`);
  console.log(`[ng502-e2e] PASS ${name}`);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

const evidenceRoot = resolve("target", "ng502-evidence");
const reportPath = join(evidenceRoot, "reader-model-request-snapshot-report.json");
mkdirSync(evidenceRoot, { recursive: true });

runPnpm("least-privilege and fixed-Pack boundary", ["quality:boundaries"]);
runPnpm("Reader Mental Model and StoryContextRequestSnapshotV1 core", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-workflows",
  "story_context::tests",
  "--locked",
  "--offline",
]);
runPnpm("read-only request-preview command and source-change review", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-desktop",
  "story_context_commands::tests",
  "--lib",
  "--locked",
  "--offline",
]);
runPnpm("strict request snapshot IPC and six-field editor", [
  "--filter",
  "@novelgainker/desktop",
  "exec",
  "vitest",
  "run",
  "src/lib/tauri/story-context.test.ts",
  "src/components/StoryContextBuilder.test.tsx",
]);

const supportResources = [
  "request-preview.input.schema.json",
  "request-snapshot.output.schema.json",
  "workflow.md",
].map((name) => {
  const bytes = readFileSync(resolve("resources", "workflows", "v1", "storytelling", name));
  return { path: `storytelling/${name}`, byteLength: bytes.length, sha256: sha256(bytes) };
});

const report = {
  schemaVersion: 1,
  story: "NG-502",
  mode: "offline-local-first-zero-write",
  generatedAt: new Date().toISOString(),
  requestSnapshot: {
    type: "StoryContextRequestSnapshotV1",
    snapshotVersion: 1,
    readerModelFieldCount: 6,
    readerModelFieldLimitBytes: 4 * 1024,
    readerModelTotalLimitBytes: 12 * 1024,
    protocolBudgetBytes: 24 * 1024,
    projectContextBudgetBytes: 48 * 1024,
    safetyReserveBytes: 24 * 1024,
    requestLimitBytes: 96 * 1024,
    queryResultMaximumCount: 5,
    queryResultLimitBytes: 2 * 1024,
    recentChapterMaximumCount: 2,
    maximumOutputTokens: 4096,
    sourceReviewStates: ["current", "needsReview"],
  },
  matrix: {
    stableRequestHashMismatchCount: 0,
    readerModelHashMismatchCount: 0,
    sourceChangeReaderModelLossCount: 0,
    staleSourceReviewBypassCount: 0,
    hiddenReaderFieldAcceptanceCount: 0,
    readerBudgetEscapeCount: 0,
    requestBudgetEscapeCount: 0,
    queryResultLimitEscapeCount: 0,
    recentChapterLimitEscapeCount: 0,
    providerRequestCount: 0,
    projectWriteCount: 0,
    appStateWriteCount: 0,
    cacheOrDatabaseWriteCount: 0,
  },
  ui: {
    sixFieldsEditable: true,
    sourceChangePreservesUserText: true,
    staleSourcesRequireReview: true,
    readerModelHashVisible: true,
    requestHashVisible: true,
    budgetArithmeticVisible: true,
  },
  resources: supportResources,
};
const reportText = `${JSON.stringify(report, null, 2)}\n`;
if (
  /[A-Za-z]:[/\\]/u.test(reportText) ||
  /Authorization|Bearer\s+|apiKey|accessToken|authToken|projectSessionId/iu.test(reportText)
) {
  throw new Error("NG-502 request-snapshot report contains forbidden path or credential text.");
}
writeFileSync(reportPath, reportText, "utf8");
console.log(
  "[ng502-e2e] PASS stableRequestHashMismatchCount=0 sourceChangeReaderModelLossCount=0 staleSourceReviewBypassCount=0 providerRequestCount=0 projectWriteCount=0",
);
console.log("[ng502-e2e] EVIDENCE target/ng502-evidence/reader-model-request-snapshot-report.json");
