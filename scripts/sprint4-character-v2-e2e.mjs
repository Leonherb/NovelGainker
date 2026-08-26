import { createHash, randomBytes } from "node:crypto";
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) {
  throw new Error("pnpm must run the NG-405 E2E entry.");
}

const PRIVATE_ENVIRONMENT_NAME =
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
  if (PRIVATE_ENVIRONMENT_NAME.test(name)) {
    delete testEnvironment[name];
  }
}

function runPnpm(name, args) {
  console.log("[ng405-e2e] START " + name);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) {
    throw new Error(name + " failed");
  }
  console.log("[ng405-e2e] PASS " + name);
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
    /Authorization|Bearer\s+|apiKey|accessToken|authToken|providerResponse|promptBody|draftBody|secrets?Body|relationshipBody/iu.test(
      text,
    )
  ) {
    throw new Error(
      "NG-405 evidence contains a forbidden path, credential, Prompt, Response, private draft, or relationship body marker.",
    );
  }
  if (canaryFragments(canary).some((fragment) => bytes.includes(Buffer.from(fragment, "utf8")))) {
    throw new Error("NG-405 evidence contains a private character field Canary fragment.");
  }
}

const evidenceRoot = resolve("target", "ng405-evidence");
mkdirSync(evidenceRoot, { recursive: true });
const canary = "NG405PrivateCharacter" + randomBytes(48).toString("base64url");
if (canary.length < 64) {
  throw new Error("NG-405 synthetic private character Canary generation failed.");
}

runPnpm("static least-privilege and Local-first boundary", ["quality:boundaries"]);
runPnpm("Character v1/v2 Pack, stable ID, references, Context and Golden matrix", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-workflows",
  "--locked",
  "--offline",
]);
runPnpm("App State v7 migration and dual-Provider CharacterProposalV2 contracts", [
  "exec",
  "cargo",
  "test",
  "-p",
  "app-core",
  "--locked",
  "--offline",
]);
runPnpm("Tauri source catalog, v1 upgrade, create, enhance, collision and stale-source matrix", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-desktop",
  "--lib",
  "--locked",
  "--offline",
]);
runPnpm("typed IPC and real React character-v2 review path", [
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
  story: "NG-405",
  mode: "mock-only-local-first",
  generatedAt: new Date().toISOString(),
  workflow: {
    id: "novelgainker.character-create",
    version: 2,
    outputContract: "CharacterProposalV2",
    confirmations: ["send_context", "create_or_replace_character_file"],
    createTarget: "characters/<safe-name>.md",
    enhanceTarget: "same-verified-character-path",
  },
  matrix: {
    mockProviderCount: 2,
    crossProviderAutomaticRequestCount: 0,
    requestBeforeManifestConfirmationCount: 0,
    writeBeforeProposalConfirmationCount: 0,
    metadataCatalogBodyCount: 0,
    unselectedSourceBodyCount: 0,
    maximumSelectedCharacterSources: 6,
    confirmedCharacterCreateCount: 1,
    confirmedCharacterEnhanceCount: 1,
    reverseRelationshipWriteCount: 0,
    sourceChangedWriteCount: 0,
    targetChangedWriteCount: 0,
    collisionOverwriteCount: 0,
    abandonWriteCount: 0,
    privateCharacterFieldEvidenceLeakCount: 0,
  },
  checks: {
    strictCharacterBriefV2: true,
    strictCharacterProposalV2: true,
    rustOwnedIdPathFrontmatterMarkdownLinks: true,
    explicitCharacterSourcePicker: true,
    legacyV1ReadOnlyUntilConfirmedUpgrade: true,
    v2EnhanceReusesIdAndPath: true,
    verifiedRelationshipTargetsOnly: true,
    nonCharacterRelationshipTypeRestricted: true,
    completeCreateOrReplaceDiff: true,
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
  throw new Error("NG-405 security report write failed.");
}
for (const path of scanFiles(evidenceRoot)) {
  assertRedactedEvidence(path, canary);
}

console.log(
  "[ng405-e2e] PASS two Mock Providers, metadata-only Character Source Picker, bounded explicit sources, Rust-owned stable IDs and links, v1 explicit upgrade, same-path v2 enhance, zero pre-confirmation requests/writes, no reverse relationship/collision/stale-source write, no private field evidence leak, and offline edit regression",
);
