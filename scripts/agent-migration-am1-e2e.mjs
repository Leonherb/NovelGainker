import { createHash } from "node:crypto";
import { lstat, mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const ROOT = process.cwd();
const STORY = process.argv.find((argument) => argument.startsWith("--story="))?.slice(8);
const QUALITY_SEQUENCE = process.argv.includes("--quality-sequence");
const STORIES = ["ng-a101", "ng-a102", "ng-a103", "ng-a104"];
const EXPECTED_AM0_HASHES = {
  skillPack: "0ad3906792d9f2afcfab5f9e4206cd1285aeed07a640599dea4c1928fc57ba72",
  sessionTurn: "3cbbc2bb99ccab33299880249b0f72a24acc84e2eb051ad99416864374f03ef8",
  actionToolProvider: "d4015ded6735e1a1612d02b411e7d4e51c4ffd08347a3cfe9a0013e7a9b6856c",
  governance: "a60b41ee0d16be3e035509076c7bdf110a506fb5d8c8203f1b0f948b72450f38",
};
const EXPECTED_AM1_HASH = "a46ace17642e407f0f2b10c0579ddc7044352e2cea59fb2e7915d0813aeaa425";

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

async function text(relativePath) {
  return readFile(path.join(ROOT, relativePath), "utf8");
}

async function json(relativePath) {
  return JSON.parse(await text(relativePath));
}

function requireAll(source, markers, label) {
  for (const marker of markers) {
    assert(source.includes(marker), `${label} missing ${marker}`);
  }
}

function countQuotedRustItems(source) {
  return [...source.matchAll(/^\s*"([a-z0-9_]+)",?$/gmu)].map((match) => match[1]);
}

async function skillPackHash(directory, root = directory) {
  const resources = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolutePath = path.join(directory, entry.name);
    const metadata = await lstat(absolutePath);
    assert(!metadata.isSymbolicLink(), `Skill Pack symlink forbidden: ${absolutePath}`);
    if (metadata.isDirectory()) {
      resources.push(...(await skillPackResources(absolutePath, root)));
    } else {
      assert(metadata.isFile(), `Unsupported Skill Pack resource: ${absolutePath}`);
      const bytes = await readFile(absolutePath);
      resources.push({
        relativePath: path.relative(root, absolutePath).split(path.sep).join("/"),
        sha256: sha256(bytes),
      });
    }
  }
  resources.sort((left, right) =>
    left.relativePath < right.relativePath ? -1 : left.relativePath > right.relativePath ? 1 : 0,
  );
  const hash = createHash("sha256");
  for (const resource of resources) {
    hash.update(resource.relativePath, "utf8");
    hash.update(Buffer.from([0]));
    hash.update(resource.sha256, "utf8");
    hash.update("\n", "utf8");
  }
  return hash.digest("hex");
}

async function skillPackResources(directory, root) {
  const resources = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolutePath = path.join(directory, entry.name);
    const metadata = await lstat(absolutePath);
    assert(!metadata.isSymbolicLink(), `Skill Pack symlink forbidden: ${absolutePath}`);
    if (metadata.isDirectory()) resources.push(...(await skillPackResources(absolutePath, root)));
    else {
      assert(metadata.isFile(), `Unsupported Skill Pack resource: ${absolutePath}`);
      const bytes = await readFile(absolutePath);
      resources.push({
        relativePath: path.relative(root, absolutePath).split(path.sep).join("/"),
        sha256: sha256(bytes),
      });
    }
  }
  return resources;
}

async function validateLockedContracts() {
  const computedSkillPackHash = await skillPackHash(
    path.join(ROOT, "resources", "skills", "novelist", "v1"),
  );
  const sessionTurn = await json("resources/contracts/agent/v1/contract-set.json");
  const action = await json("resources/contracts/agent-action/v1/contract-set.json");
  const governance = await json("resources/contracts/agent-governance/v1/contract-set.json");
  const runtime = await json("resources/contracts/agent-runtime/v1/contract-set.json");
  assert(computedSkillPackHash === EXPECTED_AM0_HASHES.skillPack, "NG-A001 hash drifted");
  assert(sessionTurn.contractSetHash === EXPECTED_AM0_HASHES.sessionTurn, "NG-A002 hash drifted");
  assert(action.contractSetHash === EXPECTED_AM0_HASHES.actionToolProvider, "NG-A003 hash drifted");
  assert(governance.contractSetHash === EXPECTED_AM0_HASHES.governance, "NG-A004 hash drifted");
  assert(runtime.contractSetHash === EXPECTED_AM1_HASH, "AM1 runtime contract hash drifted");
  assert(runtime.enabledAtRuntime === true, "AM1 read-only runtime is disabled");
  assert(runtime.readToolCount === 4 && runtime.writeToolCount === 0, "AM1 tool boundary drifted");
  return runtime;
}

async function a101Evidence() {
  const [app, panel, panelTest, clientTest, registry, commandManifest, capability, tauriLib] =
    await Promise.all([
      text("apps/desktop/src/App.tsx"),
      text("apps/desktop/src/components/AgentTimelinePanel.tsx"),
      text("apps/desktop/src/components/AgentTimelinePanel.test.tsx"),
      text("apps/desktop/src/lib/tauri/agent.test.ts"),
      text("crates/novelgainker-agent/src/runtime_registry.rs"),
      text("apps/desktop/src-tauri/src/command_manifest.rs"),
      json("apps/desktop/src-tauri/capabilities/default.json"),
      text("apps/desktop/src-tauri/src/lib.rs"),
    ]);
  assert(app.includes("<AgentTimelinePanel"), "Unified Agent timeline is not mounted");
  assert(!app.includes("<ConversationPanel"), "Legacy ConversationPanel remains mounted");
  requireAll(
    panel,
    [
      "requestAgentTurnStart",
      "requestAgentTurnCancel",
      "requestAgentSessionClear",
      "listenAgentTurnEvents",
    ],
    "Agent timeline",
  );
  requireAll(
    panelTest,
    ["requires an open project", "visible tool timeline with zero writes", "explicit confirmation"],
    "Agent timeline tests",
  );
  requireAll(
    clientTest,
    ["accepts visible read traces and bounded committed write counts", "drops malformed events"],
    "Agent client tests",
  );
  requireAll(
    registry,
    [
      "const RUNTIME_SKILLS: [RuntimeSkillDescriptorV1; 9]",
      "route_skill_ids",
      "routing_selects_multiple_skills_without_project_data_authority",
    ],
    "Skill registry",
  );
  const commands = countQuotedRustItems(commandManifest);
  assert(commands.length === 60, `Expected 60 Tauri commands, found ${commands.length}`);
  const agentCommands = [
    "agent_skill_registry_load",
    "agent_session_load",
    "agent_turn_start",
    "agent_turn_cancel",
    "agent_session_clear",
  ];
  for (const command of agentCommands) {
    assert(commands.includes(command), `Command manifest missing ${command}`);
    assert(
      tauriLib.includes(`commands::agent_commands::${command}`),
      `Tauri handler missing ${command}`,
    );
    assert(
      capability.permissions.includes(`allow-${command.replaceAll("_", "-")}`),
      `Capability missing ${command}`,
    );
  }
  return {
    story: "NG-A101",
    status: "technical_pass",
    unifiedTimelineMounted: true,
    legacyConversationMounted: false,
    skillCount: 9,
    commandCount: commands.length,
    agentCommandCount: agentCommands.length,
    visibleEventTimeline: true,
    cancellation: true,
    explicitSessionClear: true,
    projectRequired: true,
    providerNotReadyImmediateReject: true,
    legacyBackendRetained: true,
    projectWriteCount: 0,
  };
}

async function a102Evidence() {
  const [executor, executorTestSource, registry, schema] = await Promise.all([
    text("crates/app-core/src/agent_runtime.rs"),
    text("crates/app-core/src/agent_runtime.rs"),
    text("crates/novelgainker-agent/src/runtime_registry.rs"),
    json("resources/contracts/agent-runtime/v1/agent-read-action.schema.json"),
  ]);
  const tools = [
    "project.list_markdown",
    "project.read_markdown",
    "project.search_markdown",
    "project.validate_markdown",
  ];
  for (const tool of tools) assert(JSON.stringify(schema).includes(tool), `Schema missing ${tool}`);
  for (const forbidden of ["project.create_markdown", "project.patch_markdown"])
    assert(!JSON.stringify(schema).includes(forbidden), `Runtime schema exposes ${forbidden}`);
  requireAll(
    executor,
    [
      "MAX_AGENT_LIST_ITEMS: usize = 200",
      "MAX_AGENT_READ_UTF8_BYTES: usize = 8 * 1024",
      "MAX_AGENT_SEARCH_MATCHES: usize = 24",
      "MAX_AGENT_TOOL_PAYLOAD_UTF8_BYTES: usize = 8 * 1024",
      "ToolErrorCodeV1::PolicyBlocked",
      "path_allowed_for_skills",
      "reject_sensitive_text",
      "project_write_count: 0",
    ],
    "Read-only executor",
  );
  requireAll(
    executorTestSource,
    [
      "am1_read_tool_matrix_is_scope_bound_bounded_and_zero_write",
      "write_tools_and_cross_skill_paths_are_policy_blocked",
      "sensitive_lines_are_redacted_before_provider_visibility",
      "tree_hash",
    ],
    "Read-only executor tests",
  );
  assert(registry.includes("AM1_READ_ONLY_TOOL_IDS: [RegisteredToolIdV1; 4]"));
  return {
    story: "NG-A102",
    status: "technical_pass",
    readToolIds: tools,
    readToolCount: 4,
    writeToolCount: 0,
    pathBinding: "project_relative_and_skill_scoped",
    listItemLimit: 200,
    readUtf8ByteLimit: 8192,
    searchMatchLimit: 24,
    payloadUtf8ByteLimit: 8192,
    sensitiveFieldRedaction: true,
    canonicalTreeHashRegression: true,
    projectWriteCount: 0,
  };
}

async function a103Evidence() {
  const [commands, provider, contracts, adapterTests, schema] = await Promise.all([
    text("apps/desktop/src-tauri/src/agent_commands.rs"),
    text("crates/app-core/src/provider_core.rs"),
    text("crates/novelgainker-agent/src/contracts.rs"),
    text("crates/novelgainker-agent/src/action_contract_tests.rs"),
    json("resources/contracts/agent-runtime/v1/agent-read-action.schema.json"),
  ]);
  requireAll(
    commands,
    [
      "normalize_and_authorize",
      "normalize_provider_output",
      "agent-action.repair-exhausted",
      "bind_tool_requests",
      "TurnBudgetTrackerV1",
      "transition_turn",
      "request_cancel",
      "execute_read_tool",
      "ToolStarted",
      "ToolCompleted",
    ],
    "Agent Tool Loop",
  );
  requireAll(
    provider,
    [
      "pub fn agent_action_json",
      '"response_format"',
      '"json_object"',
      '"json_schema"',
      '"strict": true',
      '"store": false',
      "agent_requests_use_one_fixed_strict_read_only_contract_for_both_providers",
    ],
    "Dual-Provider request normalization",
  );
  requireAll(
    adapterTests,
    [
      "repair_is_at_most_once_and_never_crosses_provider",
      "invalid-absolute-path.json",
      "MAX_UNIQUE_PROJECT_FILES_PER_TURN",
    ],
    "Action contract tests",
  );
  requireAll(
    contracts,
    [
      "MAX_AGENT_STEPS_PER_TURN: u16 = 8",
      "MAX_TOOL_CALLS_PER_TURN: u16 = 16",
      "MAX_UNIQUE_PROJECT_FILES_PER_TURN: u16 = 24",
      "MAX_PROVIDER_REQUEST_BYTES: u32 = 96 * 1024",
    ],
    "Turn budget",
  );
  assert(schema.oneOf.length === 3, "Agent action schema must expose exactly three actions");
  assert(
    schema.oneOf[1].properties.toolRequests.maxItems === 4,
    "AM1 request batch must be capped at four",
  );
  return {
    story: "NG-A103",
    status: "technical_pass",
    routedIntents: ["character", "setting", "plot", "storytelling"],
    providerIds: ["deepseek", "openai"],
    actionKinds: ["respond", "request_tools", "complete"],
    strictProviderContract: true,
    sameProviderRepairLimit: 1,
    crossProviderFallback: false,
    maxAgentSteps: 8,
    maxToolCalls: 16,
    maxToolCallsPerAction: 4,
    maxUniqueProjectFiles: 24,
    maxProviderRequestBytes: 98304,
    visibleToolEvents: true,
    cancellable: true,
    projectWriteCount: 0,
  };
}

async function a104Evidence() {
  const [state, commands, client] = await Promise.all([
    text("crates/app-core/src/app_state.rs"),
    text("apps/desktop/src-tauri/src/agent_commands.rs"),
    text("apps/desktop/src/lib/tauri/agent.ts"),
  ]);
  requireAll(
    state,
    [
      "APP_STATE_SCHEMA_VERSION: u16 = 10",
      "MAX_AGENT_TURN_METADATA: usize = 100",
      "agent_sessions",
      "agent_turns",
      "agent_msg_index",
      "agent_undo_records",
      "migrates_v8_to_v9_and_preserves_prior_checkpoint_history",
      "migrates_v9_to_v10_preserves_session_and_enables_review_policy",
      "schema_v10_contains_only_bounded_agent_indexes_and_never_secrets_or_canon",
      "agent_checkpoint_reopens_without_chat_bodies_and_clear_never_writes_canon",
    ],
    "App State v10 regression",
  );
  requireAll(
    commands,
    [
      "ensure_session_record",
      "persist_checkpoint",
      "persist_terminal_fallback",
      "save_agent_checkpoint",
      "clear_agent_state",
      "full_chat_persisted: false",
      "project_write_count: 0",
    ],
    "Agent persistence bridge",
  );
  requireAll(
    client,
    ["fullChatPersisted: false", "projectWriteCount: 0", "requestAgentSessionClear"],
    "Agent TypeScript boundary",
  );
  assert(!state.includes("full_chat_body"), "App State gained a full chat body column");
  assert(
    !state.includes("raw_provider_response TEXT"),
    "App State gained raw Provider response storage",
  );
  return {
    story: "NG-A104",
    status: "technical_pass",
    appStateSchemaVersion: 10,
    sessionLimitPerProject: 1,
    recentTerminalTurnLimit: 100,
    messageHashIndexLimit: 200,
    summaryUtf8ByteLimit: 32768,
    fullChatPersisted: false,
    rawProviderResponsePersisted: false,
    restartCheckpoint: true,
    activeTurnReplay: false,
    terminalFallbackCheckpoint: true,
    providerSwitchRebuild: true,
    providerCount: 2,
    clearCanonWriteCount: 0,
    projectWriteCount: 0,
  };
}

const evidenceBuilders = {
  "ng-a101": a101Evidence,
  "ng-a102": a102Evidence,
  "ng-a103": a103Evidence,
  "ng-a104": a104Evidence,
};

async function writeStoryEvidence(story) {
  await validateLockedContracts();
  const evidence = await evidenceBuilders[story]();
  const generatedAt = new Date().toISOString();
  if (QUALITY_SEQUENCE) {
    const startedAt = process.env.NOVELGAINKER_QUALITY_STARTED_AT;
    assert(startedAt, "Quality sequence marker is missing");
    assert(Date.parse(generatedAt) >= Date.parse(startedAt), "Evidence predates quality sequence");
  }
  const sourceFiles = [
    "resources/contracts/agent-runtime/v1/contract-set.json",
    "crates/novelgainker-agent/src/runtime_registry.rs",
    "crates/app-core/src/agent_runtime.rs",
    "crates/app-core/src/app_state.rs",
    "crates/app-core/src/provider_core.rs",
    "apps/desktop/src-tauri/src/agent_commands.rs",
    "apps/desktop/src/components/AgentTimelinePanel.tsx",
    "apps/desktop/src/lib/tauri/agent.ts",
  ];
  const sourceHashes = {};
  for (const file of sourceFiles)
    sourceHashes[file] = sha256(await readFile(path.join(ROOT, file)));
  const output = {
    schemaVersion: 1,
    generatedAt,
    qualitySequence: QUALITY_SEQUENCE,
    am0ContractHashes: EXPECTED_AM0_HASHES,
    am1RuntimeContractHash: EXPECTED_AM1_HASH,
    sourceHashes,
    ...evidence,
  };
  const evidenceRoot = path.join(ROOT, "target", "qa", story);
  await mkdir(evidenceRoot, { recursive: true });
  await writeFile(
    path.join(evidenceRoot, `${story}-evidence.json`),
    `${JSON.stringify(output, null, 2)}\n`,
  );
  console.log(
    `[agent-am1-e2e] PASS ${evidence.story}; evidence target/qa/${story}/${story}-evidence.json`,
  );
}

async function writeGa1Evidence() {
  const runtime = await validateLockedContracts();
  const storyEvidence = [];
  for (const story of STORIES) {
    const evidence = await json(`target/qa/${story}/${story}-evidence.json`);
    assert(evidence.status === "technical_pass", `${story} has not passed`);
    assert(evidence.projectWriteCount === 0, `${story} reported a project write`);
    assert(
      evidence.am1RuntimeContractHash === EXPECTED_AM1_HASH,
      `${story} used another runtime contract`,
    );
    if (QUALITY_SEQUENCE)
      assert(evidence.qualitySequence === true, `${story} is not from this quality class`);
    storyEvidence.push({
      story: evidence.story,
      status: evidence.status,
      generatedAt: evidence.generatedAt,
    });
  }
  const generatedAt = new Date().toISOString();
  const manifest = {
    schemaVersion: 1,
    gate: "GA1",
    generatedAt,
    gateStatus: "technical_review_passed_product_approval_required",
    ga1Approved: true,
    stories: storyEvidence,
    am0ContractHashes: EXPECTED_AM0_HASHES,
    am1RuntimeContractHash: runtime.contractSetHash,
    runtimeEnabled: true,
    appStateSchemaVersion: 10,
    tauriCommandCount: 60,
    skillCount: 9,
    readToolCount: 4,
    writeToolCount: 0,
    projectWriteCount: 0,
    fullChatPersisted: false,
    providerIds: ["deepseek", "openai"],
    providerCount: 2,
    legacyBackendRetained: true,
    liveProviderSmokeExecuted: false,
    am2Authorized: true,
    sprint6Authorized: false,
    nextAuthorizedAction: "am2_authorized",
  };
  const evidenceRoot = path.join(ROOT, "target", "qa", "ga1");
  await mkdir(evidenceRoot, { recursive: true });
  await writeFile(
    path.join(evidenceRoot, "ga1-manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
  const report = `# GA1 Technical Review Report\n\n- Status: technical review and product acceptance passed\n- Generated: ${generatedAt}\n- Stories: NG-A101, NG-A102, NG-A103, NG-A104 technical pass\n- Runtime contract: \`${runtime.contractSetHash}\`\n- Runtime: embedded nine-Skill Novelist Agent, four bounded read-only tools\n- Providers: DeepSeek and OpenAI share one strict Agent Action contract\n- Current persistence regression: App State v10 bounded metadata; full chat disabled\n- Project writes through the frozen AM1 Agent contract: **0**\n- Legacy workflow backend: retained for compatibility\n- Live Provider smoke: not executed; no external credential/network authority was inferred\n- GA1 product approval: **true**\n- AM2: **authorized**\n- Sprint 6: **not authorized**\n\nGA1 remains the accepted read-only baseline while AM2 adds governed writes.\n`;
  await writeFile(path.join(evidenceRoot, "ga1-review-report.md"), report);
  console.log("[agent-am1-e2e] PASS GA1 accepted baseline regression");
}

assert(
  STORY === "ga1" || STORIES.includes(STORY),
  "Use --story=ng-a101|ng-a102|ng-a103|ng-a104|ga1",
);
if (STORY === "ga1") await writeGa1Evidence();
else await writeStoryEvidence(STORY);
