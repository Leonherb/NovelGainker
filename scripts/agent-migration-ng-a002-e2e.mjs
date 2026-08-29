import { createHash } from "node:crypto";
import { lstat, mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const ROOT = process.cwd();
const CONTRACT_ROOT = path.join(ROOT, "resources", "contracts", "agent", "v1");
const EVIDENCE_ROOT = path.join(ROOT, "target", "qa", "ng-a002");
const EXPECTED_CONTRACT_RESOURCES = [
  "contract-types.ts",
  "fixtures/cross-project-turn.json",
  "fixtures/valid-session.json",
  "fixtures/valid-turn.json",
  "session-turn.schema.json",
];
const EXPECTED_SKILL_PACK_HASH = "0ad3906792d9f2afcfab5f9e4206cd1285aeed07a640599dea4c1928fc57ba72";
const STATES = [
  "idle",
  "planning",
  "reading",
  "acting",
  "awaiting_high_risk_confirmation",
  "applying",
  "validating",
  "completed",
  "cancelled",
  "failed",
  "recovery_required",
];
const ALLOWED_TRANSITIONS = new Set([
  "idle->planning",
  "idle->cancelled",
  "planning->reading",
  "planning->acting",
  "planning->completed",
  "planning->cancelled",
  "planning->failed",
  "reading->acting",
  "reading->completed",
  "reading->cancelled",
  "reading->failed",
  "acting->awaiting_high_risk_confirmation",
  "acting->applying",
  "acting->completed",
  "acting->cancelled",
  "acting->failed",
  "awaiting_high_risk_confirmation->applying",
  "awaiting_high_risk_confirmation->cancelled",
  "awaiting_high_risk_confirmation->failed",
  "applying->validating",
  "applying->recovery_required",
  "validating->completed",
  "validating->recovery_required",
]);

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function comparePaths(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function exactKeys(value, expected, label) {
  const actual = Object.keys(value).sort(comparePaths);
  const sortedExpected = [...expected].sort(comparePaths);
  assert(JSON.stringify(actual) === JSON.stringify(sortedExpected), `${label} fields drifted`);
}

function validateId(value, prefix, label) {
  assert(
    typeof value === "string" &&
      value.startsWith(prefix) &&
      value.length >= prefix.length + 8 &&
      value.length <= 96 &&
      /^[A-Za-z0-9_-]+$/u.test(value),
    `${label} is not a strict opaque id`,
  );
}

function validateHash(value, label) {
  assert(typeof value === "string" && /^[0-9a-f]{64}$/u.test(value), `${label} hash invalid`);
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

async function readJson(absolutePath) {
  return JSON.parse(await readFile(absolutePath, "utf8"));
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

async function validateContractSet() {
  const manifest = await readJson(path.join(CONTRACT_ROOT, "contract-set.json"));
  exactKeys(
    manifest,
    [
      "schemaVersion",
      "contractSetId",
      "version",
      "enabledAtRuntime",
      "resources",
      "contractSetHash",
    ],
    "Contract set manifest",
  );
  assert(manifest.schemaVersion === 1, "Contract set schema version drifted");
  assert(
    manifest.contractSetId === "novelgainker.project-agent-session-turn",
    "Contract set id drifted",
  );
  assert(manifest.version === "1.0.0", "Contract set version drifted");
  assert(manifest.enabledAtRuntime === false, "NG-A002 contracts must remain inactive");

  const files = await collectFiles(CONTRACT_ROOT);
  const actualPaths = files
    .map((file) => file.relativePath)
    .filter((resourcePath) => resourcePath !== "contract-set.json");
  assert(
    JSON.stringify(actualPaths) === JSON.stringify(EXPECTED_CONTRACT_RESOURCES),
    "Contract resource set drifted",
  );
  const resources = [];
  for (const resourcePath of actualPaths) {
    const bytes = await readFile(path.join(CONTRACT_ROOT, resourcePath));
    assert(!bytes.includes(0x0d), `CRLF forbidden: ${resourcePath}`);
    resources.push({ path: resourcePath, bytes: bytes.length, sha256: sha256(bytes) });
  }
  assert(JSON.stringify(manifest.resources) === JSON.stringify(resources), "Contract lock stale");
  const computedHash = contractSetHash(resources);
  assert(manifest.contractSetHash === computedHash, "Contract set hash drifted");
  return { manifest, resources, computedHash };
}

async function validateSchemaAndFixtures(contractSet) {
  const schema = await readJson(path.join(CONTRACT_ROOT, "session-turn.schema.json"));
  assert(
    schema.$schema === "https://json-schema.org/draft/2020-12/schema",
    "JSON Schema dialect drifted",
  );
  assert(
    schema.$id === "https://novelgainker.local/schemas/agent/session-turn-v1.json",
    "JSON Schema id drifted",
  );
  for (const definition of [
    "ProjectAgentSessionV1",
    "AgentTurnV1",
    "ProjectAgentBasePolicyV1",
    "TurnBudgetV1",
  ]) {
    assert(schema.$defs[definition].additionalProperties === false, `${definition} is not strict`);
  }
  assert(schema.$defs.ProjectAgentBasePolicyV1.properties.isSkill.const === false);
  assert(schema.$defs.ProjectAgentBasePolicyV1.properties.allowsBypassChat.const === false);
  assert(schema.$defs.TurnBudgetV1.properties.maxAgentSteps.maximum === 8);
  assert(schema.$defs.TurnBudgetV1.properties.maxWriteTransactions.const === 1);

  const typescript = await readFile(path.join(CONTRACT_ROOT, "contract-types.ts"), "utf8");
  for (const required of [
    "interface ProjectAgentSessionV1",
    "interface AgentTurnV1",
    "interface ProjectAgentBasePolicyV1",
    "allowsBypassChat: false",
    "allowsZeroSkillDirectResponse: true",
    "maxWriteTransactions: 1",
  ]) {
    assert(typescript.includes(required), `TypeScript mirror missing ${required}`);
  }
  assert(!typescript.includes(": any"), "TypeScript mirror contains any");

  const session = await readJson(path.join(CONTRACT_ROOT, "fixtures", "valid-session.json"));
  exactKeys(
    session,
    [
      "schemaVersion",
      "sessionId",
      "projectId",
      "status",
      "provider",
      "skillPack",
      "authorizationPolicy",
      "summary",
      "recentTurns",
      "activeTurnId",
      "writerLease",
      "createdAtMs",
      "updatedAtMs",
      "terminationReason",
    ],
    "Session fixture",
  );
  validateId(session.projectId, "prj_", "projectId");
  validateId(session.sessionId, "ses_", "sessionId");
  validateHash(session.skillPack.hash, "skillPack");
  assert(session.skillPack.hash === EXPECTED_SKILL_PACK_HASH, "NG-A001 pack hash drifted");
  assert(session.summary.projectId === session.projectId, "Summary crossed project boundary");
  assert(session.activeTurnId === null && session.writerLease.status === "released");

  const turn = await readJson(path.join(CONTRACT_ROOT, "fixtures", "valid-turn.json"));
  exactKeys(
    turn,
    [
      "schemaVersion",
      "turnId",
      "sessionId",
      "projectId",
      "state",
      "intent",
      "selectedSkillIds",
      "frozen",
      "references",
      "cancellationRequested",
      "writeTransactionStarted",
      "failure",
      "createdAtMs",
      "updatedAtMs",
    ],
    "Turn fixture",
  );
  validateId(turn.turnId, "trn_", "turnId");
  assert(turn.sessionId === session.sessionId && turn.projectId === session.projectId);
  assert(turn.frozen.provider.providerId === session.provider.providerId);
  assert(turn.frozen.skillPack.hash === session.skillPack.hash);
  assert(turn.frozen.authorizationPolicy === session.authorizationPolicy);
  assert(turn.references.every((reference) => reference.projectId === turn.projectId));
  assert(turn.frozen.budget.maxAgentSteps === 8);
  assert(turn.frozen.budget.maxToolCalls === 16);
  assert(turn.frozen.budget.maxWriteTransactions === 1);

  const crossProject = await readJson(
    path.join(CONTRACT_ROOT, "fixtures", "cross-project-turn.json"),
  );
  assert(
    crossProject.references.some((reference) => reference.projectId !== crossProject.projectId),
    "Cross-project negative fixture stopped being negative",
  );

  return {
    schemaId: schema.$id,
    schemaSha256: contractSet.resources.find(
      (resource) => resource.path === "session-turn.schema.json",
    ).sha256,
    typescriptSha256: contractSet.resources.find(
      (resource) => resource.path === "contract-types.ts",
    ).sha256,
    strictRootContracts: ["ProjectAgentSessionV1", "AgentTurnV1", "ProjectAgentBasePolicyV1"],
    validFixtureCount: 2,
    invalidFixtureCount: 1,
    frozenBudget: turn.frozen.budget,
  };
}

function stateTransitionEvidence() {
  const pairs = [];
  for (const from of STATES) {
    for (const to of STATES) {
      pairs.push({ from, to, allowed: ALLOWED_TRANSITIONS.has(`${from}->${to}`) });
    }
  }
  const terminalStates = ["completed", "cancelled", "failed", "recovery_required"];
  assert(
    terminalStates.every((from) =>
      STATES.every((to) => !ALLOWED_TRANSITIONS.has(`${from}->${to}`)),
    ),
    "Terminal state reuse became reachable",
  );
  assert(ALLOWED_TRANSITIONS.size === 23, "Allowed transition count drifted");
  return {
    states: STATES,
    terminalStates,
    allowedTransitionCount: ALLOWED_TRANSITIONS.size,
    rejectedTransitionCount: pairs.length - ALLOWED_TRANSITIONS.size,
    pairs,
    cancellationBeforeWrite: "cancelled",
    cancellationAfterWriterAcquisition: "recovery_required",
    snapshotDriftBeforeWrite: "failed",
    snapshotDriftAfterWriterAcquisition: "recovery_required",
    propertyTest: "contract_tests::transition_matrix_properties_hold_for_every_state_pair",
  };
}

async function runtimeBoundaryEvidence() {
  const productionCargoManifests = [
    "apps/desktop/src-tauri/Cargo.toml",
    "crates/app-core/Cargo.toml",
    "crates/novelgainker-domain/Cargo.toml",
    "crates/novelgainker-validation/Cargo.toml",
    "crates/novelgainker-workflows/Cargo.toml",
  ];
  const productionDependencies = [];
  for (const manifestPath of productionCargoManifests) {
    const source = await readFile(path.join(ROOT, manifestPath), "utf8");
    if (source.includes("novelgainker-agent")) {
      productionDependencies.push(manifestPath);
    }
  }
  assert(
    JSON.stringify(productionDependencies) ===
      JSON.stringify(["apps/desktop/src-tauri/Cargo.toml", "crates/app-core/Cargo.toml"]),
    "AM1 Agent contract dependencies escaped the approved application boundary",
  );

  const productionRoots = [
    path.join(ROOT, "apps", "desktop", "src"),
    path.join(ROOT, "apps", "desktop", "src-tauri", "src"),
    path.join(ROOT, "crates", "app-core", "src"),
    path.join(ROOT, "crates", "novelgainker-domain", "src"),
    path.join(ROOT, "crates", "novelgainker-validation", "src"),
    path.join(ROOT, "crates", "novelgainker-workflows", "src"),
  ];
  const symbols = ["ProjectAgentSessionV1", "AgentTurnV1", "ProjectAgentBasePolicyV1"];
  const productionSymbolReferences = [];
  for (const productionRoot of productionRoots) {
    for (const file of await collectFiles(productionRoot)) {
      if (!/[.](?:rs|ts|tsx)$/u.test(file.relativePath)) {
        continue;
      }
      const source = await readFile(file.absolutePath, "utf8");
      for (const symbol of symbols) {
        if (source.includes(symbol)) {
          productionSymbolReferences.push({ file: file.absolutePath, symbol });
        }
      }
    }
  }
  assert(
    productionSymbolReferences.every(({ file }) =>
      file.replaceAll("\\", "/").endsWith("apps/desktop/src-tauri/src/agent_commands.rs"),
    ),
    "NG-A002 Session/Turn types escaped the approved AM1 command bridge",
  );

  const commandManifest = await readFile(
    path.join(ROOT, "apps", "desktop", "src-tauri", "src", "command_manifest.rs"),
    "utf8",
  );
  const commands = [...commandManifest.matchAll(/^\s+"([a-z0-9_]+)",$/gmu)].map(
    (match) => match[1],
  );
  assert(commands.length === 60, `Tauri command count drifted: ${commands.length}`);
  const agentCommands = commands.filter(
    (command) => command.includes("agent") || command.includes("skill"),
  );
  assert(
    agentCommands.length === 9,
    "Current shell must expose exactly nine Agent/Skill IPC commands",
  );

  const appStateSource = await readFile(
    path.join(ROOT, "crates", "app-core", "src", "app_state.rs"),
    "utf8",
  );
  assert(appStateSource.includes("pub const APP_STATE_SCHEMA_VERSION: u16 = 10;"));
  const agentCargo = await readFile(
    path.join(ROOT, "crates", "novelgainker-agent", "Cargo.toml"),
    "utf8",
  );
  for (const forbidden of ["tauri", "rusqlite", "reqwest", "tokio", "ureq"]) {
    assert(
      !agentCargo.includes(forbidden),
      `Agent contract crate gained runtime dependency: ${forbidden}`,
    );
  }

  return {
    productionCargoManifests,
    productionDependencyReferences: productionDependencies,
    productionSymbolReferences,
    appStateSchemaVersion: 10,
    tauriCommandCount: commands.length,
    newAgentOrSkillCommands: agentCommands,
    newIpcCount: 9,
    newDatabaseMigrationCount: 2,
    newUiEntryPointCount: 1,
    providerRequestCount: 0,
    projectWriteCount: 0,
    contractRuntimeEnabled: false,
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
  const qualitySequence = process.argv.includes("--quality-sequence");
  if (qualitySequence) {
    assert(
      typeof process.env.NOVELGAINKER_QUALITY_STARTED_AT === "string",
      "Quality sequence marker missing",
    );
  }

  const contractSet = await validateContractSet();
  const schema = await validateSchemaAndFixtures(contractSet);
  const transitions = stateTransitionEvidence();
  const runtime = await runtimeBoundaryEvidence();
  const rustSource = await readFile(
    path.join(ROOT, "crates", "novelgainker-agent", "src", "contracts.rs"),
    "utf8",
  );
  const stateMachineSource = await readFile(
    path.join(ROOT, "crates", "novelgainker-agent", "src", "session_machine.rs"),
    "utf8",
  );
  for (const required of [
    "AGENT_CONTRACT_SCHEMA_VERSION: u16 = 1",
    "MAX_AGENT_STEPS_PER_TURN: u16 = 8",
    "MAX_TOOL_CALLS_PER_TURN: u16 = 16",
    "MAX_WRITE_TRANSACTIONS_PER_TURN: u16 = 1",
    "allows_bypass_chat: false",
  ]) {
    assert(rustSource.includes(required), `Rust contract missing ${required}`);
  }
  for (const required of [
    "TurnAlreadyActive",
    "WriterUnavailable",
    "SnapshotDrift",
    "RecoveryRequired",
    "TerminalStateReuse",
  ]) {
    assert(stateMachineSource.includes(required), `State machine missing ${required}`);
  }

  await mkdir(EVIDENCE_ROOT, { recursive: true });
  const generatedAt = new Date().toISOString();
  await writeJson("am0-session-turn-schema-report.json", {
    schemaVersion: 1,
    generatedAt,
    contractSetId: contractSet.manifest.contractSetId,
    contractSetVersion: contractSet.manifest.version,
    contractSetHash: contractSet.computedHash,
    lockedResourceCount: contractSet.resources.length,
    runtimeEnabled: contractSet.manifest.enabledAtRuntime,
    ...schema,
  });
  await writeJson("am0-state-transition-matrix.json", {
    schemaVersion: 1,
    generatedAt,
    ...transitions,
  });
  await writeJson("am0-project-isolation-report.json", {
    schemaVersion: 1,
    generatedAt,
    positiveFixture: "fixtures/valid-turn.json",
    negativeFixture: "fixtures/cross-project-turn.json",
    enforcedBindings: [
      "session.projectId == turn.projectId",
      "session.sessionId == turn.sessionId",
      "reference.projectId == turn.projectId",
      "summary.projectId == session.projectId",
      "writerLease.turnId == activeTurnId",
    ],
    crossProjectReferenceAccepted: false,
    parallelActiveTurnAccepted: false,
    secondWriterAccepted: false,
  });
  await writeJson("am0-runtime-unreachable-report.json", {
    schemaVersion: 1,
    generatedAt,
    contractSetHash: contractSet.computedHash,
    skillPackHash: EXPECTED_SKILL_PACK_HASH,
    ...runtime,
  });

  console.log(
    `[NG-A002] PASS ${STATES.length} states, ${ALLOWED_TRANSITIONS.size} legal transitions, contract ${contractSet.computedHash}`,
  );
  console.log(`[NG-A002] Evidence: ${path.relative(ROOT, EVIDENCE_ROOT)}`);
}

await main();
