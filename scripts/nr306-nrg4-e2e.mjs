import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  discoverReleaseSurface,
  inspectIpcClosure,
  releaseScopeSha256,
} from "./nr-g1-gate-lib.mjs";

const root = process.cwd();
const prepare = process.argv.includes("--prepare");
const read = (relative) => readFileSync(path.join(root, relative), "utf8");
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const parseJson = (relative) => JSON.parse(read(relative));
const numericConstant = (source, name) => {
  const match = source.match(
    new RegExp(`(?:pub )?const ${name}:\\s*(?:u32|u16|usize)\\s*=\\s*([^;]+);`, "u"),
  );
  if (!match) throw new Error(`NR-306 numeric constant is missing: ${name}`);
  const expression = match[1].replaceAll("_", "").replaceAll(/\s/gu, "");
  if (!/^[0-9*+]+$/u.test(expression)) {
    throw new Error(`NR-306 constant is not mechanically evaluable: ${name}`);
  }
  return expression
    .split("+")
    .map((part) => part.split("*").reduce((product, value) => product * Number(value), 1))
    .reduce((sum, value) => sum + value, 0);
};
const stripRustTests = (source) =>
  source.replace(/\n#\[cfg\(test\)\]\n(?:pub\(crate\) )?mod tests \{[\s\S]*$/u, "\n");
const walk = (relative) => {
  const absolute = path.join(root, relative);
  return readdirSync(absolute).flatMap((name) => {
    const child = path.join(relative, name);
    return statSync(path.join(root, child)).isDirectory() ? walk(child) : [child];
  });
};
const run = (command, args, label, environment = process.env) => {
  const startedAt = performance.now();
  const result = spawnSync(command, args, {
    cwd: root,
    env: environment,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`NR-306 failed: ${label}`);
  return {
    label,
    result: "pass",
    observedDurationMs: Math.round(performance.now() - startedAt),
  };
};

const offlineEnvironment = { ...process.env, CARGO_NET_OFFLINE: "true" };
const secretName = /(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu;
for (const name of Object.keys(offlineEnvironment)) {
  if (secretName.test(name)) delete offlineEnvironment[name];
}

const pack = parseJson("resources/skills/novelist/v1/pack.json");
const inventory = parseJson("resources/skills/novelist/v1/inventory.json");
const capabilitySource = read("crates/app-core/src/capability_pack.rs");
const canonSource = read("crates/app-core/src/canon_context.rs");
const runtimeSource = read("crates/app-core/src/conversation_runtime.rs");
const providerSource = read("crates/app-core/src/provider_core.rs");
const toolSource = read("crates/app-core/src/tool_runtime.rs");
const memorySource = read("crates/app-core/src/memory.rs");
const canonProduction = stripRustTests(canonSource);
const runtimeProduction = stripRustTests(runtimeSource);

const exactCapabilityIds = [
  "create-novel-project",
  "create-setting",
  "create-character",
  "create-material",
  "create-plot",
  "novel-story-telling",
  "analytical-review",
  "create-visual-asset",
  "publish-novel",
];
const manifestIds = pack.capabilityManifests.map((relative) =>
  parseJson(`resources/skills/novelist/v1/${relative}`),
);
if (
  manifestIds.length !== exactCapabilityIds.length ||
  new Set(manifestIds.map((item) => item.id)).size !== exactCapabilityIds.length ||
  exactCapabilityIds.some((id) => !manifestIds.some((item) => item.id === id))
) {
  throw new Error("NR-306 exact nine-capability inventory drift");
}
if (
  pack.sourcePackage.id !== "novelist" ||
  pack.sourcePackage.version !== "0.2.0" ||
  pack.projection.id !== "novelgainker.novelist" ||
  pack.projection.schemaVersion !== 1 ||
  inventory.fileCount !== inventory.files.length ||
  inventory.algorithm !== "sha256"
) {
  throw new Error("NR-306 Pack identity or inventory drift");
}

const packCheck = run(process.execPath, ["scripts/nr301-pack-refresh.mjs"], "NR-301 pack drift");
const runtimePackFiles = walk("resources/skills/novelist/v1").filter((relative) =>
  /(?:pack\.json|manifests[/\\].+\.json|instructions[/\\].+\.md)$/u.test(relative),
);
for (const relative of runtimePackFiles) {
  const source = read(relative);
  for (const forbidden of [
    "AgentActionPayloadV1",
    "enabledAtRuntime",
    "featureGate",
    "writeScopes",
  ]) {
    if (source.includes(forbidden)) {
      throw new Error(`NR-306 retired runtime contract leaked into ${relative}: ${forbidden}`);
    }
  }
}

for (const required of [
  "name: NOVELIST_READ_CONTEXT_TOOL_NAME.to_owned()",
  "version: NOVELIST_READ_CONTEXT_TOOL_VERSION",
  "MAX_CAPABILITY_RESOURCES: usize = 8",
  "MAX_CANON_OPERATIONS: usize = 8",
  "MAX_CANON_UNIQUE_FILES: usize = 8",
  "MAX_CANON_CONTENT_BYTES: usize = 10 * 1024",
  "MAX_CANON_FRAGMENT_BYTES: usize = 4 * 1024",
  "check_project_file",
  "ProjectFileWatchState::Unchanged",
]) {
  if (!canonSource.includes(required)) {
    throw new Error(`NR-306 Canon/Tool boundary is missing: ${required}`);
  }
}
for (const required of [
  "bind_novelist_runtime",
  "ToolRegistry::novelist_production",
  "NR3_AGENT_LOOP_RESERVE_BYTES",
  "NOVELIST_AGENT_FALLBACK_ADDENDUM",
]) {
  if (!runtimeSource.includes(required)) {
    throw new Error(`NR-306 runtime binding is missing: ${required}`);
  }
}
for (const forbidden of [
  "selected_capability",
  "selectedCapability",
  "route_capability",
  "match_user_keyword",
  "regex::Regex",
  "KEYWORD_TABLE",
]) {
  if (runtimeProduction.includes(forbidden) || canonProduction.includes(forbidden)) {
    throw new Error(`NR-306 Host keyword router leaked into production: ${forbidden}`);
  }
}
for (const forbidden of [
  "write_project_file",
  "save_project_file",
  "atomic_replace",
  "OpenOptions",
  "fs::write",
  "remove_file",
  "create_dir",
]) {
  if (canonProduction.includes(forbidden)) {
    throw new Error(`NR-306 Canon write path is reachable: ${forbidden}`);
  }
}
for (const forbidden of [
  "plugins/cache",
  "plugins\\\\cache",
  "SKILL.md",
  "std::process",
  "Command::",
  "reqwest",
  "powershell",
  "python",
]) {
  if (capabilitySource.includes(forbidden) || canonProduction.includes(forbidden)) {
    throw new Error(`NR-306 external runtime dependency leaked: ${forbidden}`);
  }
}

const requiredTests = [
  [capabilitySource, "embedded_pack_is_exact_self_contained_and_read_only"],
  [capabilitySource, "tampering_and_duplicate_manifest_fail_closed"],
  [canonSource, "production_registry_contains_exactly_the_single_novelist_tool"],
  [canonSource, "capability_resource_zero_and_eight_succeed_while_duplicate_and_unknown_fail"],
  [canonSource, "semantic_limits_and_forbidden_paths_fail_before_executor_io"],
  [canonSource, "capability_and_canon_are_read_together_with_bounded_read_only_output"],
  [canonSource, "no_match_missing_and_invalid_utf8_sources_return_legal_partial_results"],
  [canonSource, "two_provider_selected_capabilities_are_executed_without_host_substitution"],
  [providerSource, "both_adapters_capture_native_tool_requests_and_typed_continuations"],
  [providerSource, "both_adapters_capture_the_fixed_novelist_tool_and_catalog_within_body_limit"],
  [providerSource, "large_history_memory_user_and_tool_result_fit_both_continuation_body_limits"],
  [runtimeSource, "production_runtime_exposes_catalog_and_one_tool_without_keyword_routing"],
  [runtimeSource, "production_novelist_tool_reads_canon_then_completes_the_same_conversation"],
  [runtimeSource, "production_tool_semantic_failure_is_a_task_failure_not_a_chat_failure"],
  [
    runtimeSource,
    "invalid_pack_atomically_removes_catalog_resources_and_registry_but_keeps_identity",
  ],
  [runtimeSource, "nr3_agent_loop_reserve_is_frozen_at_forty_kibibytes"],
  [memorySource, "ten_thousand_visible_messages_keep_provider_input_bounded"],
  [memorySource, "main_request_keeps_current_user_exactly_once"],
];
for (const [source, testName] of requiredTests) {
  if (!source.includes(testName)) throw new Error(`NR-306 acceptance test missing: ${testName}`);
}

const testRuns = [packCheck];
for (const filter of [
  "capability_pack::tests",
  "canon_context::tests",
  "provider_core::tests",
  "conversation_runtime::tests",
  "memory::tests",
]) {
  testRuns.push(
    run(
      "cargo",
      ["test", "-p", "app-core", "--locked", "--offline", filter],
      `Rust ${filter}`,
      offlineEnvironment,
    ),
  );
}

const ipc = inspectIpcClosure(root);
if (ipc.manifest.length !== 24 || ipc.conversation.length !== 3) {
  throw new Error(
    `NR-306 IPC surface drift: ${ipc.manifest.length} total / ${ipc.conversation.length} conversation`,
  );
}
const releaseFiles = discoverReleaseSurface(root);
const uiSources = releaseFiles
  .filter((absolute) => /apps[/\\]desktop[/\\]src[/\\].+\.(?:ts|tsx|css)$/u.test(absolute))
  .map((absolute) => readFileSync(absolute, "utf8"))
  .join("\n");
for (const forbidden of [
  "SkillSelector",
  "CapabilitySelector",
  "selectedCapability",
  "novelist.read_context",
  "AgentActionPayloadV1",
]) {
  if (uiSources.includes(forbidden)) {
    throw new Error(`NR-306 user-visible Skill/control surface leaked: ${forbidden}`);
  }
}

const profile = {
  capabilityInstructionBytes: numericConstant(capabilitySource, "MAX_CAPABILITY_INSTRUCTION_BYTES"),
  capabilityCatalogBytes: numericConstant(capabilitySource, "MAX_CAPABILITY_CATALOG_BYTES"),
  capabilityResourceContentBytes: numericConstant(
    canonSource,
    "MAX_CAPABILITY_RESOURCE_CONTENT_BYTES",
  ),
  capabilityResourceItems: numericConstant(canonSource, "MAX_CAPABILITY_RESOURCES"),
  canonContentBytes: numericConstant(canonSource, "MAX_CANON_CONTENT_BYTES"),
  canonFragmentBytes: numericConstant(canonSource, "MAX_CANON_FRAGMENT_BYTES"),
  canonOperations: numericConstant(canonSource, "MAX_CANON_OPERATIONS"),
  canonUniqueFiles: numericConstant(canonSource, "MAX_CANON_UNIQUE_FILES"),
  canonListPaths: numericConstant(canonSource, "MAX_CANON_LIST_PATHS"),
  canonSearchMatches: numericConstant(canonSource, "MAX_CANON_SEARCH_MATCHES"),
  contextBundleBytes: numericConstant(toolSource, "MAX_TOOL_RESULT_BYTES"),
  toolArgumentsBytes: numericConstant(toolSource, "MAX_TOOL_ARGUMENTS_BYTES"),
  agentLoopReserveBytes: numericConstant(runtimeSource, "NR3_AGENT_LOOP_RESERVE_BYTES"),
  providerRequestHardLimitBytes: numericConstant(providerSource, "MAX_REQUEST_BYTES"),
};
if (
  profile.capabilityInstructionBytes !== 3 * 1024 ||
  profile.capabilityCatalogBytes !== 4 * 1024 ||
  profile.capabilityResourceContentBytes !== 6 * 1024 ||
  profile.capabilityResourceItems !== 8 ||
  profile.canonContentBytes !== 10 * 1024 ||
  profile.canonFragmentBytes !== 4 * 1024 ||
  profile.canonOperations !== 8 ||
  profile.canonUniqueFiles !== 8 ||
  profile.canonListPaths !== 64 ||
  profile.canonSearchMatches !== 24 ||
  profile.contextBundleBytes !== 24 * 1024 ||
  profile.agentLoopReserveBytes !== 40 * 1024 ||
  profile.providerRequestHardLimitBytes !== 96 * 1024
) {
  throw new Error("NR-306 frozen Context/Provider budget drift");
}

const claims = {
  status: "NR-G4-technical-submission",
  nrG4Approved: false,
  storiesCompleted: ["NR-301", "NR-302", "NR-303", "NR-304", "NR-305", "NR-306"],
  sourcePackageId: pack.sourcePackage.id,
  sourcePackageVersion: pack.sourcePackage.version,
  sourceCommit: pack.sourcePackage.commit,
  sourceArchiveSha256: pack.sourcePackage.archiveSha256,
  projectionId: pack.projection.id,
  projectionSchemaVersion: pack.projection.schemaVersion,
  projectionContentVersion: pack.projection.contentVersion,
  capabilityCount: manifestIds.length,
  resourceCount: inventory.fileCount,
  packBytes: inventory.totalBytes,
  packSha256: inventory.packSha256,
  licensePresent: existsSync(path.join(root, "resources/skills/novelist/v1/LICENSE.upstream")),
  attributionPresent: existsSync(path.join(root, "resources/skills/novelist/v1/ATTRIBUTION.md")),
  productionRegistryTools: 1,
  providerVisibleToolName: "novelist.read_context",
  toolContractVersion: 1,
  providerAdapters: 2,
  providerRequestBodiesCaptured: 4,
  providerEvidenceTransport: "127.0.0.1-loopback",
  sameUserTextCapturedByBothAdapters: true,
  providerCapabilityChoicesExecutedWithoutHostSubstitution: 2,
  maximumCapabilityResourcesPerRequest: profile.capabilityResourceItems,
  capabilityResourceIdsUnique: true,
  canonOperationKindsCovered: 3,
  maximumCanonOperationsPerTurn: profile.canonOperations,
  maximumCanonFilesPerTurn: profile.canonUniqueFiles,
  maximumCanonListPaths: profile.canonListPaths,
  maximumCanonSearchMatches: profile.canonSearchMatches,
  maximumContextBundleBytes: profile.contextBundleBytes,
  maximumProviderBodyBytes: profile.providerRequestHardLimitBytes,
  agentLoopReserveBytes: profile.agentLoopReserveBytes,
  canonWritesPerTurn: 0,
  keywordRouterBranches: 0,
  userVisibleSkillSelectors: 0,
  controlJsonMessages: 0,
  newIpcCommands: 0,
  productionCommandCount: ipc.manifest.length,
  conversationCommandCount: ipc.conversation.length,
  runtimeCapabilityExternalLoads: 0,
  automatedExternalRequests: 0,
  realProviderLiveSmokeExecuted: false,
  nr4EnteredOrPacketFormed: false,
};

const reportRelative = "docs/reviews/novelist-agent-runtime-nr-g4-technical-submission.zh-CN.md";
if (!prepare) {
  const report = read(reportRelative);
  const reportMatch = report.match(
    /<!-- NR-G4-MANIFEST-CLAIMS:START -->\s*```json\s*([\s\S]*?)\s*```\s*<!-- NR-G4-MANIFEST-CLAIMS:END -->/u,
  );
  if (!reportMatch) throw new Error("NR-G4 report machine claims are missing");
  const reportClaims = JSON.parse(reportMatch[1]);
  if (JSON.stringify(reportClaims) !== JSON.stringify(claims)) {
    throw new Error("NR-G4 report claims drift from machine evidence");
  }
  for (const relative of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"]) {
    const status = read(relative);
    if (
      !status.includes('id: "NR-G4"') ||
      !status.includes('status: "technical_submission"') ||
      !status.includes("nrG4Approved: false") ||
      !status.includes("implementation_completed: true") ||
      status.includes("real_provider_live_smoke_executed: true") ||
      status.includes("nr_4_entered_or_packet_formed: true")
    ) {
      throw new Error(`NR-G4 BMAD state drift: ${relative}`);
    }
  }
}

const manifest = {
  schemaVersion: 1,
  gateId: "NR-G4",
  result: "pass",
  claims,
  profile,
  testRuns,
  conversationCommands: ipc.conversation,
  releaseFileCount: releaseFiles.length,
  releaseScopeSha256: releaseScopeSha256(root, releaseFiles),
  sourceSha256: Object.fromEntries(
    [
      "crates/app-core/src/capability_pack.rs",
      "crates/app-core/src/canon_context.rs",
      "crates/app-core/src/tool_runtime.rs",
      "crates/app-core/src/provider_core.rs",
      "crates/app-core/src/conversation_runtime.rs",
      "crates/app-core/src/memory.rs",
      "resources/skills/novelist/v1/pack.json",
      "resources/skills/novelist/v1/inventory.json",
    ].map((relative) => [relative, sha256(read(relative))]),
  ),
};
manifest.payloadSha256 = sha256(JSON.stringify(manifest));
const evidenceRoot = path.join(root, "target/qa/nr-306");
mkdirSync(evidenceRoot, { recursive: true });
writeFileSync(
  path.join(evidenceRoot, "nr-g4-manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
  "utf8",
);
writeFileSync(
  path.join(evidenceRoot, "nr-g4-claims.json"),
  `${JSON.stringify(claims, null, 2)}\n`,
  "utf8",
);
console.log(
  `[nr-306] PASS ${claims.capabilityCount} capabilities / ${claims.productionRegistryTools} production Tool / ${claims.providerAdapters} loopback adapters / Canon writes 0 / live smoke false`,
);
