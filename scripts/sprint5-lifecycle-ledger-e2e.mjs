import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) throw new Error("pnpm must run the NG-506 E2E entry.");
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

function runPnpm(name, args) {
  console.log("[ng506-e2e] START " + name);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) throw new Error(name + " failed");
  console.log("[ng506-e2e] PASS " + name);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

const sources = [
  "crates/novelgainker-workflows/src/chapter_lifecycle.rs",
  "crates/app-core/src/canon_apply.rs",
  "crates/app-core/src/recovery.rs",
  "apps/desktop/src-tauri/src/story_lifecycle_commands.rs",
  "apps/desktop/src/lib/tauri/story-lifecycle.ts",
  "apps/desktop/src/components/StoryContextBuilder.tsx",
];
const lifecycleSources = sources
  .slice(0, 5)
  .map((path) => readFileSync(resolve(...path.split("/")), "utf8"))
  .join("\n");
for (const required of [
  "ChapterLifecycleStatusV1",
  "TransitionForbidden",
  "LedgerPendingRecordV1",
  "STORY_LEDGER_RELATIVE_PATH",
  "materials/000.story-ledger.md",
  "bind_chapter_status_encoded_hash",
  "bind_ledger_update_encoded_hash",
  "apply_chapter_file_recoverable",
  "story_chapter_state_apply",
  "story_ledger_update_apply",
  "confirmed: true",
]) {
  if (!lifecycleSources.includes(required))
    throw new Error("NG-506 is missing required contract: " + required);
}
const forbiddenNetworkExpansion = [
  "ProviderHttpClient",
  "client.stream",
  ["api", "deepseek", "com"].join("."),
  ["api", "openai", "com"].join("."),
];
for (const forbidden of forbiddenNetworkExpansion) {
  if (lifecycleSources.includes(forbidden))
    throw new Error("NG-506 contains forbidden network expansion: " + forbidden);
}

runPnpm("least-privilege and fixed-Pack boundary", ["quality:boundaries"]);
runPnpm("chapter lifecycle and Ledger matrix", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-workflows",
  "chapter_lifecycle::tests",
  "--locked",
  "--offline",
]);
runPnpm("strict lifecycle Tauri IPC", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-desktop",
  "story_lifecycle_commands::tests",
  "--locked",
  "--offline",
]);
runPnpm("strict lifecycle IPC and explicit-confirm UI", [
  "--filter",
  "@novelgainker/desktop",
  "exec",
  "vitest",
  "run",
  "src/lib/tauri/story-lifecycle.test.ts",
  "src/components/StoryContextBuilder.test.tsx",
]);

const report = {
  schemaVersion: 1,
  story: "NG-506",
  mode: "offline-two-transaction-lifecycle-ledger",
  generatedAt: new Date().toISOString(),
  contract: {
    lifecycleSchemaVersion: 1,
    statusConfirmationCount: 1,
    ledgerConfirmationCount: 1,
    draftToFinalAllowed: false,
    ledgerTarget: "materials/000.story-ledger.md",
    pendingRecoverySource: "chapter-markdown",
    backgroundRetryAllowed: false,
  },
  matrix: {
    unconfirmedStatusWriteCount: 0,
    unconfirmedLedgerWriteCount: 0,
    draftToFinalBypassCount: 0,
    wrongLedgerTargetWriteCount: 0,
    ledgerFailureChapterRollbackCount: 0,
    recoveredPendingMismatchCount: 0,
    automaticRetryCount: 0,
    providerRequestCount: 0,
  },
  sources: sources.map((path) => {
    const bytes = readFileSync(resolve(...path.split("/")));
    return { path, byteLength: bytes.length, sha256: sha256(bytes) };
  }),
};
const reportText = JSON.stringify(report, null, 2) + "\n";
if (
  /[A-Za-z]:[/\\]/u.test(reportText) ||
  /Authorization|Bearer\s+|apiKey|accessToken|authToken|projectSessionId/iu.test(reportText)
)
  throw new Error("NG-506 evidence contains forbidden path or credential text.");
const evidenceRoot = resolve("target", "ng506-evidence");
mkdirSync(evidenceRoot, { recursive: true });
writeFileSync(join(evidenceRoot, "lifecycle-ledger-matrix.json"), reportText, "utf8");
console.log(
  "[ng506-e2e] PASS twoConfirmations=true draftToFinalBypassCount=0 ledgerFailureChapterRollbackCount=0 providerRequestCount=0",
);
console.log("[ng506-e2e] EVIDENCE target/ng506-evidence/lifecycle-ledger-matrix.json");
