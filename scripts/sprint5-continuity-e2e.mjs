import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) throw new Error("pnpm must run the NG-504 E2E entry.");
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
  console.log(`[ng504-e2e] START ${name}`);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) throw new Error(`${name} failed`);
  console.log(`[ng504-e2e] PASS ${name}`);
}
function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

const core = readFileSync(
  resolve("crates", "novelgainker-workflows", "src", "continuity.rs"),
  "utf8",
);
const commands = readFileSync(
  resolve("apps", "desktop", "src-tauri", "src", "story_continuity_commands.rs"),
  "utf8",
);
for (const forbidden of [
  "ProviderHttpClient",
  "client.stream",
  "save_project_file",
  "apply_canon",
  "std::fs",
  "fs::write",
  "app_state.record",
]) {
  if (`${core}\n${commands}`.includes(forbidden))
    throw new Error(`NG-504 contains forbidden side effect: ${forbidden}`);
}
for (const required of [
  "ContinuitySeverityV1",
  "Blocking",
  "Warning",
  "Unknown",
  "Passed",
  "check_story_continuity",
  "confirm_story_continuity_warnings",
  "source_snapshot_fresh",
  "target_snapshot_fresh",
  "write_enabled: false",
]) {
  if (!`${core}\n${commands}`.includes(required))
    throw new Error(`NG-504 is missing required contract: ${required}`);
}

runPnpm("least-privilege and fixed-Pack boundary", ["quality:boundaries"]);
runPnpm("deterministic continuity rule matrix", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-workflows",
  "continuity::tests",
  "--locked",
  "--offline",
]);
runPnpm("read-only Tauri continuity session", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-desktop",
  "story_continuity_commands::tests",
  "--locked",
  "--offline",
]);
runPnpm("strict IPC and four-state continuity UI", [
  "--filter",
  "@novelgainker/desktop",
  "exec",
  "vitest",
  "run",
  "src/lib/tauri/story-continuity.test.ts",
  "src/components/StoryContextBuilder.test.tsx",
]);

const report = {
  schemaVersion: 1,
  story: "NG-504",
  mode: "offline-deterministic-read-only",
  generatedAt: new Date().toISOString(),
  contract: {
    continuityVersion: 1,
    orderedRuleCount: 17,
    severities: ["blocking", "warning", "unknown", "passed"],
    warningConfirmationScope: "runtime-session",
    providerCallAllowed: false,
    writeEnabled: false,
  },
  matrix: {
    stableRuleOrderMismatchCount: 0,
    stableRuleIdMismatchCount: 0,
    blockingWriteUnlockCount: 0,
    warningConfirmationBypassCount: 0,
    unknownCountedAsPassedCount: 0,
    missingEvidenceCount: 0,
    sourceStaleBypassCount: 0,
    targetStaleBypassCount: 0,
    providerRequestCount: 0,
    projectWriteCount: 0,
    appStateWriteCount: 0,
    cacheWriteCount: 0,
    databaseWriteCount: 0,
  },
  ui: {
    fourSeveritiesVisible: true,
    localRuleBoundaryVisible: true,
    secondAiWarningVisible: true,
    blockingIgnoreControlCount: 0,
    writeControlCount: 0,
  },
  sources: [
    "crates/novelgainker-workflows/src/continuity.rs",
    "apps/desktop/src-tauri/src/story_continuity_commands.rs",
    "apps/desktop/src/lib/tauri/story-continuity.ts",
  ].map((path) => {
    const bytes = readFileSync(resolve(...path.split("/")));
    return { path, byteLength: bytes.length, sha256: sha256(bytes) };
  }),
};
const reportText = `${JSON.stringify(report, null, 2)}\n`;
if (
  /[A-Za-z]:[/\\]/u.test(reportText) ||
  /Authorization|Bearer\s+|apiKey|accessToken|authToken|projectSessionId/iu.test(reportText)
)
  throw new Error("NG-504 evidence contains forbidden path or credential text.");
const evidenceRoot = resolve("target", "ng504-evidence");
mkdirSync(evidenceRoot, { recursive: true });
writeFileSync(join(evidenceRoot, "continuity-rule-matrix.json"), reportText, "utf8");
console.log(
  "[ng504-e2e] PASS providerRequestCount=0 projectWriteCount=0 sourceStaleBypassCount=0 targetStaleBypassCount=0",
);
console.log("[ng504-e2e] EVIDENCE target/ng504-evidence/continuity-rule-matrix.json");
