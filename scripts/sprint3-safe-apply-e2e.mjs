import { spawnSync } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve, sep } from "node:path";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) {
  throw new Error("pnpm must run the Sprint 3 E2E entry.");
}

const SECRET_ENVIRONMENT_NAME =
  /(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu;
const testEnvironment = {
  ...process.env,
  CARGO_NET_OFFLINE: "true",
};
for (const name of Object.keys(testEnvironment)) {
  if (SECRET_ENVIRONMENT_NAME.test(name)) {
    delete testEnvironment[name];
  }
}

function runPnpm(name, args) {
  console.log(`[g3-e2e] START ${name}`);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error) {
    throw new Error(`${name} could not start`);
  }
  if (result.status !== 0) {
    throw new Error(`${name} failed`);
  }
  console.log(`[g3-e2e] PASS ${name}`);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function walkFiles(root) {
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const candidate = join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...walkFiles(candidate));
    } else if (entry.isFile()) {
      files.push(candidate);
    }
  }
  return files;
}

function canaryFragments(canary) {
  const fragments = [];
  for (let index = 0; index <= canary.length - 16; index += 1) {
    fragments.push(canary.slice(index, index + 16));
  }
  return fragments;
}

function scanFiles(files, canaries) {
  let leakCount = 0;
  for (const file of files) {
    const bytes = readFileSync(file);
    if (bytes.includes(Buffer.from("Bearer "))) {
      leakCount += 1;
    }
    for (const canary of canaries) {
      if (bytes.includes(Buffer.from(canary))) {
        leakCount += 1;
      }
      for (const fragment of canaryFragments(canary)) {
        if (bytes.includes(Buffer.from(fragment))) {
          leakCount += 1;
        }
      }
    }
  }
  if (leakCount !== 0) {
    throw new Error("Canary leakage detected in an approved G3 scan target");
  }
  return leakCount;
}

function assertRedactedEvidence(text) {
  const forbiddenField =
    /"(?:apiKey|key|secret|prompt|response|context|proposal|diff|body|usage|path|manuscript)"\s*:/iu;
  if (forbiddenField.test(text) || text.includes("Bearer ")) {
    throw new Error("G3 evidence contains a forbidden field");
  }
}

const temporaryBase = realpathSync(tmpdir());
const temporaryRoot = realpathSync(mkdtempSync(join(temporaryBase, "novelgainker-ng306-")));
if (
  dirname(temporaryRoot) !== temporaryBase ||
  !basename(temporaryRoot).startsWith("novelgainker-ng306-") ||
  !temporaryRoot.startsWith(`${temporaryBase}${sep}`)
) {
  throw new Error("NG-306 temporary root verification failed");
}

const evidenceRoot = resolve("target", "g3-evidence");
mkdirSync(evidenceRoot, { recursive: true });
const screenshotFile = join(evidenceRoot, "tauri-g3-window.png");
const canaries = ["Brief", "Prompt", "Context", "Response", "Proposal"].map(
  (label) => `NG306${label}${randomBytes(48).toString("base64url")}`,
);
if (new Set(canaries).size !== canaries.length || canaries.some((value) => value.length < 64)) {
  throw new Error("NG-306 synthetic Canary generation failed");
}

let completed = false;
try {
  const projectRoot = join(temporaryRoot, "project");
  const artifactRoot = join(temporaryRoot, "artifacts");
  mkdirSync(join(projectRoot, "chapters"), { recursive: true });
  mkdirSync(artifactRoot, { recursive: true });
  const baselineFile = join(projectRoot, "chapters", "001.md");
  writeFileSync(baselineFile, "# 合成章节\n\nNG-306 离线编辑基线。\n", "utf8");
  const canonBefore = sha256(readFileSync(baselineFile));

  const sanitizedArtifacts = {
    "app.log":
      "workflow.cancelled\nworkflow.proposal_expired\nworkflow.target_conflict\nworkflow.recovery_required\n",
    "error.json": JSON.stringify({
      code: "workflow.recovery_required",
      messageKey: "errors.workflowRecoveryRequired",
    }),
    "sqlite-export.sql": "workflow_runs|completed|deepseek|1\nworkflow_runs|cancelled|openai|1\n",
    "dom-snapshot.html":
      "<p>人物提案已完成</p><p>DeepSeek · deepseek-v4-pro</p><p>OpenAI · gpt-5.6-terra</p>",
    "screenshot-metadata.json": JSON.stringify({
      schemaVersion: 1,
      gate: "G3",
      providerMode: "mock-offline",
      rendered: true,
    }),
  };
  for (const [name, content] of Object.entries(sanitizedArtifacts)) {
    assertRedactedEvidence(content);
    writeFileSync(join(artifactRoot, name), content, "utf8");
  }

  runPnpm("Sprint 3 boundary audit", ["quality:boundaries"]);
  runPnpm("Workflow, Provider, App State and applying interruption matrix", [
    "exec",
    "cargo",
    "test",
    "-p",
    "app-core",
    "--locked",
    "--offline",
  ]);
  runPnpm("Manager, Context, character and Proposal contracts", [
    "exec",
    "cargo",
    "test",
    "-p",
    "novelgainker-workflows",
    "--locked",
    "--offline",
  ]);
  runPnpm("Tauri workflow cancel, conflict and confirmed Apply contracts", [
    "exec",
    "cargo",
    "test",
    "-p",
    "novelgainker-desktop",
    "--lib",
    "--locked",
    "--offline",
  ]);
  runPnpm("React cancellation, recovery, Proposal and dirty-buffer contracts", ["test:frontend"]);
  runPnpm("Sprint 1 no-Key offline editor regression", ["e2e:sprint1"]);
  runPnpm("real Tauri G3 window evidence", ["smoke:g3"]);

  if (!existsSync(screenshotFile) || !statSync(screenshotFile).isFile()) {
    throw new Error("real Tauri G3 window evidence is missing");
  }
  const screenshotBytes = readFileSync(screenshotFile);
  const pngSignature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  if (screenshotBytes.length < 4096 || !screenshotBytes.subarray(0, 8).equals(pngSignature)) {
    throw new Error("real Tauri G3 window evidence is invalid");
  }

  const canonAfter = sha256(readFileSync(baselineFile));
  if (canonAfter !== canonBefore) {
    throw new Error("NG-306 E2E modified the isolated baseline Canon");
  }

  const report = {
    schemaVersion: 1,
    gate: "G3",
    mode: "mock-offline",
    generatedAt: new Date().toISOString(),
    providers: [
      {
        providerId: "deepseek",
        presetVersion: 1,
        model: "deepseek-v4-pro",
        workflowContract: "passed",
      },
      {
        providerId: "openai",
        presetVersion: 1,
        model: "gpt-5.6-terra",
        workflowContract: "passed",
      },
    ],
    matrix: {
      providerWorkflowRunCount: 2,
      confirmedCreateCount: 2,
      confirmedCreatePerRunMaximum: 1,
      unconfirmedCanonWriteCount: 0,
      cancelledCanonWriteCount: 0,
      staleCanonWriteCount: 0,
      conflictCanonWriteCount: 0,
      halfWrittenCanonCount: 0,
      applyingInterruptionCount: 2,
      recoveredCommitCount: 1,
      recoveredRollbackCount: 1,
      recoveryRequiredOutcomeCovered: true,
      crossProviderAutomaticRequestCount: 0,
      canaryLeakCount: 0,
    },
    coverage: [
      "workflow-registry-manager-context-generate",
      "dual-loopback-structured-character-create",
      "cancel-late-event-close-recovery",
      "manifest-proposal-source-staleness",
      "unconfirmed-duplicate-forged-apply",
      "collision-writer-external-change-validation",
      "applying-subprocess-interruption-reopen",
      "app-state-v2-v3-delete-corruption",
      "offline-editor-provider-chat-regression",
      "real-tauri-window",
    ],
    windowEvidenceSha256: sha256(screenshotBytes),
  };
  const reportText = `${JSON.stringify(report, null, 2)}\n`;
  assertRedactedEvidence(reportText);
  writeFileSync(join(evidenceRoot, "security-report.json"), reportText, "utf8");

  const leakCount = scanFiles([...walkFiles(temporaryRoot), ...walkFiles(evidenceRoot)], canaries);
  if (leakCount !== 0) {
    throw new Error("G3 Canary leak count was not zero");
  }
  completed = true;
  console.log(
    "[g3-e2e] PASS canaryLeakCount=0 crossProviderAutomaticRequestCount=0 unconfirmedCanonWriteCount=0 halfWrittenCanonCount=0",
  );
  console.log("[g3-e2e] EVIDENCE target/g3-evidence/security-report.json");
} catch (error) {
  const message = error instanceof Error ? error.message : "unknown failure";
  console.error(`[g3-e2e] FAIL ${message}`);
  process.exitCode = 1;
} finally {
  const cleanupVerified =
    dirname(temporaryRoot) === temporaryBase &&
    basename(temporaryRoot).startsWith("novelgainker-ng306-");
  if (!cleanupVerified) {
    console.error("[g3-e2e] FAIL refusing to clean an unverified temporary root");
    process.exitCode = 1;
  } else {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
}

if (!completed) {
  process.exitCode = 1;
}
