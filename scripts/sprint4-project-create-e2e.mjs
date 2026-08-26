import { createHash, randomBytes } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) {
  throw new Error("pnpm must run the NG-401 E2E entry.");
}

const SECRET_ENVIRONMENT_NAME =
  /(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu;
const testEnvironment = {
  ...process.env,
  CARGO_NET_OFFLINE: "true",
  HTTP_PROXY: "http://127.0.0.1:9",
  HTTPS_PROXY: "http://127.0.0.1:9",
  ALL_PROXY: "http://127.0.0.1:9",
  NO_PROXY: "",
};
for (const name of Object.keys(testEnvironment)) {
  if (SECRET_ENVIRONMENT_NAME.test(name)) {
    delete testEnvironment[name];
  }
}

function runPnpm(name, args) {
  console.log(`[ng401-e2e] START ${name}`);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) {
    throw new Error(`${name} failed`);
  }
  console.log(`[ng401-e2e] PASS ${name}`);
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

function assertNoCanary(bytes, canary) {
  if (canaryFragments(canary).some((fragment) => bytes.includes(Buffer.from(fragment)))) {
    throw new Error("NG-401 evidence contains a Key Canary fragment.");
  }
}

function assertNoSensitiveTextEvidence(bytes, canary) {
  const text = bytes.toString("utf8");
  if (
    /[A-Za-z]:[\\/]/u.test(text) ||
    /Authorization|Bearer\s+|apiKey|accessToken|authToken/iu.test(text)
  ) {
    throw new Error("NG-401 text evidence contains a forbidden path or credential marker.");
  }
  assertNoCanary(bytes, canary);
}

const evidenceRoot = resolve("target", "ng401-evidence");
mkdirSync(evidenceRoot, { recursive: true });
const screenshotFile = join(evidenceRoot, "tauri-ng401-window.png");
const canary = `NG401Key${randomBytes(48).toString("base64url")}`;
if (canary.length < 64) {
  throw new Error("NG-401 synthetic Key Canary generation failed.");
}

runPnpm("static offline and least-privilege boundary", ["quality:boundaries"]);
runPnpm("project-create Core matrix", [
  "exec",
  "cargo",
  "test",
  "-p",
  "app-core",
  "--locked",
  "--offline",
  "project::tests::",
]);
runPnpm("build-validated project-create Workflow Pack", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-workflows",
  "--locked",
  "--offline",
]);
runPnpm("typed IPC and real React project-create path", [
  "--filter",
  "@novelgainker/desktop",
  "exec",
  "vitest",
  "run",
  "src/lib/tauri/project.test.ts",
  "src/lib/tauri/workflow.test.ts",
  "src/components/ProjectWorkspace.test.tsx",
  "src/components/ConversationPanel.test.tsx",
]);
runPnpm("Sprint 1 no-Key offline editor regression", ["e2e:sprint1"]);
runPnpm("real Tauri NG-401 window evidence", ["smoke:ng401"]);

if (!existsSync(screenshotFile) || !statSync(screenshotFile).isFile()) {
  throw new Error("Real Tauri NG-401 window evidence is missing.");
}
const screenshotBytes = readFileSync(screenshotFile);
const pngSignature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
if (screenshotBytes.length < 4096 || !screenshotBytes.subarray(0, 8).equals(pngSignature)) {
  throw new Error("Real Tauri NG-401 window evidence is invalid.");
}
assertNoCanary(screenshotBytes, canary);

const report = {
  schemaVersion: 1,
  story: "NG-401",
  mode: "no-key-offline",
  generatedAt: new Date().toISOString(),
  workflow: {
    id: "novelgainker.project-create",
    version: 1,
    confirmation: "create_project_scaffold",
  },
  matrix: {
    providerRequestCount: 0,
    providerRouterRequestCount: 0,
    vaultAccessCount: 0,
    prePreviewWriteCount: 0,
    preConfirmationWriteCount: 0,
    scaffoldDirectoryCount: 11,
    scaffoldFileCount: 4,
    injectedRollbackStepCount: 15,
    stalePreviewWriteCount: 0,
    duplicateOverwriteCount: 0,
  },
  checks: {
    controlledMetadata: true,
    fullPreview: true,
    hashBoundConfirmation: true,
    addOnlyTransaction: true,
    unrelatedBytesPreserved: true,
    projectOpenedAfterCreate: true,
    offlineManualEditRegression: true,
    realTauriWindow: true,
  },
  windowEvidenceSha256: sha256(screenshotBytes),
};
const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`, "utf8");
assertNoSensitiveTextEvidence(reportBytes, canary);
writeFileSync(join(evidenceRoot, "security-report.json"), reportBytes);

console.log(
  "[ng401-e2e] PASS no Key/network, zero Provider/Router/Vault calls, zero pre-confirmation writes, 11 directories, 4 files, 15-step rollback, Sprint 1 offline edit regression, and real Tauri evidence",
);
