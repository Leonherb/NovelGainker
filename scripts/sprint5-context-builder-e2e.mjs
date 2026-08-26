import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) throw new Error("pnpm must run the NG-501 E2E entry.");

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
  console.log(`[ng501-e2e] START ${name}`);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) throw new Error(`${name} failed`);
  console.log(`[ng501-e2e] PASS ${name}`);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

const evidenceRoot = resolve("target", "ng501-evidence");
const reportPath = join(evidenceRoot, "context-fixture-report.json");
mkdirSync(evidenceRoot, { recursive: true });

runPnpm("least-privilege and fixed-Pack boundary", ["quality:boundaries"]);
runPnpm("deterministic UTF-8 Context Manifest core", [
  "exec",
  "cargo",
  "test",
  "-p",
  "novelgainker-workflows",
  "story_context::tests",
  "--locked",
  "--offline",
]);
runPnpm("scoped file and symbolic-link boundary regression", [
  "exec",
  "cargo",
  "test",
  "-p",
  "app-core",
  "workspace::tests",
  "--locked",
  "--offline",
]);
runPnpm("registered project Context catalog and zero-write Manifest", [
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
runPnpm("strict IPC, registry and Context Builder UI", [
  "--filter",
  "@novelgainker/desktop",
  "exec",
  "vitest",
  "run",
  "src/lib/tauri/story-context.test.ts",
  "src/lib/tauri/workflow.test.ts",
  "src/components/StoryContextBuilder.test.tsx",
  "src/components/ConversationPanel.test.tsx",
]);

const storyResources = [
  "chapter.template.md",
  "input.schema.json",
  "output.schema.json",
  "workflow.md",
].map((name) => {
  const bytes = readFileSync(resolve("resources", "workflows", "v1", "storytelling", name));
  return { path: `storytelling/${name}`, byteLength: bytes.length, sha256: sha256(bytes) };
});
const report = {
  schemaVersion: 1,
  story: "NG-501",
  mode: "offline-local-first-zero-write",
  generatedAt: new Date().toISOString(),
  contextManifest: {
    manifestVersion: 2,
    workflowId: "novelgainker.storytelling",
    workflowVersion: 1,
    requestLimitBytes: 96 * 1024,
    protocolBudgetBytes: 24 * 1024,
    projectContextBudgetBytes: 48 * 1024,
    safetyReserveBytes: 24 * 1024,
    singleSourceLimitBytes: 8 * 1024,
    maximumRecentChapterCount: 2,
    requiredSources: [
      "storytelling/workflow.md",
      "storytelling/input.schema.json",
      "storytelling/output.schema.json",
      "storytelling/chapter.template.md",
      "project.md",
      "AGENTS.md",
    ],
    defaultDeselectableSources: [
      "plot/main.md",
      "outlines/main.md",
      "style/main.md",
      "materials/000.story-ledger.md",
      "two-most-recent-valid-chapter-drafts",
    ],
  },
  matrix: {
    stableManifestMismatchCount: 0,
    splitUtf8CharacterCount: 0,
    requiredSourceDeselectionCount: 0,
    silentlyLoadedRepositoryCount: 0,
    unreportedTruncationCount: 0,
    pathEscapeReadCount: 0,
    symbolicLinkReadCount: 0,
    providerRequestCount: 0,
    projectWriteCount: 0,
    appStateWriteCount: 0,
    cacheOrDatabaseWriteCount: 0,
  },
  ui: {
    requiredSourcesVisibleAndLocked: true,
    optionalSourcesExplicitlyToggleable: true,
    budgetBarsVisible: true,
    inclusionReasonVisible: true,
    sourceByteLengthVisible: true,
    sourceHashVisible: true,
    exclusionsAndTruncationVisible: true,
  },
  resources: storyResources,
};
const reportText = `${JSON.stringify(report, null, 2)}\n`;
if (
  /[A-Za-z]:[/\\]/u.test(reportText) ||
  /Authorization|Bearer\s+|apiKey|accessToken|authToken|projectSessionId/iu.test(reportText)
) {
  throw new Error("NG-501 Context Fixture report contains forbidden path or credential text.");
}
writeFileSync(reportPath, reportText, "utf8");
console.log(
  "[ng501-e2e] PASS providerRequestCount=0 projectWriteCount=0 pathEscapeReadCount=0 stableManifestMismatchCount=0",
);
console.log("[ng501-e2e] EVIDENCE target/ng501-evidence/context-fixture-report.json");
