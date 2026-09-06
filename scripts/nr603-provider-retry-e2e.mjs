import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { currentNr603Authorization, stateSection } from "./nr-stage-state.mjs";
import {
  inspectIpcClosure,
  discoverReleaseSurface,
  releaseScopeSha256,
  NR603_RETRY_COMMANDS,
} from "./nr-g1-gate-lib.mjs";
let negativeChecks = 0;
for (const file of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"]) {
  const state = readFileSync(file, "utf8");
  assert(currentNr603Authorization(state));
  const block = stateSection(state, "nr602_completion");
  for (const line of block
    .trimEnd()
    .split(/\r?\n/u)
    .filter((line) => line.trim())) {
    assert(!currentNr603Authorization(state.replace(line, line.replace(/: .*/u, ': "invalid"'))));
    assert(!currentNr603Authorization(state.replace(line, "")));
    assert(!currentNr603Authorization(state.replace(line, line + "\n" + line)));
    negativeChecks += 3;
  }
  assert(!currentNr603Authorization(state + "\nnr602_completion:\n" + block));
  assert(!currentNr603Authorization(state.replace("nr602_completion:", "missing_completion:")));
  negativeChecks += 2;
}
const ipc = inspectIpcClosure(process.cwd());
assert.equal(ipc.manifest.length, 38);
assert.deepEqual(ipc.retryCommands, NR603_RETRY_COMMANDS);
assert.equal(ipc.allConversationCommands.length, 11);
const result = spawnSync(
  "cargo",
  ["test", "-p", "app-core", "--lib", "--locked", "--offline", "nr603_", "--", "--nocapture"],
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
assert(tests >= 9);
for (const provider of ["Deepseek", "Openai"])
  assert.equal(
    [
      ...output.matchAll(
        new RegExp(
          `NR603_RETRY provider=${provider} requests=2 users=1 duplicate=0 external=0`,
          "gu",
        ),
      ),
    ].length,
    1,
  );
const report = {
  schemaVersion: 1,
  story: "NR-603",
  result: "pass",
  completedAt: new Date().toISOString(),
  productionCommandCount: ipc.manifest.length,
  retryCommands: ipc.retryCommands,
  currentAuthorizationNegativeChecks: negativeChecks,
  rustTestsPassed: tests,
  releaseScopeSha256: releaseScopeSha256(process.cwd(), discoverReleaseSurface(process.cwd())),
  supplementalSourceHashes: [
    "scripts/nr603-provider-retry-e2e.mjs",
    "scripts/nr-stage-state.mjs",
    "scripts/nr-g1-gate-lib.mjs",
  ].map((file) => ({
    file,
    sha256: createHash("sha256").update(readFileSync(file)).digest("hex"),
  })),
  explicitRetry: true,
  automaticProviderRetry: false,
  persistedProviderContinuation: false,
  nativeServerCursor: { deepseek: "unsupported_unverified", openai: "unsupported_unverified" },
  realDesktopCommands: true,
  mockWindow: true,
  nativeUiClickThrough: false,
  realProviderExecuted: false,
  multiUserTestingExecuted: false,
  externalNetworkRequests: 0,
  nrG7Approved: false,
  limitations: [
    "Loopback protocol evidence is not real Provider compatibility or semantic evaluation.",
    "Full Story quality remains separately required.",
  ],
};
mkdirSync("target/qa/nr-603", { recursive: true });
writeFileSync(
  "target/qa/nr-603/regression-validation.json",
  JSON.stringify(report, null, 2) + "\n",
);
console.log(
  "[nr-603] PASS explicit bounded retry and attempt-scoped reconciliation; full quality still required",
);
