import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { currentNr5Authorization } from "./nr-stage-state.mjs";
import {
  discoverReleaseSurface,
  inspectIpcClosure,
  releaseScopeSha256,
} from "./nr-g1-gate-lib.mjs";

const root = process.cwd();
const read = (path) => readFileSync(path, "utf8");
for (const file of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"])
  assert(currentNr5Authorization(read(file)), "Current continuous NR-5 authorization required");
const production = read("apps/desktop/src-tauri/src/commands_current.rs");
assert(
  /CanonChangeCoordinator::new_nr5\(/u.test(production),
  "Production chapter coordinator must be mounted",
);
assert(
  /runtime_view_for\(/u.test(production),
  "Startup reconciliation must use current Host capacity",
);
const ipc = inspectIpcClosure(root);
assert.equal(
  ipc.manifest.length - ipc.publicationCommands.length - ipc.sessionCommands.length,
  28,
  "No new chapter IPC; later publication commands have a separate closed authorization check",
);
const environment = { ...process.env, CARGO_NET_OFFLINE: "true" };
for (const key of Object.keys(environment))
  if (/(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu.test(key))
    delete environment[key];
const result = spawnSync(
  "cargo",
  [
    "test",
    "-p",
    "app-core",
    "--lib",
    "--locked",
    "--offline",
    "nr502_",
    "--",
    "--skip",
    "five_hundred_real",
    "--nocapture",
  ],
  {
    cwd: root,
    env: environment,
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  },
);
if (result.error || result.status !== 0) {
  process.stderr.write(result.stderr ?? "");
  process.stdout.write(result.stdout ?? "");
  throw new Error("NR-502 focused integration failed");
}
const passed = Number(result.stdout.match(/test result: ok\. (\d+) passed/u)?.[1] ?? 0);
assert(passed >= 16, "Nonempty complete directed test suite required");
const nativeLoops = [
  ...result.stdout.matchAll(
    /NR502_LOOP provider=(Deepseek|Openai) existing=(false|true) final_failure=(false|true) attempts=(\d+) maximum_body=(\d+) cap=(\d+) external=0/gu,
  ),
].map((match) => ({
  provider: match[1],
  update: match[2] === "true",
  replyFailedAfterCommit: match[3] === "true",
  attempts: Number(match[4]),
  maximumNativeBodyBytes: Number(match[5]),
  cap: Number(match[6]),
}));
assert.equal(nativeLoops.length, 8, "Both native providers, create/update, final success/failure");
assert.equal(
  new Set(
    nativeLoops.map((row) => [row.provider, row.update, row.replyFailedAfterCommit].join(":")),
  ).size,
  8,
);
for (const row of nativeLoops) {
  assert.equal(row.cap, 768 * 1024);
  assert(row.maximumNativeBodyBytes > 48 * 1024 && row.maximumNativeBodyBytes <= row.cap);
  assert.equal(row.attempts, row.update ? 4 : 3);
}
const report = {
  schemaVersion: 1,
  story: "NR-502",
  result: "pass",
  evidenceScope: "offline-focused-host-native-http-and-storage-integration",
  completedAt: new Date().toISOString(),
  releaseScopeSha256: releaseScopeSha256(root, discoverReleaseSurface(root)),
  rustTestsPassed: passed,
  nativeLoops,
  productionCommandCount: ipc.manifest.length,
  chapterProductionExecutorMounted: true,
  realProviderExecuted: false,
  multiUserTestingExecuted: false,
  fullStoryQualityRequired: true,
  scaleVerification: {
    test: "nr502_five_hundred_real_commits_exceed_legacy_record_limit_without_dropping_undo",
    requiredRunner: "pnpm quality -> Rust tests -> cargo test --workspace --locked --offline",
    executedByThisFocusedScript: false,
  },
  limitations: [
    "Synthetic loopback HTTP executes the actual Host/runtime/disk path, not live model semantic intent.",
    "React behavior is verified by the full quality React step; this is not a native GUI click-through.",
    "The separate real 500-commit test is mandatory in full quality, not substituted by synthetic 2048-record metadata fixtures.",
    "No NR-G6/NR-G7 approval, publication runtime or multi-user evidence is claimed.",
  ],
};
mkdirSync("target/qa/nr-502", { recursive: true });
writeFileSync(
  "target/qa/nr-502/regression-validation.json",
  JSON.stringify(report, null, 2) + "\n",
);
console.log(
  `[nr-502] PASS ${passed} directed tests and ${nativeLoops.length} native HTTP loops; full Story quality still required`,
);
