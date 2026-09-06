import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  assertReportClaims,
  discoverReleaseSurface,
  inspectIpcClosure,
  hasApprovedNr4Ipc,
  parseReportClaims,
  proveReleaseScannerRejectsNewProductionFile,
  releaseScopeSha256,
  scanRetiredProductReferences,
} from "./nr-g1-gate-lib.mjs";

const root = process.cwd();
const read = (relative) => readFileSync(path.join(root, relative), "utf8");
const readEvidence = (relative) => {
  const evidence = JSON.parse(read(relative));
  if (evidence.result !== "pass")
    throw new Error(`NR-G1 upstream evidence is not passing: ${relative}`);
  return evidence;
};
const requireNumber = (record, key, relative) => {
  if (!Number.isSafeInteger(record[key]) || record[key] < 0) {
    throw new Error(`NR-G1 upstream evidence field is missing: ${relative}#${key}`);
  }
  return record[key];
};
const requireBoolean = (record, key, relative) => {
  if (typeof record[key] !== "boolean") {
    throw new Error(`NR-G1 upstream evidence field is missing: ${relative}#${key}`);
  }
  return record[key];
};

const upstreamPaths = {
  nr001: "target/qa/nr-001/nr-001-evidence.json",
  nr002: "target/qa/nr-002/nr-002-evidence.json",
  nr003: "target/qa/nr-003/nr-003-evidence.json",
  nr004: "target/qa/nr-004/nr-004-evidence.json",
  nr005: "target/qa/nr-005/nr-005-evidence.json",
};
const upstream = Object.fromEntries(
  Object.entries(upstreamPaths).map(([name, relative]) => [name, readEvidence(relative)]),
);

const releaseFiles = discoverReleaseSurface(root);
const retiredFindings = scanRetiredProductReferences(root, releaseFiles);
if (retiredFindings.length !== 0) {
  throw new Error(`retired production reference remains: ${JSON.stringify(retiredFindings[0])}`);
}
const negativeProductionFixtureRejected = proveReleaseScannerRejectsNewProductionFile(root);
const ipc = inspectIpcClosure(root);
const nr4Entered = ipc.nr4Commands.length > 0;
if (nr4Entered) {
  const status = read("bmad/workflow-status.yaml");
  for (const field of ["product_approved", "implementation_authorized"]) {
    const changed = status.replace(/nr4_packet:[\s\S]*$/u, (packet) =>
      packet.replace(`${field}: true`, `${field}: false`),
    );
    if (hasApprovedNr4Ipc(changed)) throw new Error(`NR-4 missing ${field} was accepted`);
  }
}

const commandsSource = read("apps/desktop/src-tauri/src/commands_current.rs");
const handlerDeclarations = (
  commandsSource.match(/fn run_conversation_provider(?:<R: tauri::Runtime>)?\(/gu) ?? []
).length;
if (handlerDeclarations !== 1) throw new Error("Provider worker declaration drift");
const handlerProviderRuns =
  (commandsSource.match(/run_conversation_provider(?:<R: tauri::Runtime>)?\(/gu) ?? []).length -
  handlerDeclarations;
const managedRuntimeCalls = (commandsSource.match(/execute_memory_managed_turn\(/gu) ?? []).length;
const changeRuntimeCalls = (
  commandsSource.match(/execute_memory_managed_turn_with_changes\(/gu) ?? []
).length;
if (changeRuntimeCalls !== (nr4Entered ? 1 : 0)) {
  throw new Error("Current NR-4 runtime mount drift");
}
const runtimeSource = read("crates/app-core/src/conversation_runtime.rs");
const providerExecutions = (runtimeSource.match(/execute_provider_turn\(/gu) ?? []).length;
const releaseConversationPathCount = handlerProviderRuns * managedRuntimeCalls * providerExecutions;
if (releaseConversationPathCount !== 1) {
  throw new Error(`NR-G1 release Conversation path count drift: ${releaseConversationPathCount}`);
}

const automatedExternalRequests = Math.max(
  requireNumber(upstream.nr002, "automatedExternalRequests", upstreamPaths.nr002),
  requireNumber(upstream.nr004, "automatedExternalRequests", upstreamPaths.nr004),
  requireNumber(upstream.nr005, "automatedExternalRequests", upstreamPaths.nr005),
);
if (automatedExternalRequests !== 0) throw new Error("NR-G1 automation made an external request");
for (const [name, relative] of [
  ["nr004", upstreamPaths.nr004],
  ["nr005", upstreamPaths.nr005],
]) {
  if (requireBoolean(upstream[name], "liveProviderSmokeExecuted", relative)) {
    throw new Error(`real Provider Live Smoke was executed: ${relative}`);
  }
}

const manifest = {
  schemaVersion: 2,
  gateId: "NR-G1",
  status: "NR-G1-technical-submission",
  nrG1Approved: false,
  result: "pass",
  productionCommandCount: ipc.baselineManifest.length,
  evidenceScope: nr4Entered ? "historical-nr0-contract-regression" : "technical-submission",
  currentProductionCommandCount: ipc.manifest.length,
  currentNr4Commands: ipc.nr4Commands,
  conversationCommandCount: ipc.conversation.length,
  conversationCommands: ipc.conversation,
  commandSetsEqual: true,
  providerCallsPerTurn: requireNumber(upstream.nr004, "providerCallsPerTurn", upstreamPaths.nr004),
  skillCallsPerTurn: requireNumber(upstream.nr004, "skillCallsPerTurn", upstreamPaths.nr004),
  toolCallsPerTurn: requireNumber(upstream.nr004, "toolCallsPerTurn", upstreamPaths.nr004),
  canonOperationsPerTurn: requireNumber(
    upstream.nr004,
    "canonOperationsPerTurn",
    upstreamPaths.nr004,
  ),
  sessionReconciliationMaximumIntervalMs: requireNumber(
    upstream.nr005,
    "sessionReconciliationMaximumIntervalMs",
    upstreamPaths.nr005,
  ),
  releaseConversationPathCount,
  retiredProductReferences: retiredFindings.length,
  conversationStartsBlank: requireBoolean(
    upstream.nr002,
    "conversationStartsBlank",
    upstreamPaths.nr002,
  ),
  automatedExternalRequests,
  realProviderLiveSmokeExecuted: false,
  productionReleaseFileCount: releaseFiles.length,
  configuredProductionResourceCount: releaseFiles.filter((file) =>
    file.includes(`${path.sep}src-tauri${path.sep}resources${path.sep}`),
  ).length,
  negativeProductionFixtureRejected,
  releaseScopeSha256: releaseScopeSha256(root, releaseFiles),
  ipcSetsSha256: createHash("sha256").update(JSON.stringify(ipc)).digest("hex"),
  upstreamEvidenceSha256: Object.fromEntries(
    Object.entries(upstreamPaths).map(([name, relative]) => [
      name,
      createHash("sha256").update(read(relative)).digest("hex"),
    ]),
  ),
};

if (
  manifest.providerCallsPerTurn !== 1 ||
  manifest.skillCallsPerTurn !== 0 ||
  manifest.toolCallsPerTurn !== 0 ||
  manifest.canonOperationsPerTurn !== 0 ||
  manifest.sessionReconciliationMaximumIntervalMs > 2000 ||
  manifest.conversationStartsBlank !== true
) {
  throw new Error("NR-G1 derived acceptance metrics are outside the approved boundary");
}

const report = read("docs/reviews/novelist-agent-runtime-nr-g1-technical-submission.zh-CN.md");
const reportClaims = parseReportClaims(report);
assertReportClaims(manifest, reportClaims);
let reportDriftFixtureRejected = false;
try {
  assertReportClaims(manifest, {
    ...reportClaims,
    productionCommandCount: reportClaims.productionCommandCount + 1,
  });
} catch {
  reportDriftFixtureRejected = true;
}
if (!reportDriftFixtureRejected) throw new Error("NR-G1 report drift fixture was accepted");
manifest.reportDriftFixtureRejected = reportDriftFixtureRejected;

for (const relative of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"]) {
  const status = read(relative);
  if (
    !status.includes('status: "passed"') ||
    !status.includes("nrG1Approved: true") ||
    status.includes("real_provider_live_smoke_executed: true")
  ) {
    throw new Error(`NR-G1 BMAD/report Gate state drift: ${relative}`);
  }
}

manifest.payloadSha256 = createHash("sha256").update(JSON.stringify(manifest)).digest("hex");
const evidenceRoot = path.join(root, "target/qa/nr-006");
mkdirSync(evidenceRoot, { recursive: true });
writeFileSync(
  path.join(evidenceRoot, nr4Entered ? "nr-g1-regression-manifest.json" : "nr-g1-manifest.json"),
  JSON.stringify(manifest, null, 2) + "\n",
  "utf8",
);
console.log(
  `[nr-006] PASS ${releaseFiles.length} release files / ${ipc.manifest.length} current commands / 24 historical commands / 3 Conversation IPC`,
);
