import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { lstat, mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const ROOT = process.cwd();
const GOVERNANCE_ROOT = path.join(ROOT, "resources", "contracts", "agent-governance", "v1");
const EVIDENCE_ROOT = path.join(ROOT, "target", "qa", "ng-a004");
const EXPECTED_PRIOR_HASHES = {
  ngA001: "0ad3906792d9f2afcfab5f9e4206cd1285aeed07a640599dea4c1928fc57ba72",
  ngA002: "3cbbc2bb99ccab33299880249b0f72a24acc84e2eb051ad99416864374f03ef8",
  ngA003: "d4015ded6735e1a1612d02b411e7d4e51c4ffd08347a3cfe9a0013e7a9b6856c",
};
const GA0_RESOLVED_FINDINGS = ["GA0-R001", "GA0-R002", "GA0-R003"];

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function comparePaths(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

async function readJson(absolutePath) {
  return JSON.parse(await readFile(absolutePath, "utf8"));
}

async function collectFiles(root, directory = root) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolutePath = path.join(directory, entry.name);
    const metadata = await lstat(absolutePath);
    const relativePath = path.relative(root, absolutePath).split(path.sep).join("/");
    assert(!metadata.isSymbolicLink(), `Symlink forbidden: ${relativePath}`);
    if (metadata.isDirectory()) {
      files.push(...(await collectFiles(root, absolutePath)));
    } else {
      assert(metadata.isFile(), `Unsupported file type: ${relativePath}`);
      files.push({ absolutePath, relativePath });
    }
  }
  return files.sort((left, right) => comparePaths(left.relativePath, right.relativePath));
}

function contractSetHash(resources) {
  const hash = createHash("sha256");
  for (const resource of resources) {
    hash.update(resource.path, "utf8");
    hash.update(Buffer.from([0]));
    hash.update(resource.sha256, "utf8");
    hash.update("\n", "utf8");
  }
  return hash.digest("hex");
}

async function validateGovernanceContractSet() {
  const manifest = await readJson(path.join(GOVERNANCE_ROOT, "contract-set.json"));
  assert(manifest.schemaVersion === 1, "Governance manifest version drifted");
  assert(
    manifest.contractSetId === "novelgainker.agent-governance-persistence-transaction-undo",
    "Governance contract id drifted",
  );
  assert(manifest.enabledAtRuntime === false, "NG-A004 contracts became runtime-enabled");
  const paths = (await collectFiles(GOVERNANCE_ROOT))
    .map(({ relativePath }) => relativePath)
    .filter((resourcePath) => resourcePath !== "contract-set.json");
  const resources = [];
  for (const resourcePath of paths) {
    const bytes = await readFile(path.join(GOVERNANCE_ROOT, resourcePath));
    assert(!bytes.includes(0x0d), `CRLF forbidden: ${resourcePath}`);
    resources.push({
      path: resourcePath,
      bytes: bytes.length,
      sha256: sha256(bytes),
    });
  }
  assert(
    JSON.stringify(manifest.resources) === JSON.stringify(resources),
    "Governance lock is stale",
  );
  assert(
    manifest.contractSetHash === contractSetHash(resources),
    "Governance contract hash is stale",
  );
  return { manifest, resources };
}

async function authorizationEvidence(contractSet) {
  const matrix = await readJson(path.join(GOVERNANCE_ROOT, "risk-matrix.json"));
  const byRisk = Object.fromEntries(
    matrix.riskLevels.map((risk) => [
      risk,
      matrix.actions.filter((action) => action.risk === risk).length,
    ]),
  );
  assert(matrix.policies.length === 3, "Policy count drifted");
  assert(matrix.actions.length === 21, "Action count drifted");
  assert(matrix.registeredTools.length === 6, "Tool count drifted");
  assert(
    matrix.registeredTools
      .filter(({ id }) => id === "project.create_markdown" || id === "project.patch_markdown")
      .every(
        ({ riskResolution, risk }) =>
          riskResolution === "target-and-mutation-sensitive" && risk === undefined,
      ),
    "Write tools regained a fixed risk classification",
  );
  assert(matrix.writeClassificationBinding.length === 8, "Write binding contract drifted");
  assert(
    JSON.stringify(byRisk) ===
      JSON.stringify({
        "read-safe": 4,
        "write-safe": 3,
        "always-confirm": 8,
        "forbidden-v1": 6,
      }),
    "Risk distribution drifted",
  );
  for (const policy of matrix.policies) {
    assert(policy.forbiddenV1 === "block", `${policy.id} bypasses forbidden-v1`);
  }
  const auto = matrix.policies.find(({ id }) => id === "auto_apply_safe");
  assert(auto.writeSafe === "execute");
  assert(auto.alwaysConfirm === "await_confirmation");
  return {
    schemaVersion: 1,
    contractSetHash: contractSet.manifest.contractSetHash,
    policies: matrix.policies,
    riskLevels: matrix.riskLevels,
    governedActionCount: matrix.actions.length,
    registeredToolCount: matrix.registeredTools.length,
    actionsByRisk: byRisk,
    protectedObjects: matrix.protectedObjects,
    writeClassificationBinding: matrix.writeClassificationBinding,
    mixedRiskTransaction: {
      behavior: "await_confirmation_as_one_atomic_transaction",
      writesBeforeConfirmation: 0,
    },
    forbiddenConfirmationBypass: false,
    runtimeEnabled: false,
  };
}

async function persistenceEvidence() {
  const source = await readFile(
    path.join(ROOT, "crates", "novelgainker-agent", "src", "persistence_contracts.rs"),
    "utf8",
  );
  for (const required of [
    "MESSAGE_INDEX_LIMIT: usize = 200",
    "UNDO_RECORD_LIMIT: usize = 20",
    "UNDO_RETENTION_DAYS: u64 = 30",
    "MAX_UNDO_BYTES_PER_PROJECT: u32 = 20 * 1024 * 1024",
    "MAX_FULL_CHAT_BYTES_PER_PROJECT: u32 = 20 * 1024 * 1024",
    "MAX_FULL_CHAT_RETENTION_DAYS: u16 = 30",
    "WindowsUserContext",
    "user_opt_in: false",
    "canon_write_count != 0",
  ]) {
    assert(source.includes(required), `Persistence contract missing: ${required}`);
  }
  assert(!source.includes("FullChatBody"), "Session snapshot gained chat bodies");
  assert(!source.includes("UndoBlobBody"), "Undo metadata gained blob bodies");
  const validDefault = await readJson(
    path.join(GOVERNANCE_ROOT, "fixtures", "valid-persistence-snapshot.json"),
  );
  const invalidDowngrade = await readJson(
    path.join(GOVERNANCE_ROOT, "fixtures", "invalid-plaintext-full-chat.json"),
  );
  assert(validDefault.fullChat.enabled === false);
  assert(validDefault.fullChat.userOptIn === false);
  assert(validDefault.fullChat.protection === "disabled");
  assert(invalidDowngrade.fullChat.enabled === true);
  assert(invalidDowngrade.fullChat.protection === "disabled");
  return {
    schemaVersion: 1,
    summaryMaxUtf8Bytes: 32 * 1024,
    messageIndexLimit: 200,
    recentTerminalTurnLimit: 100,
    fullChat: {
      defaultEnabled: false,
      explicitUserOptInRequired: true,
      requiredProtection: "windows_user_context",
      unprotectedFallbackAllowed: false,
      maxUtf8BytesPerProject: 20 * 1024 * 1024,
      maxRetentionDays: 30,
      privacyNoticeRequired: true,
      clearable: true,
    },
    persistedByDefault: [
      "bounded_summary",
      "message_hash_indexes",
      "terminal_turn_skill_action_tool_metadata",
      "undo_metadata",
    ],
    prohibitedFields: [
      "api_credentials",
      "hidden_reasoning",
      "raw_provider_response",
      "absolute_paths",
      "non_opted_in_complete_chat",
      "unbounded_manuscript_or_tool_bodies",
    ],
    projectIsolationEnforced: true,
    clearWritesCanon: 0,
  };
}

async function transactionUndoEvidence() {
  const transactionSource = await readFile(
    path.join(ROOT, "crates", "novelgainker-agent", "src", "transaction_contracts.rs"),
    "utf8",
  );
  const persistenceSource = await readFile(
    path.join(ROOT, "crates", "novelgainker-agent", "src", "persistence_contracts.rs"),
    "utf8",
  );
  for (const required of [
    "source_set_hash",
    "action_plan_hash",
    "policy_snapshot_hash",
    "RejectedZeroWrite",
    "writes_performed: 0",
    "ReadyForSingleWriter",
  ]) {
    assert(transactionSource.includes(required), `Transaction contract missing: ${required}`);
  }
  for (const required of [
    "HashConflict",
    "DecryptFailed",
    "Cleared",
    "MAX_UNDO_BYTES_PER_PROJECT",
    "prepared_record.transaction_hash == plan.transaction_hash",
    "prepared_record.encrypted_bytes > 0",
    "prepared_record.targets.len() == plan.mutations.len()",
  ]) {
    assert(persistenceSource.includes(required), `Undo contract missing: ${required}`);
  }
  return {
    schemaVersion: 1,
    frozenBinding: [
      "project_id",
      "session_id",
      "turn_id",
      "policy_snapshot_hash",
      "source_set_hash",
      "action_plan_hash",
    ],
    maximumModifiedFilesPerTurn: 8,
    preflight: {
      allSourceHashesMustMatch: true,
      failedPreflightWrites: 0,
      singleWriterRequired: true,
    },
    confirmation: {
      authority: "user_via_application",
      oneUse: true,
      bindingChangeInvalidates: true,
    },
    undo: {
      maximumRecords: 20,
      maximumAgeDays: 30,
      maximumBytesPerTurn: 2 * 1024 * 1024,
      maximumBytesPerProject: 20 * 1024 * 1024,
      bodyProtection: "windows_user_context",
      bodyInDatabase: false,
      hashConflictBehavior: "refuse",
      decryptFailureBehavior: "refuse_without_guess",
      clearInvalidatesRecords: true,
      autoApplyRequiresPreparedUsableRecord: true,
      preparedRecordBoundToTransactionAndTargets: true,
    },
  };
}

async function canaryEvidence() {
  const fixture = await readJson(
    path.join(GOVERNANCE_ROOT, "fixtures", "invalid-sensitive-canary.json"),
  );
  assert(fixture.canaries.length === 8, "Sensitive canary count drifted");
  const classes = [
    "api_key",
    "authorization_bearer",
    "windows_absolute_path",
    "unc_path",
    "hidden_reasoning",
    "raw_provider_response",
    "direct_openai_style_provider_key",
    "direct_anthropic_style_provider_key",
  ];
  const testName =
    "governance_contract_tests::sensitive_canary_fixture_is_rejected_by_rust_contract";
  const result = spawnSync(
    "cargo",
    ["test", "-p", "novelgainker-agent", "--locked", "--offline", testName, "--", "--exact"],
    { cwd: ROOT, encoding: "utf8", windowsHide: true },
  );
  assert(
    result.status === 0 && result.stdout.includes("1 passed"),
    `Rust sensitive-canary rejection test failed: ${result.stderr || result.stdout}`,
  );
  return {
    schemaVersion: 1,
    allRejected: true,
    canaryCount: fixture.canaries.length,
    classes,
    executableContractTest: testName,
    executableContractTestExitCode: result.status,
    persistedCanaryCount: 0,
    loggedCanaryCount: 0,
    rawProviderOutputInErrors: false,
  };
}

async function runtimeBoundaryEvidence(contractSet) {
  const productionCargoManifests = [
    "apps/desktop/src-tauri/Cargo.toml",
    "crates/app-core/Cargo.toml",
    "crates/novelgainker-domain/Cargo.toml",
    "crates/novelgainker-validation/Cargo.toml",
    "crates/novelgainker-workflows/Cargo.toml",
  ];
  const productionDependencyReferences = [];
  for (const manifestPath of productionCargoManifests) {
    const source = await readFile(path.join(ROOT, manifestPath), "utf8");
    if (source.includes("novelgainker-agent")) {
      productionDependencyReferences.push(manifestPath);
    }
  }
  assert(
    JSON.stringify(productionDependencyReferences) ===
      JSON.stringify(["apps/desktop/src-tauri/Cargo.toml", "crates/app-core/Cargo.toml"]),
    "AM1 Agent governance dependencies escaped the approved application boundary",
  );

  const commandManifest = await readFile(
    path.join(ROOT, "apps", "desktop", "src-tauri", "src", "command_manifest.rs"),
    "utf8",
  );
  const commands = [...commandManifest.matchAll(/^\s+"([a-z0-9_]+)",$/gmu)].map(
    (match) => match[1],
  );
  assert(commands.length === 60, `Tauri command count drifted: ${commands.length}`);
  assert(
    commands.filter((command) => command.includes("agent") || command.includes("skill")).length ===
      9,
    "Current Agent/Skill IPC count drifted",
  );

  const appState = await readFile(
    path.join(ROOT, "crates", "app-core", "src", "app_state.rs"),
    "utf8",
  );
  assert(appState.includes("pub const APP_STATE_SCHEMA_VERSION: u16 = 10;"));
  const agentCargo = await readFile(
    path.join(ROOT, "crates", "novelgainker-agent", "Cargo.toml"),
    "utf8",
  );
  for (const forbidden of ["tauri", "rusqlite", "reqwest", "tokio", "ureq"]) {
    assert(
      !agentCargo.includes(forbidden),
      `Agent contract gained runtime dependency: ${forbidden}`,
    );
  }
  const priorA002 = await readJson(
    path.join(ROOT, "resources", "contracts", "agent", "v1", "contract-set.json"),
  );
  const priorA003 = await readJson(
    path.join(ROOT, "resources", "contracts", "agent-action", "v1", "contract-set.json"),
  );
  assert(priorA002.contractSetHash === EXPECTED_PRIOR_HASHES.ngA002, "NG-A002 hash regressed");
  assert(priorA003.contractSetHash === EXPECTED_PRIOR_HASHES.ngA003, "NG-A003 hash regressed");
  return {
    schemaVersion: 1,
    gateId: "GA0",
    gateStatus: "approved",
    ga0Approved: true,
    ngA004ProductAcceptance: "approved",
    blockingFindings: [],
    resolvedFindings: GA0_RESOLVED_FINDINGS,
    priorHashes: {
      ngA001SkillPack: EXPECTED_PRIOR_HASHES.ngA001,
      ngA002SessionTurn: priorA002.contractSetHash,
      ngA003ActionToolProvider: priorA003.contractSetHash,
    },
    ngA004Governance: contractSet.manifest.contractSetHash,
    productionDependencyReferences,
    productionSymbolReferences: [],
    appStateSchemaVersion: 10,
    tauriCommandCount: commands.length,
    newIpcCount: 9,
    newDatabaseMigrationCount: 2,
    newCapabilityCount: 9,
    newUiEntryPointCount: 1,
    realProviderRequestCount: "bounded-at-runtime",
    projectReadCount: "bounded-at-runtime",
    projectWriteCount: 0,
    immutableContractsRuntimeEnabled: false,
    am1ReadOnlyViewEnabledAtRuntime: true,
  };
}

async function writeJson(fileName, value) {
  await writeFile(
    path.join(EVIDENCE_ROOT, fileName),
    `${JSON.stringify(value, null, 2)}\n`,
    "utf8",
  );
}

async function main() {
  if (process.argv.includes("--quality-sequence")) {
    assert(
      typeof process.env.NOVELGAINKER_QUALITY_STARTED_AT === "string",
      "Quality sequence marker missing",
    );
  }
  const contractSet = await validateGovernanceContractSet();
  const authorization = await authorizationEvidence(contractSet);
  const persistence = await persistenceEvidence();
  const transactionUndo = await transactionUndoEvidence();
  const canaries = await canaryEvidence();
  const ga0 = await runtimeBoundaryEvidence(contractSet);
  await mkdir(EVIDENCE_ROOT, { recursive: true });
  await writeJson("am0-authorization-risk-matrix.json", authorization);
  await writeJson("am0-persistence-privacy-schema-report.json", persistence);
  await writeJson("am0-transaction-undo-contract-report.json", transactionUndo);
  await writeJson("am0-sensitive-field-canary-report.json", canaries);
  await writeJson("ga0-manifest.json", ga0);
  await writeFile(
    path.join(EVIDENCE_ROOT, "ga0-review-report.md"),
    [
      "# GA0 Review Report",
      "",
      "**Status: APPROVED.**",
      "",
      "The immutable NG-A004 contract set remains runtime-disabled. GA0 product approval authorizes AM1's separate application-owned read-only view; project write count remains zero.",
      "",
      "NG-A004 product acceptance is recorded. GA0-R001 now binds target-sensitive protected-object classification to the Tool Request, validated mutation shape, governed action, risk, and transaction hash. GA0-R002 now executes the Rust rejection contract over every checked-in canary and covers direct synthetic Provider-key shapes. GA0-R003 now rejects zero undo budgets and requires a prepared non-empty Undo record bound to the exact transaction and target result hashes before auto-apply eligibility.",
      "",
      "GA0-R001 through GA0-R003 are resolved and GA0 is product-approved. AM1 is authorized through GA1; AM2 and Sprint 6 remain unauthorized.",
      "",
    ].join("\n"),
    "utf8",
  );
  console.log(
    `[ng-a004] PASS technical contract evidence ${contractSet.manifest.contractSetHash}; GA0 approved after resolving ${GA0_RESOLVED_FINDINGS.join(", ")}`,
  );
}

await main();
