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
const numberConstant = (source, name) => {
  const match = source.match(new RegExp(`${name}:\\s*(?:u32|usize)\\s*=\\s*([^;]+);`, "u"));
  if (!match) throw new Error(`NR-106 Rust constant is missing: ${name}`);
  const expression = match[1].replaceAll("_", "").replaceAll(/\s/gu, "");
  if (!/^[0-9*+]+$/u.test(expression)) {
    throw new Error(`NR-106 Rust constant is not mechanically evaluable: ${name}`);
  }
  return expression
    .split("+")
    .map((sumPart) => sumPart.split("*").reduce((product, value) => product * Number(value), 1))
    .reduce((sum, value) => sum + value, 0);
};
const profileField = (source, name) => {
  const match = source.match(new RegExp(`${name}:\\s*([^,]+),`, "u"));
  if (!match) throw new Error(`NR-106 selected profile field is missing: ${name}`);
  const expression = match[1].replaceAll("_", "").replaceAll(/\s/gu, "");
  if (!/^[0-9*+]+$/u.test(expression)) {
    throw new Error(`NR-106 selected profile field is not mechanically evaluable: ${name}`);
  }
  return expression
    .split("+")
    .map((sumPart) => sumPart.split("*").reduce((product, value) => product * Number(value), 1))
    .reduce((sum, value) => sum + value, 0);
};

const providerSource = read("crates/app-core/src/provider_core.rs");
const memorySource = read("crates/app-core/src/memory.rs");
const runtimeSource = read("crates/app-core/src/conversation_runtime.rs");
const conversationSource = read("crates/app-core/src/conversation.rs");
const selectedProfileMatch = memorySource.match(
  /pub const NR1_CONTEXT_PROFILE: ContextProfile = ContextProfile \{([\s\S]*?)\n\};/u,
);
if (!selectedProfileMatch) throw new Error("NR-106 selected Rust profile is missing");
const selectedProfileSource = selectedProfileMatch[1];
const commandsSource = read("apps/desktop/src-tauri/src/commands_current.rs");

const maximumMessageCount = numberConstant(providerSource, "MAX_MESSAGE_COUNT");
const maximumMessageBytes = numberConstant(providerSource, "MAX_MESSAGE_BYTES");
const maximumRequestBytes = numberConstant(providerSource, "MAX_REQUEST_BYTES");
const maximumResponseBytes = numberConstant(providerSource, "MAX_RESPONSE_BYTES");
const selectedProfile = {
  triggerBytes: profileField(selectedProfileSource, "trigger_bytes"),
  protectedTailBytes: profileField(selectedProfileSource, "protected_tail_bytes"),
  segmentContentBytes: profileField(selectedProfileSource, "segment_content_bytes"),
  compactorInputBytes: profileField(selectedProfileSource, "compactor_input_bytes"),
  backfillInputBytes: profileField(selectedProfileSource, "backfill_input_bytes"),
  summaryBytes: profileField(selectedProfileSource, "summary_bytes"),
  compactorRequestedMax: profileField(selectedProfileSource, "compactor_requested_max"),
  mainReplyRequestedMax: profileField(selectedProfileSource, "main_reply_requested_max"),
  cumulativeRequestedOutputMax: profileField(
    selectedProfileSource,
    "cumulative_requested_output_max",
  ),
};

const systemMatch = providerSource.match(
  /pub const CONVERSATION_SYSTEM_INSTRUCTION: &str =\s*"([^"]+)";/u,
);
if (!systemMatch) throw new Error("NR-106 Conversation system instruction is missing");
const systemInstructionBytes = Buffer.byteLength(systemMatch[1], "utf8");
const mainRequestReserveBytes = 2 * 1024;
const guaranteedRecentCapacityBytes =
  maximumRequestBytes -
  systemInstructionBytes -
  selectedProfile.summaryBytes -
  maximumMessageBytes -
  mainRequestReserveBytes;

const candidates = [
  {
    id: "compact-48k",
    triggerBytes: 48 * 1024,
    protectedTailBytes: 16 * 1024,
    segmentContentBytes: 4 * 1024,
    compactorInputBytes: 64 * 1024,
    backfillInputBytes: 8 * 1024,
    summaryBytes: 8 * 1024,
    compactorRequestedMax: 768,
    mainReplyRequestedMax: 4096,
    cumulativeRequestedOutputMax: 4864,
    selected: false,
    observation: "更早触发且摘要余量较小；机械可行，但长篇对话的稳定摘要空间偏紧。",
  },
  {
    id: "balanced-64k",
    ...selectedProfile,
    selected: true,
    observation: "在 96 KiB 硬边界内同时保留 24 KiB 精确尾部、8 KiB 回填和 12 KiB 摘要。",
  },
  {
    id: "tail-heavy-80k",
    triggerBytes: 80 * 1024,
    protectedTailBytes: 32 * 1024,
    segmentContentBytes: 6 * 1024,
    compactorInputBytes: 80 * 1024,
    backfillInputBytes: 12 * 1024,
    summaryBytes: 14 * 1024,
    compactorRequestedMax: 1024,
    mainReplyRequestedMax: 4096,
    cumulativeRequestedOutputMax: 5120,
    selected: false,
    observation: "精确尾部更大，但最大当前消息与摘要同时存在时的请求余量更窄。",
  },
];

for (const candidate of candidates) {
  const outputSum = candidate.compactorRequestedMax + candidate.mainReplyRequestedMax;
  const recentCapacity =
    maximumRequestBytes -
    systemInstructionBytes -
    candidate.summaryBytes -
    maximumMessageBytes -
    mainRequestReserveBytes;
  if (
    candidate.triggerBytes <= 0 ||
    candidate.triggerBytes >= maximumRequestBytes ||
    candidate.segmentContentBytes <= 0 ||
    candidate.segmentContentBytes > candidate.backfillInputBytes ||
    candidate.compactorInputBytes >= maximumRequestBytes ||
    outputSum > candidate.cumulativeRequestedOutputMax ||
    recentCapacity < candidate.protectedTailBytes
  ) {
    throw new Error(`NR-106 candidate violates a mechanical boundary: ${candidate.id}`);
  }
  candidate.guaranteedRecentCapacityBytes = recentCapacity;
}

if (
  selectedProfile.compactorRequestedMax + selectedProfile.mainReplyRequestedMax >
  selectedProfile.cumulativeRequestedOutputMax
) {
  throw new Error("NR-106 cumulative requested output boundary is invalid");
}
if (guaranteedRecentCapacityBytes < selectedProfile.protectedTailBytes) {
  throw new Error("NR-106 selected profile cannot reserve the protected exact tail");
}
const maximumCompactorMessageCount =
  Math.ceil(selectedProfile.compactorInputBytes / selectedProfile.segmentContentBytes) + 2;
if (maximumCompactorMessageCount > maximumMessageCount) {
  throw new Error("NR-106 selected profile can exceed the Provider message-count boundary");
}

for (const required of ["complete_once_cancellable_blocking", "stream_once_cancellable_blocking"]) {
  if (!providerSource.includes(required))
    throw new Error(`NR-106 Provider primitive missing: ${required}`);
}
if (
  !runtimeSource.includes("complete_once_cancellable_blocking") ||
  !conversationSource.includes("stream_once_cancellable_blocking") ||
  !commandsSource.includes("execute_memory_managed_turn")
) {
  throw new Error(
    "NR-106 release Conversation orchestration is not using the frozen zero-retry path",
  );
}

const remediationTests = [
  [providerSource, "dynamic_system_instruction_is_captured_for_both_adapters_and_modes"],
  [providerSource, "invalid_system_contract_is_rejected_before_any_outbound_attempt"],
  [memorySource, "repeated_normal_compaction_backfills_at_least_one_segment_without_gaps"],
  [memorySource, "summary_validation_enforces_exact_ordered_unique_nonempty_sections"],
  [memorySource, "invalid_summary_never_creates_or_replaces_memory"],
  [runtimeSource, "valid_memory_without_incremental_plan_skips_compactor_and_preserves_bytes"],
  [runtimeSource, "invalid_compactor_summary_is_hidden_and_main_reply_still_runs_once"],
];
for (const [source, testName] of remediationTests) {
  if (!source.includes(testName)) {
    throw new Error("NR-G2 remediation evidence test is missing: " + testName);
  }
}

const testEnvironment = { ...process.env, CARGO_NET_OFFLINE: "true" };
const secretName = /(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu;
for (const name of Object.keys(testEnvironment)) {
  if (secretName.test(name)) delete testEnvironment[name];
}
const testFilters = ["memory::tests", "conversation_runtime::tests", "provider_core::tests"];
const testRuns = [];
for (const filter of testFilters) {
  const startedAt = performance.now();
  const result = spawnSync("cargo", ["test", "-p", "app-core", "--locked", "--offline", filter], {
    cwd: root,
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`NR-106 Rust evidence failed: ${filter}`);
  testRuns.push({
    filter,
    result: "pass",
    observedDurationMs: Math.round(performance.now() - startedAt),
  });
}

const ipc = inspectIpcClosure(root);
const releaseFiles = discoverReleaseSurface(root);
const longAssistantFixtureBytes = Buffer.byteLength("长".repeat(18_000), "utf8");
const longAssistantSegmentCount = Math.ceil(
  longAssistantFixtureBytes / selectedProfile.segmentContentBytes,
);
const claims = {
  status: "NR-G2-technical-submission",
  nrG2Approved: false,
  storiesCompleted: ["NR-101", "NR-102", "NR-103", "NR-104", "NR-105", "NR-106"],
  remediationsCompleted: ["NR-G2-R01", "NR-G2-R02", "NR-G2-R03"],
  dynamicSystemInstructionSerializationPathsTested: 4,
  invalidSystemContractOutboundAttempts: 0,
  validMemoryIncrementalUnavailableCompactorAttempts: 0,
  summaryHeadingsRequired: 5,
  selectedProfileId: "balanced-64k",
  maximumOutboundAttemptsPerTurn: 2,
  belowThresholdOutboundAttempts: 1,
  compactionTriggeredOutboundAttempts: 2,
  automaticRetriesPerTurn: 0,
  skillCallsPerTurn: 0,
  toolCallsPerTurn: 0,
  canonOperationsPerTurn: 0,
  visibleHistoryFixtureMessages: 10_000,
  providerMessageHardLimit: maximumMessageCount,
  providerRequestHardLimitBytes: maximumRequestBytes,
  currentMessageHardLimitBytes: maximumMessageBytes,
  responseHardLimitBytes: maximumResponseBytes,
  longAssistantFixtureBytes,
  longAssistantSegmentCount,
  currentUserMessageOccurrences: 1,
  backfillMinimumProgressSegments: 1,
  automatedExternalRequests: 0,
  realProviderLiveSmokeExecuted: false,
  realProviderSummarySemanticsVerified: false,
};

const report = read("docs/reviews/novelist-agent-runtime-nr-g2-technical-submission.zh-CN.md");
const reportMatch = report.match(
  /<!-- NR-G2-MANIFEST-CLAIMS:START -->\s*```json\s*([\s\S]*?)\s*```\s*<!-- NR-G2-MANIFEST-CLAIMS:END -->/u,
);
if (!reportMatch) throw new Error("NR-G2 report machine claims are missing");
const reportClaims = JSON.parse(reportMatch[1]);
if (JSON.stringify(reportClaims) !== JSON.stringify(claims)) {
  throw new Error("NR-G2 report claims drift from machine evidence");
}

for (const relative of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"]) {
  const status = read(relative);
  if (
    !status.includes('id: "NR-G2"') ||
    !status.includes('status: "technical_submission"') ||
    !status.includes("nrG2Approved: false") ||
    status.includes("real_provider_live_smoke_executed: true") ||
    status.includes("nr_2_authorized: true")
  ) {
    throw new Error(`NR-G2 BMAD state drift: ${relative}`);
  }
}

const manifest = {
  schemaVersion: 1,
  gateId: "NR-G2",
  result: "pass",
  claims,
  selectedProfile: {
    ...selectedProfile,
    systemInstructionBytes,
    mainRequestReserveBytes,
    guaranteedRecentCapacityBytes,
    maximumCompactorMessageCount,
  },
  candidates,
  testRuns,
  conversationCommands: ipc.conversation,
  productionCommandCount: ipc.manifest.length,
  releaseFileCount: releaseFiles.length,
  releaseScopeSha256: releaseScopeSha256(root, releaseFiles),
  sourceSha256: Object.fromEntries(
    [
      "crates/app-core/src/memory.rs",
      "crates/app-core/src/conversation_runtime.rs",
      "crates/app-core/src/provider_core.rs",
      "crates/app-core/src/conversation.rs",
      "apps/desktop/src-tauri/src/commands_current.rs",
    ].map((relative) => [relative, sha256(read(relative))]),
  ),
};
manifest.payloadSha256 = sha256(JSON.stringify(manifest));
const evidenceRoot = path.join(root, "target/qa/nr-106");
mkdirSync(evidenceRoot, { recursive: true });
writeFileSync(
  path.join(evidenceRoot, "nr-g2-manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
  "utf8",
);
console.log(
  `[nr-106] PASS ${claims.visibleHistoryFixtureMessages} messages / ${claims.maximumOutboundAttemptsPerTurn} max attempts / ${selectedProfile.triggerBytes} byte trigger / live smoke false`,
);
