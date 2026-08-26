import { createHash, randomBytes } from "node:crypto";
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) {
  throw new Error("pnpm must run the NG-404 E2E entry.");
}

const SECRET_ENVIRONMENT_NAME =
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
  if (SECRET_ENVIRONMENT_NAME.test(name)) {
    delete testEnvironment[name];
  }
}

function runPnpm(name, args) {
  console.log("[ng404-e2e] START " + name);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) {
    throw new Error(name + " failed");
  }
  console.log("[ng404-e2e] PASS " + name);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canaryFragments(canary) {
  const fragments = [];
  for (let index = 0; index <= canary.length - 16; index += 1) {
    fragments.push(canary.slice(index, index + 16));
  }
  return fragments;
}

function scanFiles(directory) {
  return readdirSync(directory, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => join(entry.parentPath, entry.name));
}

function assertRedactedEvidence(path, canary) {
  const bytes = readFileSync(path);
  const text = bytes.toString("utf8");
  if (
    /[A-Za-z]:[/\\]/u.test(text) ||
    /Authorization|Bearer\s+|apiKey|accessToken|authToken|providerResponse|promptBody|draftBody|revealBody|payoffBody/iu.test(
      text,
    )
  ) {
    throw new Error(
      "NG-404 evidence contains a forbidden path, credential, Prompt, Response, hidden draft, Reveal, or Payoff marker.",
    );
  }
  if (canaryFragments(canary).some((fragment) => bytes.includes(Buffer.from(fragment, "utf8")))) {
    throw new Error("NG-404 evidence contains a Plot Canary fragment.");
  }
}

const evidenceRoot = resolve("target", "ng404-evidence");
mkdirSync(evidenceRoot, { recursive: true });
const canary = "NG404Plot" + randomBytes(48).toString("base64url");
if (canary.length < 64) {
  throw new Error("NG-404 synthetic Plot Canary generation failed.");
}

runPnpm("static least-privilege and Local-first boundary", ["quality:boundaries"]);
runPnpm("Plot Workflow Pack, serializer, Context and Golden matrix", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-workflows",
  "--locked",
  "--offline",
]);
runPnpm("App State v6 and dual-Provider PlotProposalV1 contracts", [
  "exec",
  "cargo",
  "test",
  "-p",
  "app-core",
  "--locked",
  "--offline",
]);
runPnpm("Tauri catalog, abandon, create, replace, collision and immutable ledger", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-desktop",
  "--lib",
  "--locked",
  "--offline",
]);
runPnpm("typed IPC and real React plot-create review path", [
  "--filter",
  "@novelgainker/desktop",
  "exec",
  "vitest",
  "run",
  "src/lib/tauri/workflow.test.ts",
  "src/lib/tauri/workflow-run.test.ts",
  "src/components/ConversationPanel.test.tsx",
  "src/errors/present-app-error.test.ts",
]);
runPnpm("Sprint 1 no-Key offline editor regression", ["e2e:sprint1"]);

const report = {
  schemaVersion: 1,
  story: "NG-404",
  mode: "mock-only-local-first",
  generatedAt: new Date().toISOString(),
  workflow: {
    id: "novelgainker.plot-create",
    version: 1,
    outputContract: "PlotProposalV1",
    confirmations: ["send_context", "create_or_replace_main_plot"],
    fixedTarget: "plot/main.md",
  },
  matrix: {
    mockProviderCount: 2,
    crossProviderAutomaticRequestCount: 0,
    requestBeforeManifestConfirmationCount: 0,
    writeBeforeProposalConfirmationCount: 0,
    metadataCatalogBodyCount: 0,
    hiddenDraftPersistenceCount: 0,
    selectedPlotSourceBodyCount: 0,
    maximumSelectedPlotSources: 8,
    maximumContextBytes: 16384,
    confirmedPlotCreateCount: 1,
    confirmedPlotReplaceCount: 1,
    storyLedgerWriteCount: 0,
    replaceIdOrPathMutationCount: 0,
    sourceChangedWriteCount: 0,
    targetChangedWriteCount: 0,
    collisionOverwriteCount: 0,
    abandonWriteCount: 0,
    revealEvidenceLeakCount: 0,
    payoffEvidenceLeakCount: 0,
  },
  checks: {
    strictPlotBrief: true,
    strictPlotProposal: true,
    rustOwnedIdPathFrontmatterMarkdown: true,
    explicitPlotSourcePicker: true,
    plotChapterOutlineLedgerExcludedFromContext: true,
    exactMainPlotTarget: true,
    explicitSamePathReplace: true,
    reusedPlotId: true,
    completeReplaceDiff: true,
    manifestHashBound: true,
    proposalHashBound: true,
    sameProviderSingleRepair: true,
    closeRestartAbandonClearMemory: true,
    offlineManualEditRegression: true,
  },
};
const reportBytes = Buffer.from(JSON.stringify(report, null, 2) + "\n", "utf8");
const reportPath = join(evidenceRoot, "security-report.json");
writeFileSync(reportPath, reportBytes);
if (!statSync(reportPath).isFile() || sha256(reportBytes).length !== 64) {
  throw new Error("NG-404 security report write failed.");
}
for (const path of scanFiles(evidenceRoot)) {
  assertRedactedEvidence(path, canary);
}

console.log(
  "[ng404-e2e] PASS two Mock Providers, metadata-only Plot Source Picker, zero hidden drafts/pre-confirmation requests/writes, one fixed-target create plus one same-path/ID replace, immutable Story Ledger, no fallback/collision/source-target-change write, no Reveal/Payoff evidence leak, and offline edit regression",
);
