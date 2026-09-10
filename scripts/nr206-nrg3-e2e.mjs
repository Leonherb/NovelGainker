import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  discoverReleaseSurface,
  inspectIpcClosure,
  releaseScopeSha256,
} from "./nr-g1-gate-lib.mjs";

const root = process.cwd();
const read = (relative) => readFileSync(path.join(root, relative), "utf8");
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const numericConstant = (source, name) => {
  const match = source.match(
    new RegExp(`(?:pub )?const ${name}:\\s*(?:u32|usize)\\s*=\\s*([^;]+);`, "u"),
  );
  if (!match) throw new Error(`NR-206 numeric constant is missing: ${name}`);
  const expression = match[1].replaceAll("_", "").replaceAll(/\s/gu, "");
  if (!/^[0-9*+]+$/u.test(expression)) {
    throw new Error(`NR-206 numeric constant is not mechanically evaluable: ${name}`);
  }
  return expression
    .split("+")
    .map((sumPart) => sumPart.split("*").reduce((product, value) => product * Number(value), 1))
    .reduce((sum, value) => sum + value, 0);
};
const durationConstant = (source, name, unit) => {
  const match = source.match(
    new RegExp(`pub const ${name}: Duration = Duration::from_${unit}\\(([^)]+)\\);`, "u"),
  );
  if (!match) throw new Error(`NR-206 duration constant is missing: ${name}`);
  return Number(match[1].replaceAll("_", "").trim());
};
const stripRustTests = (source) =>
  source
    .replaceAll("\r\n", "\n")
    .replace(/\n#\[cfg\(test\)\]\n(?:pub\(crate\) )?mod tests \{[\s\S]*$/u, "\n");

const providerSource = read("crates/app-core/src/provider_core.rs");
const runtimeSource = read("crates/app-core/src/conversation_runtime.rs");
const toolSource = read("crates/app-core/src/tool_runtime.rs");
const taskSource = read("crates/app-core/src/task_store.rs");
const commandSource = read("apps/desktop/src-tauri/src/commands_current.rs");
const clientSource = read("apps/desktop/src/lib/tauri/conversation.ts");
const viewSource = read("apps/desktop/src/components/ConversationView.tsx");
const activitySource = read("apps/desktop/src/components/ToolActivityList.tsx");

const profile = {
  toolDefinitions: numericConstant(toolSource, "MAX_TOOL_DEFINITIONS"),
  toolDescriptionBytes: numericConstant(toolSource, "MAX_TOOL_DESCRIPTION_BYTES"),
  toolSchemaBytes: numericConstant(toolSource, "MAX_TOOL_SCHEMA_BYTES"),
  allToolDefinitionsBytes: numericConstant(toolSource, "MAX_TOOL_DEFINITIONS_BYTES"),
  toolArgumentsBytes: numericConstant(toolSource, "MAX_TOOL_ARGUMENTS_BYTES"),
  toolResultBytes: numericConstant(toolSource, "MAX_TOOL_RESULT_BYTES"),
  compactorOutputTokens: numericConstant(runtimeSource, "NR2_COMPACTOR_OUTPUT_BUDGET"),
  agentAttemptOutputTokens: numericConstant(runtimeSource, "NR2_AGENT_ATTEMPT_OUTPUT_BUDGET"),
  cumulativeOutputTokens: numericConstant(runtimeSource, "NR2_CUMULATIVE_OUTPUT_BUDGET"),
  toolDeadlineSeconds: durationConstant(runtimeSource, "TOOL_EXECUTION_DEADLINE", "secs"),
  cancellationAcknowledgementMs:
    durationConstant(runtimeSource, "TOOL_CANCELLATION_ACK_DEADLINE", "secs") * 1000,
};

if (
  profile.compactorOutputTokens + 2 * profile.agentAttemptOutputTokens !==
  profile.cumulativeOutputTokens
) {
  throw new Error("NR-206 cumulative requested-output budget drift");
}
if (profile.toolDeadlineSeconds !== 30 || profile.cancellationAcknowledgementMs !== 2000) {
  throw new Error("NR-206 Tool deadline or cancellation acknowledgement drift");
}

for (const required of [
  'body["parallel_tool_calls"] = Value::Bool(false)',
  '"reasoning.encrypted_content"',
  '"function_call_output"',
  'body["tool_choice"] = Value::String("none".to_owned())',
  'body["thinking"] = json!({"type": "disabled"})',
]) {
  if (!providerSource.includes(required)) {
    throw new Error(`NR-206 Provider-native serialization boundary is missing: ${required}`);
  }
}

const runtimeViewRequired = [
  "conversation_session_load",
  "ConversationRuntimeView",
  "AgentRuntimeEvent::Task",
];
for (const required of runtimeViewRequired) {
  if (!commandSource.includes(required) && !clientSource.includes(required)) {
    throw new Error(`NR-206 Runtime View/Event boundary is missing: ${required}`);
  }
}
// The current UI renders per-call records instead of the retired aggregate summary.
if (!viewSource.includes("<ToolActivityList") || !activitySource.includes("STATUS[task.status]")) {
  throw new Error("NR-206 controlled Task Activity rendering is missing");
}
for (const label of ["等待执行", "正在执行", "执行完成", "执行失败", "已中止"]) {
  if (!activitySource.includes(label)) {
    throw new Error(`NR-206 controlled inline task label is missing: ${label}`);
  }
}
if (!viewSource.includes('role="status"') || !viewSource.includes('aria-live="polite"')) {
  throw new Error("NR-206 active Task Activity live-region boundary is missing");
}

const taskRecord = taskSource.match(/pub struct TaskRecord \{([\s\S]*?)\n\}/u)?.[1];
if (!taskRecord) throw new Error("NR-206 TaskRecord schema is missing");
for (const forbidden of ["arguments_json", "output_json", "provider_continuation", "api_key"]) {
  if (taskRecord.includes(forbidden)) {
    throw new Error(`NR-206 TaskRecord persists a forbidden raw field: ${forbidden}`);
  }
}

for (const required of [
  "validate_live_transition_payload",
  "sha256(result.call_id.as_bytes()) != task.call_id_hash",
  "valid_stored_task_state",
  "FAILED_TASK_ERRORS",
  "RECOVERY_INTERRUPTED_ERROR",
]) {
  if (!taskSource.includes(required)) {
    throw new Error("NR-G3-R01 strict Task Store boundary is missing: " + required);
  }
}
for (const required of ["ExecutorUnqualified", "qualified_executor_contracts_cover_"]) {
  if (!toolSource.includes(required)) {
    throw new Error("NR-G3-R02 closed Registry boundary is missing: " + required);
  }
}
for (const required of [
  "execute_with_profile_and_registry",
  "compactor_tool_turn_uses_committed_memory_and_stays_within_three_provider_attempts",
]) {
  if (!runtimeSource.includes(required)) {
    throw new Error("NR-G3-R03 shared Turn orchestration evidence is missing: " + required);
  }
}

const releaseFiles = discoverReleaseSurface(root);
const syntheticIds = ["test.echo", "test.fail", "test.block_until_cancelled"];
const syntheticFindings = [];
for (const absolute of releaseFiles) {
  const original = readFileSync(absolute, "utf8");
  const productionSource = absolute.endsWith(".rs") ? stripRustTests(original) : original;
  for (const id of syntheticIds) {
    if (productionSource.includes(id)) {
      syntheticFindings.push({
        file: path.relative(root, absolute).split(path.sep).join("/"),
        id,
      });
    }
  }
}
if (syntheticFindings.length !== 0) {
  throw new Error(
    `NR-206 synthetic Tool leaked into production: ${JSON.stringify(syntheticFindings[0])}`,
  );
}

const requiredTests = [
  [providerSource, "both_adapters_capture_native_tool_requests_and_typed_continuations"],
  [providerSource, "native_tool_parsers_reject_multiple_or_unknown_required_items"],
  [providerSource, "continuation_bounds_and_provider_identity_reject_before_outbound"],
  [runtimeSource, "one_tool_round_executes_once_and_keeps_task_terminal_independent"],
  [runtimeSource, "second_tool_call_fails_with_the_stable_round_limit_without_reexecution"],
  [runtimeSource, "schema_rejection_records_a_failed_task_and_continues_without_execution"],
  [runtimeSource, "tool_failure_is_terminal_for_task_but_attempt_two_can_complete_conversation"],
  [
    runtimeSource,
    "cancelling_a_running_tool_acknowledges_within_two_seconds_and_skips_attempt_two",
  ],
  [runtimeSource, "nr2_requested_output_budget_is_mechanically_bounded"],
  [taskSource, "interrupted_tasks_cancel_once_and_terminal_never_regresses"],
  [taskSource, "duplicate_call_id_is_rejected_without_a_second_task"],
  [taskSource, "tool_result_identity_status_and_error_are_strict_before_write"],
  [taskSource, "persisted_task_state_matrix_hashes_and_error_allowlist_are_strict"],
  [taskSource, "invalid_store_is_rejected_before_recovery_without_rewrite"],
  [toolSource, "production_registry_is_empty"],
  [toolSource, "provider_visible_tool_names_are_unique_across_versions"],
  [toolSource, "qualified_executor_contracts_cover_success_failure_and_two_second_cancellation"],
  [toolSource, "unqualified_executor_is_rejected_before_visibility_or_execution"],
  [
    runtimeSource,
    "compactor_tool_turn_uses_committed_memory_and_stays_within_three_provider_attempts",
  ],
  [
    read("apps/desktop/src/components/ConversationView.reconciliation.test.tsx"),
    "merges Task snapshots and events only by a greater taskRevision",
  ],
  [
    read("apps/desktop/src/components/ConversationView.test.tsx"),
    "restores every Host tool record after reopening without generating an API reply",
  ],
];
for (const [source, testName] of requiredTests) {
  if (!source.includes(testName)) {
    throw new Error(`NR-206 acceptance test is missing: ${testName}`);
  }
}

const testEnvironment = { ...process.env, CARGO_NET_OFFLINE: "true" };
const secretName = /(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu;
for (const name of Object.keys(testEnvironment)) {
  if (secretName.test(name)) delete testEnvironment[name];
}
const testRuns = [];
for (const filter of [
  "tool_runtime::tests",
  "task_store::tests",
  "provider_core::tests",
  "conversation_runtime::tests",
]) {
  const startedAt = performance.now();
  const result = spawnSync("cargo", ["test", "-p", "app-core", "--locked", "--offline", filter], {
    cwd: root,
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`NR-206 Rust evidence failed: ${filter}`);
  testRuns.push({
    filter,
    result: "pass",
    observedDurationMs: Math.round(performance.now() - startedAt),
  });
}

const ipc = inspectIpcClosure(root);
const claims = {
  status: "NR-G3-technical-submission",
  nrG3Approved: false,
  storiesCompleted: ["NR-201", "NR-202", "NR-203", "NR-204", "NR-205", "NR-206"],
  providerAdapters: 2,
  nativeToolRequestBodiesCaptured: 4,
  maximumProviderAttemptsPerTurn: 3,
  maximumAgentAttemptsPerTurn: 2,
  maximumToolExecutionsPerTurn: 1,
  maximumToolRoundsPerTurn: 1,
  automaticRetriesPerTurn: 0,
  compactorRequestedOutputTokens: profile.compactorOutputTokens,
  eachAgentAttemptRequestedOutputTokens: profile.agentAttemptOutputTokens,
  cumulativeRequestedOutputTokens: profile.cumulativeOutputTokens,
  toolDeadlineSeconds: profile.toolDeadlineSeconds,
  cancellationAcknowledgementMs: profile.cancellationAcknowledgementMs,
  maximumToolDefinitions: profile.toolDefinitions,
  maximumToolArgumentsBytes: profile.toolArgumentsBytes,
  maximumToolResultBytes: profile.toolResultBytes,
  maximumContinuationItemBytes: 32 * 1024,
  maximumContinuationBytes: 48 * 1024,
  providerRequestHardLimitBytes: numericConstant(providerSource, "MAX_REQUEST_BYTES"),
  strictToolResultCallPairing: true,
  strictTaskStateMatrixValidated: true,
  taskErrorCodeAllowlistValidated: true,
  invalidTaskStoreRecoveryRejected: true,
  unqualifiedExecutorRegistrationRejected: true,
  productionRegistryTools: 0,
  sharedProductionTestTurnOrchestrator: true,
  compactorMemoryConsumedByInitial: true,
  compactorToolCombinedProviderAttempts: 3,
  mainProviderAttemptsWithTool: 2,
  toolExecutions: 1,
  syntheticToolIdsInProduction: syntheticFindings.length,
  productionCommandCount: ipc.baselineManifest.length,
  conversationCommandCount: ipc.conversation.length,
  skillCallsPerTurn: 0,
  canonReadsPerTurn: 0,
  canonWritesPerTurn: 0,
  externalToolCallsPerTurn: 0,
  automatedExternalRequests: 0,
  realProviderLiveSmokeExecuted: false,
  nr3EnteredOrPacketFormed: false,
};

const report = read("docs/reviews/novelist-agent-runtime-nr-g3-technical-submission.zh-CN.md");
const reportMatch = report.match(
  /<!-- NR-G3-MANIFEST-CLAIMS:START -->\s*```json\s*([\s\S]*?)\s*```\s*<!-- NR-G3-MANIFEST-CLAIMS:END -->/u,
);
if (!reportMatch) throw new Error("NR-G3 report machine claims are missing");
const reportClaims = JSON.parse(reportMatch[1]);
if (JSON.stringify(reportClaims) !== JSON.stringify(claims)) {
  throw new Error("NR-G3 report claims drift from machine evidence");
}

for (const relative of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"]) {
  const status = read(relative);
  if (
    !status.includes('id: "NR-G3"') ||
    !status.includes('status: "technical_submission"') ||
    !status.includes("nrG3Approved: false") ||
    !status.includes("implementation_completed: true") ||
    status.includes("real_provider_live_smoke_executed: true") ||
    status.includes("nr_3_to_nr_7_packet_authorized: true")
  ) {
    throw new Error(`NR-G3 BMAD state drift: ${relative}`);
  }
}

const manifest = {
  schemaVersion: 1,
  gateId: "NR-G3",
  evidenceScope: ipc.nr4Commands.length
    ? "historical-nr2-contract-regression"
    : "technical-submission",
  currentProductionCommandCount: ipc.manifest.length,
  result: "pass",
  claims,
  profile,
  testRuns,
  conversationCommands: ipc.conversation,
  releaseFileCount: releaseFiles.length,
  releaseScopeSha256: releaseScopeSha256(root, releaseFiles),
  sourceSha256: Object.fromEntries(
    [
      "crates/app-core/src/tool_runtime.rs",
      "crates/app-core/src/task_store.rs",
      "crates/app-core/src/provider_core.rs",
      "crates/app-core/src/conversation_runtime.rs",
      "apps/desktop/src-tauri/src/commands_current.rs",
      "apps/desktop/src/lib/tauri/conversation.ts",
      "apps/desktop/src/components/ConversationView.tsx",
    ].map((relative) => [relative, sha256(read(relative))]),
  ),
};
manifest.payloadSha256 = sha256(JSON.stringify(manifest));
const evidenceRoot = path.join(root, "target/qa/nr-206");
mkdirSync(evidenceRoot, { recursive: true });
writeFileSync(
  path.join(
    evidenceRoot,
    ipc.nr4Commands.length ? "nr-g3-regression-manifest.json" : "nr-g3-manifest.json",
  ),
  `${JSON.stringify(manifest, null, 2)}\n`,
  "utf8",
);
console.log(
  `[nr-206] PASS ${claims.providerAdapters} adapters / ${claims.maximumProviderAttemptsPerTurn} max Provider attempts / ${claims.maximumToolExecutionsPerTurn} max Tool execution / live smoke false`,
);
for (const required of [
  "ToolRegistry::empty_production()",
  "agent.tool_round_limit",
  "execute_bounded_tool",
  "terminalize_cancelled_turn_with_text",
]) {
  if (!runtimeSource.includes(required)) {
    throw new Error(`NR-206 Agent Loop boundary is missing: ${required}`);
  }
}
for (const required of [
  "ConversationRuntimeView",
  "runtime.interrupted",
  "task_revision",
  "call_id_hash",
  "arguments_sha256",
  "result_sha256",
]) {
  if (!taskSource.includes(required)) {
    throw new Error(`NR-206 Task Store boundary is missing: ${required}`);
  }
}
