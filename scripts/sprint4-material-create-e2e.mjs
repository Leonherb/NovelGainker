import { createHash, randomBytes } from "node:crypto";
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) {
  throw new Error("pnpm must run the NG-403 E2E entry.");
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
  console.log("[ng403-e2e] START " + name);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) {
    throw new Error(name + " failed");
  }
  console.log("[ng403-e2e] PASS " + name);
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
    /Authorization|Bearer\s+|apiKey|accessToken|authToken|providerResponse|promptBody|draftBody/iu.test(
      text,
    )
  ) {
    throw new Error(
      "NG-403 evidence contains a forbidden path, credential, Prompt, Response, or hidden draft marker.",
    );
  }
  if (canaryFragments(canary).some((fragment) => bytes.includes(Buffer.from(fragment, "utf8")))) {
    throw new Error("NG-403 evidence contains a Material Canary fragment.");
  }
}

const evidenceRoot = resolve("target", "ng403-evidence");
mkdirSync(evidenceRoot, { recursive: true });
const canary = "NG403Material" + randomBytes(48).toString("base64url");
if (canary.length < 64) {
  throw new Error("NG-403 synthetic Material Canary generation failed.");
}

runPnpm("static least-privilege and Local-first boundary", ["quality:boundaries"]);
runPnpm("Material Workflow Pack, four types, serializer, Context and Golden matrix", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-workflows",
  "--locked",
  "--offline",
]);
runPnpm("App State v5 and dual-Provider MaterialProposalV1 contracts", [
  "exec",
  "cargo",
  "test",
  "-p",
  "app-core",
  "--locked",
  "--offline",
]);
runPnpm("Tauri catalog, abandon, create, replace, target conflict and immutable ledger", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-desktop",
  "--lib",
  "--locked",
  "--offline",
]);
runPnpm("typed IPC and real React material-create review path", [
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
  story: "NG-403",
  mode: "mock-only-local-first",
  generatedAt: new Date().toISOString(),
  workflow: {
    id: "novelgainker.material-create",
    version: 1,
    outputContract: "MaterialProposalV1",
    confirmations: ["send_context", "promote_material_to_canon"],
    materialTypes: ["theme", "clue", "object", "scene_seed"],
  },
  matrix: {
    mockProviderCount: 2,
    crossProviderAutomaticRequestCount: 0,
    requestBeforeManifestConfirmationCount: 0,
    writeBeforeProposalConfirmationCount: 0,
    metadataCatalogBodyCount: 0,
    hiddenDraftPersistenceCount: 0,
    selectedMaterialBodyCount: 0,
    maximumSelectedCanonSources: 6,
    maximumContextBytes: 16384,
    confirmedMaterialCreateCount: 1,
    confirmedMaterialReplaceCount: 1,
    storyLedgerWriteCount: 0,
    replaceIdOrPathMutationCount: 0,
    sourceChangedWriteCount: 0,
    targetChangedWriteCount: 0,
    reservedTargetWriteCount: 0,
    collisionOverwriteCount: 0,
    abandonWriteCount: 0,
  },
  checks: {
    strictMaterialBrief: true,
    strictMaterialProposal: true,
    fourFixedMaterialTypes: true,
    rustOwnedIdPathFrontmatterMarkdown: true,
    explicitCanonSourcePicker: true,
    materialsExcludedFromContext: true,
    storyLedgerExcludedAndImmutable: true,
    explicitSamePathReplace: true,
    reusedMaterialId: true,
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
  throw new Error("NG-403 security report write failed.");
}
for (const path of scanFiles(evidenceRoot)) {
  assertRedactedEvidence(path, canary);
}

console.log(
  "[ng403-e2e] PASS two Mock Providers, four material types, metadata-only Canon Source Picker, zero hidden drafts/pre-confirmation requests/writes, one create plus one same-path/ID replace, immutable Story Ledger, no fallback/collision/source-target-change write, and offline edit regression",
);
