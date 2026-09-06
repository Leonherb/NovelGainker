import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { currentNr6Authorization, stateSection } from "./nr-stage-state.mjs";
import {
  inspectIpcClosure,
  discoverReleaseSurface,
  releaseScopeSha256,
  NR601_SESSION_COMMANDS,
} from "./nr-g1-gate-lib.mjs";
let negativeChecks = 0;
for (const file of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"]) {
  const state = readFileSync(file, "utf8");
  assert(currentNr6Authorization(state));
  const block = stateSection(state, "nr_g6_technical_submission");
  for (const line of block
    .trimEnd()
    .split(/\r?\n/u)
    .filter((line) => line.trim())) {
    assert(!currentNr6Authorization(state.replace(line, line.replace(/: .*/u, ': "invalid"'))));
    assert(!currentNr6Authorization(state.replace(line, "")));
    negativeChecks += 2;
  }
  assert(!currentNr6Authorization(state + "\nnr_g6_technical_submission:\n" + block));
  assert(
    !currentNr6Authorization(state.replace("nr_g6_technical_submission:", "missing_submission:")),
  );
  negativeChecks += 2;
}
const ipc = inspectIpcClosure(process.cwd());
assert.equal(
  ipc.manifest.length - ipc.sessionManagementCommands.length - ipc.retryCommands.length,
  34,
);
assert.deepEqual(ipc.sessionCatalogCommands, [...NR601_SESSION_COMMANDS].sort());
assert.equal(
  ipc.allConversationCommands.length -
    ipc.sessionManagementCommands.length -
    ipc.retryCommands.length,
  7,
);
const result = spawnSync(
  "cargo",
  ["test", "-p", "app-core", "--lib", "--locked", "--offline", "nr601_", "--", "--nocapture"],
  {
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    env: { ...process.env, CARGO_NET_OFFLINE: "true" },
  },
);
assert.ifError(result.error);
process.stdout.write(result.stdout ?? "");
process.stderr.write(result.stderr ?? "");
assert.equal(result.status, 0);
const output = result.stdout + result.stderr;
const tests = Number(output.match(/test result: ok\. (\d+) passed/u)?.[1] ?? 0);
assert(tests >= 14);
const providers = [
  ...output.matchAll(
    /NR601_SWITCH provider=(Deepseek|Openai) captured_history=true captured_tasks=true captured_events=true shared_canon=true external=0/gu,
  ),
]
  .map((row) => row[1])
  .sort();
assert.deepEqual(providers, ["Deepseek", "Openai"]);
const digest = (file) => createHash("sha256").update(readFileSync(file)).digest("hex");
const report = {
  schemaVersion: 1,
  story: "NR-601",
  result: "pass",
  completedAt: new Date().toISOString(),
  productionCommandCount: ipc.manifest.length,
  sessionCommands: ipc.sessionCommands,
  currentAuthorizationNegativeChecks: negativeChecks,
  rustTestsPassed: tests,
  releaseScopeSha256: releaseScopeSha256(process.cwd(), discoverReleaseSurface(process.cwd())),
  supplementalSourceHashes: [
    "scripts/nr601-session-isolation-e2e.mjs",
    "scripts/nr-stage-state.mjs",
    "scripts/nr-g1-gate-lib.mjs",
  ].map((file) => ({ file, sha256: digest(file) })),
  loopbackProviders: providers,
  realDesktopCommands: true,
  mockWindow: true,
  realProviderExecuted: false,
  multiUserTestingExecuted: false,
  externalNetworkRequests: 0,
  nrG7Approved: false,
  limitations: [
    "Actual desktop commands and worker with loopback protocols, not a native WebView click-through or real Provider semantic validation.",
    "Catalog/History/Memory/Task, quotas, crash-safe publication of catalog, CAS, Windows reparse rejection and captured session ownership are directed tests; full Story quality remains required.",
    "Archive operations, branching and export belong to NR-602 and are not exposed by this Story.",
  ],
};
mkdirSync("target/qa/nr-601", { recursive: true });
writeFileSync(
  "target/qa/nr-601/regression-validation.json",
  JSON.stringify(report, null, 2) + "\n",
);
console.log(
  "[nr-601] PASS scoped sessions and actual desktop switching; full quality still required",
);
