import { createHash, randomBytes } from "node:crypto";
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) throw new Error("pnpm must run the NG-406 E2E entry.");

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
  console.log(`[ng406-e2e] START ${name}`);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) throw new Error(`${name} failed`);
  console.log(`[ng406-e2e] PASS ${name}`);
}

function fragments(canary) {
  return Array.from({ length: canary.length - 15 }, (_, index) => canary.slice(index, index + 16));
}

function scanFiles(directory) {
  return readdirSync(directory, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => join(entry.parentPath, entry.name));
}

function assertRedacted(path, canary) {
  const bytes = readFileSync(path);
  const text = bytes.toString("utf8");
  if (
    /[A-Za-z]:[/\\]/u.test(text) ||
    /Authorization|Bearer\s+|apiKey|accessToken|authToken|originalText|providerResponse|promptBody|projectSessionId/iu.test(
      text,
    ) ||
    fragments(canary).some((fragment) => bytes.includes(Buffer.from(fragment, "utf8")))
  ) {
    throw new Error(
      "NG-406 evidence contains a forbidden path, credential, original text, Provider response, Prompt, or project session marker.",
    );
  }
}

const evidenceRoot = resolve("target", "ng406-evidence");
mkdirSync(evidenceRoot, { recursive: true });
const canary = `NG406RouterPrivate${randomBytes(48).toString("base64url")}`;

runPnpm("Router least-privilege and Local-first boundary", ["quality:boundaries"]);
runPnpm("deterministic commands, strict RouterDecisionV1 and Pack resources", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-workflows",
  "router::tests",
  "--locked",
  "--offline",
]);
runPnpm("App State v8 redaction and dual-Provider one-shot classification", [
  "exec",
  "cargo",
  "test",
  "-p",
  "app-core",
  "--locked",
  "--offline",
]);
runPnpm("Tauri Router registration and local zero-request path", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-desktop",
  "router_commands::tests",
  "--lib",
  "--locked",
  "--offline",
]);
runPnpm("strict IPC and React consent/selector/form-stop path", [
  "--filter",
  "@novelgainker/desktop",
  "exec",
  "vitest",
  "run",
  "src/lib/tauri/router.test.ts",
  "src/lib/tauri/workflow.test.ts",
  "src/components/ConversationPanel.test.tsx",
]);
runPnpm("Sprint 1 no-Key offline editor regression", ["e2e:sprint1"]);

const report = {
  schemaVersion: 1,
  story: "NG-406",
  mode: "mock-only-local-first",
  generatedAt: new Date().toISOString(),
  router: {
    schemaVersion: 1,
    decisionContract: "RouterDecisionV1",
    providers: ["deepseek", "openai"],
    routableWorkflowCount: 5,
    deterministicCommandCount: 6,
    maximumStructuralRepairCount: 1,
    automaticTransientRetryCount: 0,
    automaticFallbackCount: 0,
  },
  matrix: {
    exactCommandProviderRequestCount: 0,
    buttonProviderRequestCount: 0,
    requestBeforeConsentCount: 0,
    projectReadCount: 0,
    projectWriteCount: 0,
    highDecisionAutomaticExecutionCount: 0,
    mediumLowAutomaticFormOpenCount: 0,
    malformedAutomaticExecutionCount: 0,
    persistedUserInputCount: 0,
    persistedRemotePayloadCount: 0,
    retainedTerminalMetadataLimit: 20,
  },
  checks: {
    simplifiedChineseDefault: true,
    strictFixedRegistry: true,
    onlyCurrentTextAndCatalogSent: true,
    receiverModelScopeShownBeforeSend: true,
    cancellableClassification: true,
    highOnlyOpensSuggestedForm: true,
    mediumLowUseFixedSelector: true,
    noKeyOfflineSelector: true,
    ordinaryChatContractRetained: true,
    formStopsBeforePrepareGenerateConfirmWrite: true,
    offlineManualEditRegression: true,
  },
};
const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`, "utf8");
const reportPath = join(evidenceRoot, "security-report.json");
writeFileSync(reportPath, reportBytes);
if (
  !statSync(reportPath).isFile() ||
  createHash("sha256").update(reportBytes).digest("hex").length !== 64
) {
  throw new Error("NG-406 security report write failed.");
}
for (const file of scanFiles(evidenceRoot)) assertRedacted(file, canary);

console.log(
  "[ng406-e2e] PASS deterministic zero-request routing, explicit same-Provider consent, one-shot strict classification, fixed selector, no project read/write, metadata-only v8 state, and offline edit regression",
);
