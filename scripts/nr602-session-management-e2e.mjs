import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { currentNr602Authorization, stateSection } from "./nr-stage-state.mjs";
import {
  inspectIpcClosure,
  discoverReleaseSurface,
  releaseScopeSha256,
  NR602_SESSION_COMMANDS,
} from "./nr-g1-gate-lib.mjs";
let negativeChecks = 0;
for (const file of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"]) {
  const state = readFileSync(file, "utf8");
  assert(currentNr602Authorization(state));
  const block = stateSection(state, "nr601_completion");
  for (const line of block
    .trimEnd()
    .split(/\r?\n/u)
    .filter((line) => line.trim())) {
    assert(!currentNr602Authorization(state.replace(line, line.replace(/: .*/u, ': "invalid"'))));
    assert(!currentNr602Authorization(state.replace(line, "")));
    assert(!currentNr602Authorization(state.replace(line, line + "\n" + line)));
    negativeChecks += 3;
  }
  assert(!currentNr602Authorization(state + "\nnr601_completion:\n" + block));
  assert(!currentNr602Authorization(state.replace("nr601_completion:", "missing_completion:")));
  negativeChecks += 2;
}
const ipc = inspectIpcClosure(process.cwd());
assert.equal(ipc.manifest.length - ipc.retryCommands.length, 37);
assert.deepEqual(ipc.sessionManagementCommands, [...NR602_SESSION_COMMANDS].sort());
assert.equal(ipc.sessionCatalogCommands.length, 4);
assert.equal(ipc.allConversationCommands.length - ipc.retryCommands.length, 10);
const result = spawnSync(
  "cargo",
  ["test", "-p", "app-core", "--lib", "--locked", "--offline", "nr602_", "--", "--nocapture"],
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
assert.equal(
  [
    ...output.matchAll(
      /NR602_BRANCH original_undo=true inherited_tasks=0 inherited_changes=0 replay=0 external=0/gu,
    ),
  ].length,
  1,
);
const digest = (file) => createHash("sha256").update(readFileSync(file)).digest("hex");
const report = {
  schemaVersion: 1,
  story: "NR-602",
  result: "pass",
  completedAt: new Date().toISOString(),
  productionCommandCount: ipc.manifest.length,
  sessionManagementCommands: ipc.sessionManagementCommands,
  currentAuthorizationNegativeChecks: negativeChecks,
  rustTestsPassed: tests,
  releaseScopeSha256: releaseScopeSha256(process.cwd(), discoverReleaseSurface(process.cwd())),
  supplementalSourceHashes: [
    "scripts/nr602-session-management-e2e.mjs",
    "scripts/nr-stage-state.mjs",
    "scripts/nr-g1-gate-lib.mjs",
  ].map((file) => ({ file, sha256: digest(file) })),
  realDesktopCommands: true,
  nativeDialogClickThrough: false,
  mockWindow: true,
  realProviderExecuted: false,
  multiUserTestingExecuted: false,
  externalNetworkRequests: 0,
  nrG7Approved: false,
  limitations: [
    "Native file selection is substituted in tests; actual save-dialog accessibility and clean Windows installation remain user-test work.",
    "Independent registries plus a real OS writer lease model cross-process ownership; this is not an independently installed second app click-through.",
    "Visible export preserves text the user explicitly entered, which may itself contain private content; hidden runtime data is excluded, not inferred secret-redaction of user prose.",
    "Full Story quality is separate and remains required.",
  ],
};
mkdirSync("target/qa/nr-602", { recursive: true });
writeFileSync(
  "target/qa/nr-602/regression-validation.json",
  JSON.stringify(report, null, 2) + "\n",
);
console.log(
  "[nr-602] PASS bounded session management, visible export and closed IPC; full quality still required",
);
