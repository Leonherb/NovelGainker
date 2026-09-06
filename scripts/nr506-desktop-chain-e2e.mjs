import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { currentNr5Authorization } from "./nr-stage-state.mjs";
import {
  discoverReleaseSurface,
  inspectIpcClosure,
  releaseScopeSha256,
} from "./nr-g1-gate-lib.mjs";

const digest = (file) => createHash("sha256").update(readFileSync(file)).digest("hex");
for (const file of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"])
  assert(currentNr5Authorization(readFileSync(file, "utf8")));
const ipc = inspectIpcClosure(process.cwd());
assert.equal(ipc.manifest.length - ipc.sessionCommands.length, 30);
const frozen = [
  [
    "target/qa/nr-505/quality.json",
    "856b2d077986d10f7b1e1fda04c59f43f3cd6ba28a58a1a4b62d47ee384fdd1a",
  ],
  [
    "target/qa/nr-505/validation.json",
    "0ecf2048929a14871377da3b467ec553a50aebdabec66eb1ec4198cb61b73ea2",
  ],
  [
    "target/qa/nr-505/completed-structure-fixture.epub",
    "63151feddff7e41c72fd0a9d0d55a419bac398d638f65e5af09cccb8e61df8df",
  ],
];
for (const [file, hash] of frozen) assert.equal(digest(file), hash, file);
const result = spawnSync(
  "cargo",
  ["test", "-p", "app-core", "--lib", "--locked", "--offline", "nr506_", "--", "--nocapture"],
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
assert(tests >= 5);
const chains = [
  ...output.matchAll(
    /NR506_DESKTOP provider=(Deepseek|Openai) chapters=12 publication=1 reopened=true undo_stale=true real_commands=true mock_window=true external=0/gu,
  ),
].map((row) => row[1]);
const faults = [
  ...output.matchAll(
    /NR506_FAULT provider=(Deepseek|Openai) zero_text=true partial_text=true invalid_tool=true pending_discard=true committed_after_failure=true external=0/gu,
  ),
].map((row) => row[1]);
assert.deepEqual(chains.sort(), ["Deepseek", "Openai"]);
assert.deepEqual(faults.sort(), ["Deepseek", "Openai"]);
const supplement = [
  "Cargo.lock",
  "crates/app-core/Cargo.toml",
  "crates/app-core/build.rs",
  "crates/app-core/tests/windows-test.manifest",
  "scripts/nr506-desktop-chain-e2e.mjs",
];
const report = {
  schemaVersion: 1,
  story: "NR-506",
  result: "pass",
  completedAt: new Date().toISOString(),
  releaseScopeSha256: releaseScopeSha256(process.cwd(), discoverReleaseSurface(process.cwd())),
  supplementalSourceHashes: supplement.map((file) => ({ file, sha256: digest(file) })),
  rustTestsPassed: tests,
  productionCommandCount: ipc.manifest.length,
  twelveChapterDesktopChains: chains,
  desktopFaultMatrices: faults,
  productionCommandSourceIncluded: true,
  productionWorkerAndIpcExecuted: true,
  mockWindow: true,
  syntheticVault: true,
  syntheticLoopback: true,
  realProviderExecuted: false,
  externalNetworkRequests: 0,
  multiUserTestingExecuted: false,
  nrG6Approved: false,
  nrG7Approved: false,
  fullStoryQualityRequired: true,
  preservedFrozenEvidence: frozen.map(([file, sha256]) => ({ file, sha256 })),
  limitations: [
    "Real desktop command handlers, worker, local stores and native HTTP are executed through Tauri MockRuntime; this is not a native WebView click-through.",
    "Synthetic model responses do not prove semantic authorization, real Provider compatibility or literary quality.",
    "Maximum native publication chains and independent ZIP/XML structure checks remain separate NR-505 regressions in the same full-quality run.",
    "Real multi-user testing, reader certification and build distribution are not performed.",
  ],
};
mkdirSync("target/qa/nr-506", { recursive: true });
writeFileSync(
  "target/qa/nr-506/regression-validation.json",
  JSON.stringify(report, null, 2) + "\n",
);
console.log(
  "[nr-506] PASS actual desktop IPC twelve-chapter and fault chains; full quality still required",
);
