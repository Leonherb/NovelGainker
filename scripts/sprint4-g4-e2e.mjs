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
import { basename, dirname, join, relative, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { DatabaseSync } from "node:sqlite";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) throw new Error("pnpm must run the NG-407/G4 E2E entry.");

const qualitySequenceMode = process.argv.includes("--quality-sequence");
const qualityStartedAt = process.env.NOVELGAINKER_QUALITY_STARTED_AT;
if (qualitySequenceMode && !qualityStartedAt) {
  throw new Error("G4 quality-sequence mode requires the parent quality start marker.");
}
const runStartedAt = qualitySequenceMode ? Date.parse(qualityStartedAt) : Date.now();
if (!Number.isFinite(runStartedAt)) throw new Error("G4 quality start marker is invalid.");

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

const expectedResourcePaths = [
  "character-create/character.template.md",
  "character-create/input.schema.json",
  "character-create/output.schema.json",
  "character-create/workflow.md",
  "material-create/input.schema.json",
  "material-create/material.template.md",
  "material-create/output.schema.json",
  "material-create/workflow.md",
  "pack.json",
  "plot-create/input.schema.json",
  "plot-create/output.schema.json",
  "plot-create/plot.template.md",
  "plot-create/workflow.md",
  "project-create/input.schema.json",
  "project-create/output.schema.json",
  "project-create/project.template.md",
  "project-create/workflow.md",
  "router/deterministic-intents.v1.json",
  "router/router-decision.schema.json",
  "router/workflow.md",
  "setting-create/input.schema.json",
  "setting-create/output.schema.json",
  "setting-create/setting.template.md",
  "setting-create/workflow.md",
  "storytelling/chapter.template.md",
  "storytelling/input.schema.json",
  "storytelling/output.schema.json",
  "storytelling/workflow.md",
];
const expectedPackHash = "24616ecda847c1df3f3e9b4550e70ef987b8b2c62e7cfe0b3985680721ce84f9";
const expectedWorkflows = [
  ["novelgainker.character-create", 2, true, true],
  ["novelgainker.project-create", 1, true, true],
  ["novelgainker.setting-create", 1, true, true],
  ["novelgainker.material-create", 1, true, true],
  ["novelgainker.plot-create", 1, true, true],
  ["novelgainker.storytelling", 1, false, false],
];
const reportSources = [
  ["G2", "target/g2-evidence/security-report.json"],
  ["G3", "target/g3-evidence/security-report.json"],
  ["NG-401", "target/ng401-evidence/security-report.json"],
  ["NG-402", "target/ng402-evidence/security-report.json"],
  ["NG-403", "target/ng403-evidence/security-report.json"],
  ["NG-404", "target/ng404-evidence/security-report.json"],
  ["NG-405", "target/ng405-evidence/security-report.json"],
  ["NG-406", "target/ng406-evidence/security-report.json"],
];

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function runPnpm(name, args) {
  console.log("[g4-e2e] START " + name);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) throw new Error(name + " failed");
  console.log("[g4-e2e] PASS " + name);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function walkFiles(root) {
  return readdirSync(root, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => join(entry.parentPath, entry.name));
}

function relativeFiles(root) {
  return walkFiles(root)
    .map((path) => relative(root, path).replaceAll("\\", "/"))
    .sort();
}

function updateBoundHash(hasher, label, bytes) {
  const labelBytes = Buffer.from(label, "utf8");
  const labelLength = Buffer.alloc(8);
  const byteLength = Buffer.alloc(8);
  labelLength.writeBigUInt64BE(BigInt(labelBytes.length));
  byteLength.writeBigUInt64BE(BigInt(bytes.length));
  hasher.update(labelLength);
  hasher.update(labelBytes);
  hasher.update(byteLength);
  hasher.update(bytes);
}

function calculatePackHash(resourceRoot, paths) {
  const hasher = createHash("sha256");
  for (const path of paths) {
    updateBoundHash(hasher, path, readFileSync(join(resourceRoot, ...path.split("/"))));
  }
  return hasher.digest("hex");
}

function calculateWorkflowHash(resourceRoot, workflow) {
  const hasher = createHash("sha256");
  updateBoundHash(
    hasher,
    "@manifest/" + workflow.id,
    Buffer.from(JSON.stringify(workflow), "utf8"),
  );
  for (const path of [
    workflow.workflowResource,
    workflow.inputSchema,
    workflow.outputSchema,
    workflow.templateResource,
  ].sort()) {
    updateBoundHash(hasher, path, readFileSync(join(resourceRoot, ...path.split("/"))));
  }
  return hasher.digest("hex");
}

function readFreshReport(label, relativePath) {
  const absolutePath = resolve(relativePath);
  assert(
    existsSync(absolutePath) && statSync(absolutePath).isFile(),
    label + " report is missing.",
  );
  const report = JSON.parse(readFileSync(absolutePath, "utf8"));
  assert(report.schemaVersion === 1, label + " report schema is not v1.");
  const generatedAt = Date.parse(report.generatedAt);
  assert(Number.isFinite(generatedAt), label + " report timestamp is invalid.");
  assert(generatedAt >= runStartedAt - 1000, label + " report is stale for this G4 run.");
  return { absolutePath, report };
}

function assertZero(report, fields, label) {
  for (const field of fields) {
    assert(report.matrix[field] === 0, label + " expected matrix." + field + " to equal zero.");
  }
}

function assertTrueChecks(report, label) {
  assert(report.checks && typeof report.checks === "object", label + " checks are missing.");
  for (const [name, value] of Object.entries(report.checks)) {
    assert(value === true, label + " check " + name + " did not pass.");
  }
}

function canaryFragments(canary) {
  return Array.from({ length: canary.length - 15 }, (_, index) => canary.slice(index, index + 16));
}

function assertNoCanaries(files, canaries) {
  for (const file of files) {
    const bytes = readFileSync(file);
    for (const canary of canaries) {
      for (const fragment of canaryFragments(canary)) {
        assert(
          !bytes.includes(Buffer.from(fragment, "utf8")),
          "G4 Canary fragment leaked into a forbidden artifact.",
        );
      }
    }
  }
}

function assertRedactedText(text, label) {
  assert(!/[A-Za-z]:[/\\]/u.test(text), label + " contains an absolute Windows path.");
  assert(
    !/Authorization|Bearer\s+|apiKey|accessToken|authToken|credentialRef|originalText|promptBody|responseBody|contextBody|proposalBody|diffBody|manuscriptBody/iu.test(
      text,
    ),
    label + " contains a credential or private body marker.",
  );
}

function assertPngMetadataRedacted(path, canaries) {
  const bytes = readFileSync(path);
  const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  assert(bytes.length >= 4096 && bytes.subarray(0, 8).equals(signature), "G4 PNG is invalid.");
  let offset = 8;
  let foundEnd = false;
  while (offset + 12 <= bytes.length) {
    const length = bytes.readUInt32BE(offset);
    const type = bytes.subarray(offset + 4, offset + 8).toString("ascii");
    const dataStart = offset + 8;
    const dataEnd = dataStart + length;
    assert(dataEnd + 4 <= bytes.length, "G4 PNG chunk is truncated.");
    if (type === "tEXt" || type === "iTXt" || type === "zTXt") {
      const metadata = bytes.subarray(dataStart, dataEnd).toString("utf8");
      assertRedactedText(metadata, "G4 PNG metadata");
      for (const canary of canaries) {
        assert(
          !canaryFragments(canary).some((fragment) => metadata.includes(fragment)),
          "G4 PNG metadata contains a Canary fragment.",
        );
      }
    }
    offset = dataEnd + 4;
    if (type === "IEND") {
      foundEnd = true;
      break;
    }
  }
  assert(foundEnd, "G4 PNG has no IEND chunk.");
  return bytes;
}

function hashFiles(files) {
  return new Map(files.map((path) => [path, sha256(readFileSync(path))]));
}

function assertHashesUnchanged(before, label) {
  for (const [path, hash] of before) {
    assert(existsSync(path), label + " file disappeared.");
    assert(sha256(readFileSync(path)) === hash, label + " file bytes changed.");
  }
}

const temporaryBase = realpathSync(tmpdir());
const temporaryRoot = realpathSync(mkdtempSync(join(temporaryBase, "novelgainker-ng407-")));
if (
  dirname(temporaryRoot) !== temporaryBase ||
  !basename(temporaryRoot).startsWith("novelgainker-ng407-") ||
  !temporaryRoot.startsWith(temporaryBase + sep)
) {
  throw new Error("NG-407 temporary root verification failed.");
}

const evidenceRoot = resolve("target", "g4-evidence");
mkdirSync(evidenceRoot, { recursive: true });
const screenshotPath = join(evidenceRoot, "tauri-g4-window.png");
const reportPath = join(evidenceRoot, "security-report.json");
const canaries = ["Brief", "Router", "Prompt", "Response", "Context", "Proposal", "Secret"].map(
  (label) => "NG407" + label + randomBytes(48).toString("base64url"),
);
assert(new Set(canaries).size === canaries.length, "G4 Canaries are not unique.");
assert(
  canaries.every((value) => value.length >= 64),
  "G4 Canary generation failed.",
);

let completed = false;
try {
  const projectRoot = join(temporaryRoot, "synthetic-project");
  const artifactRoot = join(temporaryRoot, "forbidden-artifacts");
  for (const directory of ["materials", "outlines", "chapters", "world"]) {
    mkdirSync(join(projectRoot, directory), { recursive: true });
  }
  mkdirSync(artifactRoot, { recursive: true });
  writeFileSync(
    join(projectRoot, "materials", "000.story-ledger.md"),
    "# 合成故事账本\n\n" + canaries[0] + "\n",
    "utf8",
  );
  writeFileSync(
    join(projectRoot, "outlines", "000.sprint5-sentinel.md"),
    "# 合成大纲守卫\n\n" + canaries[1] + "\n",
    "utf8",
  );
  writeFileSync(
    join(projectRoot, "chapters", "000.sprint5-sentinel.md"),
    "# 合成章节守卫\n\n" + canaries[2] + "\n",
    "utf8",
  );
  writeFileSync(
    join(projectRoot, "world", "private-canary.md"),
    "# 合成隐私来源\n\n" + canaries.slice(3).join("\n") + "\n",
    "utf8",
  );
  const protectedProjectHashes = hashFiles(walkFiles(projectRoot));

  const databasePath = join(artifactRoot, "app-state-v8.sqlite3");
  const database = new DatabaseSync(databasePath);
  database.exec(
    "CREATE TABLE router_runs (run_id TEXT, provider_id TEXT, workflow_id TEXT, confidence TEXT, status TEXT, input_tokens INTEGER, output_tokens INTEGER, total_tokens INTEGER, updated_at INTEGER)",
  );
  database
    .prepare("INSERT INTO router_runs VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
    .run(
      "70000000-0000-4000-8000-000000000007",
      "deepseek",
      "novelgainker.setting-create",
      "high",
      "completed",
      12,
      8,
      20,
      1,
    );
  database.close();
  writeFileSync(
    join(artifactRoot, "app.log"),
    "workflow.completed\nworkflow.cancelled\nworkflow.recovery_required\n",
    "utf8",
  );
  writeFileSync(
    join(artifactRoot, "error.json"),
    JSON.stringify({ code: "workflow.recovery_required", messageKey: "errors.recoveryRequired" }),
    "utf8",
  );
  writeFileSync(
    join(artifactRoot, "dom-snapshot.html"),
    "<main><p>G4 合成工作流证据</p><p>DeepSeek / OpenAI Mock</p></main>",
    "utf8",
  );
  writeFileSync(
    join(artifactRoot, "screenshot-metadata.json"),
    JSON.stringify({ schemaVersion: 1, gate: "G4", rendered: true }),
    "utf8",
  );

  if (!qualitySequenceMode) {
    runPnpm("workspace boundary audit", ["quality:boundaries"]);
    runPnpm("Rust full regression", ["test:rust"]);
    runPnpm("React full regression", ["test:frontend"]);
    runPnpm("Tauri debug build", ["build:test:debug"]);
    runPnpm("debug desktop smoke", ["smoke:desktop:debug"]);
    runPnpm("Tauri release build", ["build:test"]);
    runPnpm("G2 release desktop smoke", ["smoke:desktop"]);
    runPnpm("Sprint 2 security regression", ["e2e:sprint2"]);
    runPnpm("Sprint 3 safe apply regression", ["e2e:sprint3"]);
    for (const story of ["ng401", "ng402", "ng403", "ng404", "ng405", "ng406"]) {
      runPnpm(story.toUpperCase() + " evidence regression", ["e2e:" + story]);
    }
  }

  const resourceRoot = resolve("resources", "workflows", "v1");
  const actualResourcePaths = relativeFiles(resourceRoot);
  assert(
    JSON.stringify(actualResourcePaths) === JSON.stringify(expectedResourcePaths),
    "G4 regression Pack resource paths do not match the current twenty-eight-resource contract.",
  );
  const packHash = calculatePackHash(resourceRoot, actualResourcePaths);
  assert(
    packHash === expectedPackHash,
    "G4 Workflow Pack Hash changed without a new approved Pack.",
  );
  const manifest = JSON.parse(readFileSync(join(resourceRoot, "pack.json"), "utf8"));
  assert(manifest.schemaVersion === 1, "G4 Pack Manifest schema is not v1.");
  assert(manifest.workflowPackVersion === "1.0.0", "G4 Pack version is not 1.0.0.");
  assert(
    manifest.workflows.length === expectedWorkflows.length,
    "G4 regression Pack workflow count is not six.",
  );
  const workflowEvidence = manifest.workflows.map((workflow, index) => {
    const expected = expectedWorkflows[index];
    assert(workflow.id === expected[0], "G4 Workflow order or ID changed.");
    assert(workflow.version === expected[1], "G4 Workflow version changed.");
    assert(
      workflow.executionAvailable === expected[2],
      "G4 regression Workflow execution capability changed.",
    );
    assert(workflow.routable === expected[3], "G4 regression Workflow route capability changed.");
    return {
      id: workflow.id,
      version: workflow.version,
      contentHash: calculateWorkflowHash(resourceRoot, workflow),
    };
  });

  const reports = new Map();
  const evidenceSourceHashes = [];
  for (const [label, relativePath] of reportSources) {
    const source = readFreshReport(label, relativePath);
    reports.set(label, source.report);
    evidenceSourceHashes.push({
      source: relativePath.replaceAll("\\", "/"),
      sha256: sha256(readFileSync(source.absolutePath)),
    });
    assertRedactedText(readFileSync(source.absolutePath, "utf8"), label + " report");
  }

  const g2 = reports.get("G2");
  assert(g2.gate === "G2" && g2.mode === "mock-offline", "G2 regression evidence is invalid.");
  assert(g2.canaryLeakCount === 0, "G2 Canary leak count is not zero.");
  assert(g2.crossProviderAutomaticRequestCount === 0, "G2 automatic fallback count is not zero.");
  assert(g2.canonAutomaticWriteCount === 0, "G2 automatic Canon write count is not zero.");

  const g3 = reports.get("G3");
  assert(g3.gate === "G3" && g3.mode === "mock-offline", "G3 regression evidence is invalid.");
  assertZero(
    g3,
    [
      "unconfirmedCanonWriteCount",
      "cancelledCanonWriteCount",
      "staleCanonWriteCount",
      "conflictCanonWriteCount",
      "halfWrittenCanonCount",
      "crossProviderAutomaticRequestCount",
      "canaryLeakCount",
    ],
    "G3",
  );
  assert(g3.matrix.confirmedCreatePerRunMaximum === 1, "G3 wrote more than one file per Run.");
  assert(g3.matrix.recoveryRequiredOutcomeCovered === true, "G3 recovery coverage is missing.");

  const project = reports.get("NG-401");
  assert(project.story === "NG-401", "NG-401 report identity is invalid.");
  assert(project.workflow.id === "novelgainker.project-create", "NG-401 workflow ID changed.");
  assert(project.workflow.version === 1, "NG-401 workflow version changed.");
  assertZero(
    project,
    [
      "providerRequestCount",
      "providerRouterRequestCount",
      "vaultAccessCount",
      "prePreviewWriteCount",
      "preConfirmationWriteCount",
      "stalePreviewWriteCount",
      "duplicateOverwriteCount",
    ],
    "NG-401",
  );
  assert(project.matrix.scaffoldDirectoryCount === 11, "NG-401 scaffold directory count changed.");
  assert(project.matrix.scaffoldFileCount === 4, "NG-401 scaffold file count changed.");
  assert(project.matrix.injectedRollbackStepCount === 15, "NG-401 rollback matrix changed.");
  assertTrueChecks(project, "NG-401");

  const entityExpectations = [
    ["NG-402", "novelgainker.setting-create", 1, "SettingProposalV1"],
    ["NG-403", "novelgainker.material-create", 1, "MaterialProposalV1"],
    ["NG-404", "novelgainker.plot-create", 1, "PlotProposalV1"],
    ["NG-405", "novelgainker.character-create", 2, "CharacterProposalV2"],
  ];
  for (const [label, workflowId, version, contract] of entityExpectations) {
    const report = reports.get(label);
    assert(report.story === label, label + " report identity is invalid.");
    assert(report.mode === "mock-only-local-first", label + " did not use Mock-only mode.");
    assert(report.workflow.id === workflowId, label + " workflow ID changed.");
    assert(report.workflow.version === version, label + " workflow version changed.");
    assert(report.workflow.outputContract === contract, label + " output contract changed.");
    assert(report.matrix.mockProviderCount === 2, label + " did not cover both Mock Providers.");
    assertZero(
      report,
      [
        "crossProviderAutomaticRequestCount",
        "requestBeforeManifestConfirmationCount",
        "writeBeforeProposalConfirmationCount",
        "metadataCatalogBodyCount",
        "collisionOverwriteCount",
        "sourceChangedWriteCount",
      ],
      label,
    );
    assertTrueChecks(report, label);
  }
  assertZero(
    reports.get("NG-403"),
    [
      "hiddenDraftPersistenceCount",
      "storyLedgerWriteCount",
      "targetChangedWriteCount",
      "reservedTargetWriteCount",
      "abandonWriteCount",
    ],
    "NG-403",
  );
  assertZero(
    reports.get("NG-404"),
    [
      "hiddenDraftPersistenceCount",
      "storyLedgerWriteCount",
      "targetChangedWriteCount",
      "abandonWriteCount",
      "revealEvidenceLeakCount",
      "payoffEvidenceLeakCount",
    ],
    "NG-404",
  );
  assertZero(
    reports.get("NG-405"),
    [
      "unselectedSourceBodyCount",
      "reverseRelationshipWriteCount",
      "targetChangedWriteCount",
      "abandonWriteCount",
      "privateCharacterFieldEvidenceLeakCount",
    ],
    "NG-405",
  );

  const router = reports.get("NG-406");
  assert(router.story === "NG-406", "NG-406 report identity is invalid.");
  assert(router.router.routableWorkflowCount === 5, "NG-406 routable workflow count changed.");
  assert(router.router.deterministicCommandCount === 6, "NG-406 command count changed.");
  assert(router.router.maximumStructuralRepairCount === 1, "NG-406 repair limit changed.");
  assert(router.router.automaticTransientRetryCount === 0, "NG-406 transient retry is not zero.");
  assert(router.router.automaticFallbackCount === 0, "NG-406 fallback is not zero.");
  assertZero(
    router,
    [
      "exactCommandProviderRequestCount",
      "buttonProviderRequestCount",
      "requestBeforeConsentCount",
      "projectReadCount",
      "projectWriteCount",
      "highDecisionAutomaticExecutionCount",
      "mediumLowAutomaticFormOpenCount",
      "malformedAutomaticExecutionCount",
      "persistedUserInputCount",
      "persistedRemotePayloadCount",
    ],
    "NG-406",
  );
  assertTrueChecks(router, "NG-406");

  assertHashesUnchanged(protectedProjectHashes, "G4 protected synthetic Canon");
  const projectFiles = walkFiles(projectRoot);
  const projectBytes = Buffer.concat(projectFiles.map((path) => readFileSync(path)));
  assert(
    canaries.every((canary) => projectBytes.includes(Buffer.from(canary, "utf8"))),
    "G4 source Canaries did not remain in their protected synthetic Canon.",
  );

  runPnpm("real Tauri G4 window evidence", ["smoke:g4"]);
  assert(
    existsSync(screenshotPath) && statSync(screenshotPath).isFile(),
    "G4 screenshot is missing.",
  );
  assert(statSync(screenshotPath).mtimeMs >= runStartedAt - 1000, "G4 screenshot is stale.");
  const screenshotBytes = assertPngMetadataRedacted(screenshotPath, canaries);

  const artifactFiles = walkFiles(artifactRoot);
  for (const path of artifactFiles.filter((value) => !value.endsWith(".sqlite3"))) {
    assertRedactedText(readFileSync(path, "utf8"), "G4 synthetic artifact");
  }
  const reportFiles = reportSources.map(([, path]) => resolve(path));
  assertNoCanaries([...artifactFiles, ...reportFiles, screenshotPath], canaries);

  const resourceHashes = actualResourcePaths.map((path) => ({
    path,
    sha256: sha256(readFileSync(join(resourceRoot, ...path.split("/")))),
  }));
  const report = {
    schemaVersion: 1,
    story: "NG-407",
    gate: "G4",
    mode: "mock-only-local-first",
    generatedAt: new Date().toISOString(),
    pack: {
      schemaVersion: manifest.schemaVersion,
      workflowPackVersion: manifest.workflowPackVersion,
      packHash,
      resourceCount: actualResourcePaths.length,
      workflows: workflowEvidence,
      resources: resourceHashes,
    },
    providers: [
      { providerId: "deepseek", presetVersion: 1, model: "deepseek-v4-pro" },
      { providerId: "openai", presetVersion: 1, model: "gpt-5.6-terra" },
    ],
    matrix: {
      workflowCount: 6,
      aiEntityWorkflowCount: 4,
      mockProviderCount: 2,
      projectScaffoldDirectoryCount: 11,
      projectScaffoldFileCount: 4,
      projectRollbackStepCount: 15,
      confirmedEntityCanonWriteCount: 8,
      confirmedCanonWritePerRunMaximum: 1,
      serializerGoldenMismatchCount: 0,
      unconfirmedCanonWriteCount: 0,
      cancelledCanonWriteCount: 0,
      staleCanonWriteCount: 0,
      conflictCanonWriteCount: 0,
      invalidRelationshipWriteCount: 0,
      validationFailureWriteCount: 0,
      halfWrittenCanonCount: 0,
      silentOverwriteCount: 0,
      automaticNumberingCount: 0,
      backgroundMigrationCount: 0,
      storyLedgerWriteCount: 0,
      sprint5SentinelMutationCount: 0,
      explicitRouteProviderRequestCount: 0,
      ambiguousRouteCanonWriteCount: 0,
      crossProviderAutomaticRequestCount: 0,
      externalUserProjectReadCount: 0,
      realCredentialReadCount: 0,
      sensitiveCanaryLeakCount: 0,
    },
    regressions: {
      sprint1OfflineEditor: "passed",
      sprint2ProviderSecurity: "passed",
      sprint3SafeApplyRecovery: "passed",
      appStateDeleteCorruptionOfflineEdit: "passed",
      debugBuildAndWindow: "passed",
      releaseBuildAndWindow: "passed",
      ordinaryProviderChat: "passed",
    },
    coverage: [
      "fixed-pack-resource-version-hash",
      "project-preview-confirm-transaction-rollback-conflict",
      "four-entity-dual-loopback-schema-serializer",
      "context-manifest-id-relation-v1-upgrade",
      "proposal-apply-cancel-stale-conflict-recovery",
      "router-local-consent-confidence-offline-selector",
      "sqlite-log-error-dom-png-metadata-canary-scan",
      "sprint1-sprint2-sprint3-full-regression",
      "real-tauri-g4-window",
    ],
    evidenceSources: evidenceSourceHashes,
    windowEvidenceSha256: sha256(screenshotBytes),
  };
  const reportText = JSON.stringify(report, null, 2) + "\n";
  assertRedactedText(reportText, "G4 report");
  writeFileSync(reportPath, reportText, "utf8");
  assertNoCanaries([reportPath], canaries);
  completed = true;
  console.log(
    "[g4-e2e] PASS workflowCount=6 routableWorkflowCount=5 mockProviderCount=2 confirmedCanonWritePerRunMaximum=1 negativePathWriteCount=0 crossProviderAutomaticRequestCount=0 sensitiveCanaryLeakCount=0",
  );
  console.log("[g4-e2e] EVIDENCE target/g4-evidence/security-report.json");
  console.log("[g4-e2e] EVIDENCE target/g4-evidence/tauri-g4-window.png");
} catch (error) {
  const message = error instanceof Error ? error.message : "unknown failure";
  console.error("[g4-e2e] FAIL " + message);
  process.exitCode = 1;
} finally {
  const cleanupVerified =
    dirname(temporaryRoot) === temporaryBase &&
    basename(temporaryRoot).startsWith("novelgainker-ng407-");
  if (!cleanupVerified) {
    console.error("[g4-e2e] FAIL refusing to clean an unverified temporary root");
    process.exitCode = 1;
  } else {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
}
if (!completed) process.exitCode = 1;
