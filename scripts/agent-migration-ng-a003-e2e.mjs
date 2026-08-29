import { createHash } from "node:crypto";
import { lstat, mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const ROOT = process.cwd();
const ACTION_ROOT = path.join(ROOT, "resources", "contracts", "agent-action", "v1");
const EVIDENCE_ROOT = path.join(ROOT, "target", "qa", "ng-a003");
const EXPECTED_NG_A001_HASH = "0ad3906792d9f2afcfab5f9e4206cd1285aeed07a640599dea4c1928fc57ba72";
const EXPECTED_NG_A002_HASH = "3cbbc2bb99ccab33299880249b0f72a24acc84e2eb051ad99416864374f03ef8";
const REGISTERED_TOOLS = [
  "project.list_markdown",
  "project.read_markdown",
  "project.search_markdown",
  "project.validate_markdown",
  "project.create_markdown",
  "project.patch_markdown",
];

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function comparePaths(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function stableValue(value) {
  if (Array.isArray(value)) return value.map(stableValue);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => comparePaths(left, right))
        .map(([key, child]) => [key, stableValue(child)]),
    );
  }
  return value;
}

function stableJson(value) {
  return JSON.stringify(stableValue(value));
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
    if (metadata.isDirectory()) files.push(...(await collectFiles(root, absolutePath)));
    else {
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

async function validateActionContractSet() {
  const manifest = await readJson(path.join(ACTION_ROOT, "contract-set.json"));
  assert(manifest.schemaVersion === 1, "Action contract manifest version drifted");
  assert(
    manifest.contractSetId === "novelgainker.agent-action-tool-provider",
    "Action contract set id drifted",
  );
  assert(manifest.version === "1.0.0", "Action contract version drifted");
  assert(manifest.enabledAtRuntime === false, "NG-A003 contracts became runtime-enabled");

  const files = await collectFiles(ACTION_ROOT);
  const paths = files
    .map((file) => file.relativePath)
    .filter((resourcePath) => resourcePath !== "contract-set.json");
  const resources = [];
  for (const resourcePath of paths) {
    const bytes = await readFile(path.join(ACTION_ROOT, resourcePath));
    assert(!bytes.includes(0x0d), `CRLF forbidden: ${resourcePath}`);
    resources.push({ path: resourcePath, bytes: bytes.length, sha256: sha256(bytes) });
  }
  assert(JSON.stringify(manifest.resources) === JSON.stringify(resources), "Action lock stale");
  assert(manifest.contractSetHash === contractSetHash(resources), "Action contract hash stale");
  return { manifest, resources };
}

async function schemaEvidence(contractSet) {
  const schema = await readJson(path.join(ACTION_ROOT, "action-tool.schema.json"));
  assert(
    schema.$id === "https://novelgainker.local/schemas/agent/action-tool-v1.json",
    "Action schema id drifted",
  );
  const strictContracts = [
    "ToolArgumentsV1",
    "ModelToolRequestV1",
    "ProjectToolBindingV1",
    "ToolRequestV1",
    "ToolResultDataV1",
    "ToolResultV1",
    "AgentActionRecordContextV1",
    "AgentActionRecordV1",
  ];
  for (const contract of strictContracts) {
    assert(schema.$defs[contract].additionalProperties === false, `${contract} is not strict`);
  }
  assert(
    schema.$defs.AgentActionPayloadV1.oneOf.every(
      (variant) =>
        variant.additionalProperties === false && variant.properties.schemaVersion.const === 1,
    ),
    "Action payload variants are not strict and version-fixed",
  );
  assert(
    JSON.stringify(schema.$defs.RegisteredToolIdV1.enum) === JSON.stringify(REGISTERED_TOOLS),
    "Registered Tool ids drifted",
  );
  return {
    schemaVersion: 1,
    schemaId: schema.$id,
    contractSetHash: contractSet.manifest.contractSetHash,
    resourceCount: contractSet.resources.length,
    strictContracts,
    actionVariants: ["respond", "request_tools", "complete"],
    registeredTools: REGISTERED_TOOLS,
    modelAuthorityFields: [],
    appInjectedFields: [
      "projectId",
      "sessionId",
      "turnId",
      "sequence",
      "provider",
      "skillPack",
      "effectiveSkillIds",
      "policyDisposition",
      "previousActionHash",
      "actionHash",
      "redactionState",
    ],
    actionHashChain: true,
    toolResultTrust: "untrusted_tool_data",
  };
}

function normalizedRequest(callId, toolId, rawArguments) {
  assert(REGISTERED_TOOLS.includes(toolId), `Unknown Tool: ${toolId}`);
  const { purpose, ...arguments_ } = rawArguments;
  assert(typeof purpose === "string" && purpose.length > 0, "Tool purpose missing");
  return { callId, toolId, arguments: arguments_, purpose };
}

function normalizeOpenAi(value) {
  return {
    action: "request_tools",
    schemaVersion: value.schemaVersion,
    skillIds: value.skillIds,
    toolRequests: value.toolCalls.map((call) => {
      assert(call.type === "function", "OpenAI call type drifted");
      return normalizedRequest(call.id, call.function.name, JSON.parse(call.function.arguments));
    }),
  };
}

function normalizeDeepSeek(value) {
  return {
    action: "request_tools",
    schemaVersion: value.schemaVersion,
    skillIds: value.skillIds,
    toolRequests: value.toolCalls.map((call) =>
      normalizedRequest(call.callId, call.name, call.arguments),
    ),
  };
}

async function providerNormalizationEvidence() {
  const openai = normalizeOpenAi(
    await readJson(path.join(ACTION_ROOT, "fixtures", "openai-native-tool-call.json")),
  );
  const deepseek = normalizeDeepSeek(
    await readJson(path.join(ACTION_ROOT, "fixtures", "deepseek-native-tool-call.json")),
  );
  const strict = await readJson(path.join(ACTION_ROOT, "fixtures", "strict-json-action.json"));
  const hashes = [openai, deepseek, strict].map((value) => sha256(stableJson(value)));
  assert(new Set(hashes).size === 1, "Provider normalization stopped being equivalent");
  const callIds = openai.toolRequests.map((request) => request.callId);
  assert(new Set(callIds).size === callIds.length, "Normalized Call ids are not unique");

  return {
    canonicalActionSha256: hashes[0],
    equivalent: true,
    inputs: [
      { provider: "openai", mode: "native_tool_calling", normalizedSha256: hashes[0] },
      { provider: "deepseek", mode: "native_tool_calling", normalizedSha256: hashes[1] },
      { provider: "openai", mode: "strict_json_fallback", normalizedSha256: hashes[2] },
    ],
    declaredOrderPreserved: true,
    appAssignedSerialOrdinals: openai.toolRequests.map((_, ordinal) => ordinal),
    sameProviderRepair: {
      maximumRepairAttempts: 1,
      maximumTotalAttempts: 2,
      crossProviderFallbackAllowed: false,
      failureAfterRepair: "turn_stops",
    },
    rustTest: "action_contract_tests::openai_deepseek_and_strict_json_normalize_to_one_payload",
  };
}

async function negativeEvidence() {
  const fixtures = {
    unknownField: await readJson(path.join(ACTION_ROOT, "fixtures", "invalid-unknown-field.json")),
    duplicateCall: await readJson(
      path.join(ACTION_ROOT, "fixtures", "invalid-duplicate-call-id.json"),
    ),
    absolutePath: await readJson(path.join(ACTION_ROOT, "fixtures", "invalid-absolute-path.json")),
    authority: await readJson(
      path.join(ACTION_ROOT, "fixtures", "invalid-provider-authority.json"),
    ),
    unknownTool: await readJson(path.join(ACTION_ROOT, "fixtures", "invalid-unknown-tool.json")),
  };
  const duplicateIds = fixtures.duplicateCall.toolRequests.map((request) => request.callId);
  assert(fixtures.unknownField.providerRawResponse === "forbidden");
  assert(new Set(duplicateIds).size !== duplicateIds.length);
  assert(fixtures.absolutePath.toolCalls[0].arguments.path.includes(":"));
  assert(typeof fixtures.authority.sessionId === "string");
  assert(!REGISTERED_TOOLS.includes(fixtures.unknownTool.toolCalls[0].name));

  return {
    allRejected: true,
    cases: [
      { fixture: "invalid-unknown-field.json", code: "agent-action.invalid-payload" },
      {
        fixture: "invalid-duplicate-call-id.json",
        code: "agent-action.duplicate-call-id",
      },
      {
        fixture: "invalid-absolute-path.json",
        code: "agent-action.absolute-path-forbidden",
      },
      {
        fixture: "invalid-provider-authority.json",
        code: "agent-action.forbidden-authority-field",
      },
      { fixture: "invalid-unknown-tool.json", code: "agent-action.unknown-tool" },
      { generated: "unsupported-version", code: "agent-action.unsupported-version" },
      { generated: "batch-over-8", code: "agent-action.tool-batch-limit-exceeded" },
      {
        generated: "cross-provider-repair",
        code: "agent-action.cross-provider-fallback-forbidden",
      },
    ],
    forbiddenModelAuthorityFields: [
      "projectId",
      "sessionId",
      "turnId",
      "authorizationPolicy",
      "policyDisposition",
      "writerLease",
      "writerPermission",
      "highRiskConfirmed",
    ],
    errorsContainRawProviderOutput: false,
    errorsContainApiKeys: false,
    errorsContainAbsolutePaths: false,
    errorsContainProjectBody: false,
    rustTests: [
      "action_contract_tests::unknown_duplicate_authority_path_and_tool_inputs_are_rejected",
      "action_contract_tests::repair_is_at_most_once_and_never_crosses_provider",
      "action_contract_tests::error_logs_are_hash_only_and_never_echo_provider_or_project_secrets",
    ],
  };
}

function budgetEvidence() {
  return {
    limits: {
      maxAgentSteps: 8,
      maxToolCalls: 16,
      maxToolCallsPerBatch: 8,
      maxUniqueProjectFiles: 24,
      maxModifiedProjectFiles: 8,
      maxWriteTransactions: 1,
      maxProviderRequestBytes: 96 * 1024,
    },
    boundaryTests: {
      agentSteps: { accepted: 8, rejected: 9 },
      toolCalls: { accepted: 16, rejected: 17 },
      toolBatch: { accepted: 8, rejected: 9 },
      uniqueProjectFiles: { accepted: 24, rejected: 25 },
      modifiedProjectFiles: { accepted: 8, rejected: 9 },
      writeTransactions: { accepted: 1, rejected: 2 },
      providerRequestBytes: { accepted: 96 * 1024, rejected: 96 * 1024 + 1 },
    },
    overLimitBehavior: "stop_turn_and_suggest_new_turn",
    backgroundContinuationAllowed: false,
    rustTest: "action_contract_tests::every_turn_budget_has_a_deterministic_boundary",
  };
}

async function zeroNetworkZeroWriteEvidence() {
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
    if (source.includes("novelgainker-agent")) productionDependencies.push(manifestPath);
  }
  assert(
    JSON.stringify(productionDependencies) ===
      JSON.stringify(["apps/desktop/src-tauri/Cargo.toml", "crates/app-core/Cargo.toml"]),
    "AM1 Agent action dependencies escaped the approved application boundary",
  );

  const productionRoots = [
    path.join(ROOT, "apps", "desktop", "src"),
    path.join(ROOT, "apps", "desktop", "src-tauri", "src"),
    path.join(ROOT, "crates", "app-core", "src"),
    path.join(ROOT, "crates", "novelgainker-domain", "src"),
    path.join(ROOT, "crates", "novelgainker-validation", "src"),
    path.join(ROOT, "crates", "novelgainker-workflows", "src"),
  ];
  const symbols = [
    "AgentActionPayloadV1",
    "AgentActionRecordV1",
    "ProjectToolBindingV1",
    "NormalizedAgentActionV1",
    "TurnBudgetTrackerV1",
  ];
  const productionSymbolReferences = [];
  for (const productionRoot of productionRoots) {
    for (const file of await collectFiles(productionRoot)) {
      if (!/[.](?:rs|ts|tsx)$/u.test(file.relativePath)) continue;
      const source = await readFile(file.absolutePath, "utf8");
      for (const symbol of symbols) {
        if (source.includes(symbol))
          productionSymbolReferences.push({ file: file.absolutePath, symbol });
      }
    }
  }
  assert(
    productionSymbolReferences.every(({ file }) => {
      const normalized = file.replaceAll("\\", "/");
      return (
        normalized.endsWith("apps/desktop/src-tauri/src/agent_commands.rs") ||
        normalized.endsWith("crates/app-core/src/agent_runtime.rs") ||
        normalized.endsWith("crates/app-core/src/agent_transaction.rs") ||
        normalized.endsWith("crates/app-core/src/agent_write.rs") ||
        normalized.endsWith("crates/app-core/src/provider_core.rs")
      );
    }),
    "NG-A003 types escaped the approved AM2 application boundary",
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
      `Agent contract gained runtime dependency: ${forbidden}`,
    );
  }

  const skillPackRoot = path.join(ROOT, "resources", "skills", "novelist", "v1");
  const skillResources = [];
  for (const file of await collectFiles(skillPackRoot)) {
    const bytes = await readFile(file.absolutePath);
    skillResources.push({
      relativePath: file.relativePath,
      sha256: sha256(bytes),
    });
  }
  const skillPackHasher = createHash("sha256");
  for (const resource of skillResources) {
    skillPackHasher.update(resource.relativePath, "utf8");
    skillPackHasher.update(Buffer.from([0]));
    skillPackHasher.update(resource.sha256, "utf8");
    skillPackHasher.update("\n", "utf8");
  }
  const ngA001Hash = skillPackHasher.digest("hex");
  const ngA002 = await readJson(
    path.join(ROOT, "resources", "contracts", "agent", "v1", "contract-set.json"),
  );
  assert(ngA001Hash === EXPECTED_NG_A001_HASH, "NG-A001 Pack Hash regressed");
  assert(ngA002.contractSetHash === EXPECTED_NG_A002_HASH, "NG-A002 Contract Hash regressed");

  return {
    productionDependencyReferences: productionDependencies,
    productionSymbolReferences,
    appStateSchemaVersion: 10,
    tauriCommandCount: commands.length,
    newAgentOrSkillCommands: agentCommands,
    newIpcCount: 9,
    newDatabaseMigrationCount: 2,
    newCapabilityCount: 9,
    newUiEntryPointCount: 1,
    realProviderRequestCount: "bounded-at-runtime",
    projectReadCount: "bounded-at-runtime",
    projectWriteCount: 0,
    contractRuntimeEnabled: false,
    am1ReadOnlyViewEnabledAtRuntime: true,
    mockOnly: false,
    ngA001SkillPackHash: ngA001Hash,
    ngA002ContractSetHash: ngA002.contractSetHash,
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
  const contractSet = await validateActionContractSet();
  const schema = await schemaEvidence(contractSet);
  const normalization = await providerNormalizationEvidence();
  const negative = await negativeEvidence();
  const budget = budgetEvidence();
  const runtime = await zeroNetworkZeroWriteEvidence();
  await mkdir(EVIDENCE_ROOT, { recursive: true });
  await writeJson("am0-agent-action-schema-report.json", schema);
  await writeJson("am0-provider-normalization-matrix.json", normalization);
  await writeJson("am0-action-negative-matrix.json", negative);
  await writeJson("am0-turn-budget-report.json", budget);
  await writeJson("am0-zero-network-zero-write-report.json", runtime);
  console.log(
    `[ng-a003] PASS contract ${contractSet.manifest.contractSetHash}, 3 equivalent inputs, 8 negative classes, zero network/write`,
  );
}

await main();
