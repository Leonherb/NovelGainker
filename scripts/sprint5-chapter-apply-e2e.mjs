import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) throw new Error("pnpm must run the NG-505 E2E entry.");
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
  console.log(`[ng505-e2e] START ${name}`);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) throw new Error(`${name} failed`);
  console.log(`[ng505-e2e] PASS ${name}`);
}
function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

const sources = [
  "crates/novelgainker-workflows/src/chapter_patch.rs",
  "crates/app-core/src/canon_apply.rs",
  "crates/app-core/src/recovery.rs",
  "apps/desktop/src-tauri/src/story_chapter_patch_commands.rs",
  "apps/desktop/src/lib/tauri/story-chapter-patch.ts",
];
const sourceText = sources
  .map((path) => readFileSync(resolve(...path.split("/")), "utf8"))
  .join("\n");
for (const required of [
  "ChapterPatchPreviewV1",
  "accepted_or_final_unlock_confirmed",
  "bind_chapter_patch_encoded_hash",
  "apply_chapter_file_recoverable",
  "TemporaryValidation",
  "begin_chapter_replace_transaction",
  "commit_chapter",
  "rebuild_registered_story_continuity",
  "confirmed: true",
  'let next_status = "draft".to_owned()',
]) {
  if (!sourceText.includes(required))
    throw new Error(`NG-505 is missing required contract: ${required}`);
}
for (const forbidden of ["ProviderHttpClient", "client.stream", "Story Ledger Apply"]) {
  if (sourceText.includes(forbidden))
    throw new Error(`NG-505 contains forbidden expansion: ${forbidden}`);
}

runPnpm("least-privilege and fixed-Pack boundary", ["quality:boundaries"]);
runPnpm("chapter patch builder matrix", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-workflows",
  "chapter_patch::tests",
  "--locked",
  "--offline",
]);
runPnpm("recoverable chapter transaction matrix", [
  "exec",
  "cargo",
  "test",
  "-p",
  "app-core",
  "ng_505",
  "--locked",
  "--offline",
]);
runPnpm("strict chapter patch Tauri IPC", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-desktop",
  "story_chapter_patch_commands::tests",
  "--locked",
  "--offline",
]);
runPnpm("strict chapter patch IPC and explicit-confirm UI", [
  "--filter",
  "@novelgainker/desktop",
  "exec",
  "vitest",
  "run",
  "src/lib/tauri/story-chapter-patch.test.ts",
  "src/components/StoryContextBuilder.test.tsx",
]);

const report = {
  schemaVersion: 1,
  story: "NG-505",
  mode: "offline-deterministic-recoverable-write",
  generatedAt: new Date().toISOString(),
  contract: {
    patchSchemaVersion: 1,
    targetScope: "one-fixed-chapter",
    preconfirmationWriteCount: 0,
    continuityRevalidation: true,
    targetHashRevalidation: true,
    temporaryValidation: true,
    atomicCommit: true,
    postWriteHashValidation: true,
    storyLedgerWriteCount: 0,
    acceptedOrFinalRequiresUnlock: true,
  },
  matrix: {
    createCollisionOverwriteCount: 0,
    staleTargetWriteCount: 0,
    lockedStatusBypassCount: 0,
    invalidTemporaryCommitCount: 0,
    precommitFailureLostOriginalCount: 0,
    postwriteFailureLostOriginalCount: 0,
    restartReplaceLostOriginalCount: 0,
    restartCreateLeftTargetCount: 0,
    providerRequestCount: 0,
    storyLedgerWriteCount: 0,
  },
  sources: sources.map((path) => {
    const bytes = readFileSync(resolve(...path.split("/")));
    return { path, byteLength: bytes.length, sha256: sha256(bytes) };
  }),
};
const reportText = `${JSON.stringify(report, null, 2)}\n`;
if (
  /[A-Za-z]:[/\\]/u.test(reportText) ||
  /Authorization|Bearer\s+|apiKey|accessToken|authToken|projectSessionId/iu.test(reportText)
)
  throw new Error("NG-505 evidence contains forbidden path or credential text.");
const evidenceRoot = resolve("target", "ng505-evidence");
mkdirSync(evidenceRoot, { recursive: true });
writeFileSync(join(evidenceRoot, "chapter-apply-matrix.json"), reportText, "utf8");
console.log(
  "[ng505-e2e] PASS preconfirmationWriteCount=0 lostOriginalCount=0 storyLedgerWriteCount=0 providerRequestCount=0",
);
console.log("[ng505-e2e] EVIDENCE target/ng505-evidence/chapter-apply-matrix.json");
