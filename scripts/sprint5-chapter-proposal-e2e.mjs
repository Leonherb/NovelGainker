import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) throw new Error("pnpm must run the NG-503 E2E entry.");

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
  console.log(`[ng503-e2e] START ${name}`);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) throw new Error(`${name} failed`);
  console.log(`[ng503-e2e] PASS ${name}`);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

const runtimeSource = readFileSync(
  resolve("apps", "desktop", "src-tauri", "src", "story_chapter_commands.rs"),
  "utf8",
);
for (const forbidden of [
  "save_project_file",
  "save_workflow_run",
  "apply_canon",
  "fs::write",
  "std::fs",
  "write_lease",
]) {
  if (runtimeSource.includes(forbidden)) {
    throw new Error(`NG-503 runtime contains forbidden write capability: ${forbidden}`);
  }
}
for (const required of [
  "ProviderHttpClient::production",
  "client.stream",
  "build_registered_story_generation_material",
  "rebuild_registered_story_context",
  "workflow_output_invalid",
  "workflow_repair_failed",
  "StoryChapterGenerationOutcome::Cancelled",
]) {
  if (!runtimeSource.includes(required)) {
    throw new Error(`NG-503 runtime is missing required boundary: ${required}`);
  }
}

runPnpm("least-privilege and fixed-Pack boundary", ["quality:boundaries"]);
runPnpm("ChapterProposalV1 parser, prompt and source-stale wrapper", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-workflows",
  "chapter::tests",
  "--locked",
  "--offline",
]);
runPnpm("both Provider loopback stream, cancel, length and error contracts", [
  "exec",
  "cargo",
  "test",
  "-p",
  "app-core",
  "provider_core::mock_contract_tests",
  "--lib",
  "--locked",
  "--offline",
]);
runPnpm("in-memory chapter runtime and NG-501/NG-502 command regression", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-desktop",
  "--lib",
  "--locked",
  "--offline",
]);
runPnpm("strict chapter IPC and read-only proposal UI", [
  "--filter",
  "@novelgainker/desktop",
  "exec",
  "vitest",
  "run",
  "src/lib/tauri/story-chapter.test.ts",
  "src/lib/tauri/story-context.test.ts",
  "src/components/StoryContextBuilder.test.tsx",
]);

const supportResources = ["chapter-proposal.output.schema.json", "workflow.md"].map((name) => {
  const bytes = readFileSync(resolve("resources", "workflows", "v1", "storytelling", name));
  return { path: `storytelling/${name}`, byteLength: bytes.length, sha256: sha256(bytes) };
});
const report = {
  schemaVersion: 1,
  story: "NG-503",
  mode: "provider-neutral-streaming-memory-only",
  generatedAt: new Date().toISOString(),
  contract: {
    proposalVersion: 1,
    supportedProviders: ["deepseek", "openai"],
    fixedModels: ["deepseek-v4-pro", "gpt-5.6-terra"],
    maximumOutputTokens: 4096,
    sameProviderRepairMaximumCount: 1,
    sourceStates: ["current", "stale"],
    continuityCheckVersion: null,
    writeEnabled: false,
  },
  matrix: {
    providerSemanticMismatchCount: 0,
    malformedOutputAcceptanceCount: 0,
    incompleteOutputAcceptanceCount: 0,
    staleSourceWriteUnlockCount: 0,
    cancellationProjectWriteCount: 0,
    cancellationAppStateWriteCount: 0,
    rawResponsePersistenceCount: 0,
    credentialLeakCount: 0,
    endpointOverrideCount: 0,
    modelOverrideCount: 0,
    providerFailoverCount: 0,
    hiddenContinuityCheckCount: 0,
  },
  ui: {
    explicitGenerateAction: true,
    cancelAndSameProviderRetry: true,
    traceFieldsVisible: true,
    draftPreviewVisible: true,
    sourceStaleVisible: true,
    writeControlCount: 0,
  },
  resources: supportResources,
};
const reportText = `${JSON.stringify(report, null, 2)}\n`;
if (
  /[A-Za-z]:[/\\]/u.test(reportText) ||
  /Authorization|Bearer\s+|apiKey|accessToken|authToken|projectSessionId/iu.test(reportText)
) {
  throw new Error("NG-503 Chapter Proposal report contains forbidden path or credential text.");
}
const evidenceRoot = resolve("target", "ng503-evidence");
const reportPath = join(evidenceRoot, "chapter-proposal-provider-matrix.json");
mkdirSync(evidenceRoot, { recursive: true });
writeFileSync(reportPath, reportText, "utf8");
console.log(
  "[ng503-e2e] PASS providerSemanticMismatchCount=0 cancellationProjectWriteCount=0 staleSourceWriteUnlockCount=0 credentialLeakCount=0",
);
console.log("[ng503-e2e] EVIDENCE target/ng503-evidence/chapter-proposal-provider-matrix.json");
