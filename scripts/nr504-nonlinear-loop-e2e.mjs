import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { currentNr5Authorization } from "./nr-stage-state.mjs";
import {
  discoverReleaseSurface,
  inspectIpcClosure,
  releaseScopeSha256,
} from "./nr-g1-gate-lib.mjs";

const root = process.cwd();
for (const file of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"])
  assert(currentNr5Authorization(readFileSync(file, "utf8")));
const ipc = inspectIpcClosure(root);
assert.equal(
  ipc.manifest.length - ipc.publicationCommands.length - ipc.sessionCommands.length,
  28,
  "Frozen NR-504 surface excludes later authorized publication commands",
);
for (const [path, hash] of [
  [
    "target/qa/nr-503/quality.json",
    "af4ece90d838542481f508b2688020f5df2091c3a0d9f4586c483f153c66a2c0",
  ],
  [
    "target/qa/nr-503/validation.json",
    "752d9f55bc390cf91cca14b2039254f5e5cad87c616c5c412b7da8506da5918f",
  ],
])
  if (existsSync(path))
    assert.equal(createHash("sha256").update(readFileSync(path)).digest("hex"), hash);
const environment = { ...process.env, CARGO_NET_OFFLINE: "true" };
for (const key of Object.keys(environment))
  if (/(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu.test(key))
    delete environment[key];
const result = spawnSync(
  "cargo",
  ["test", "-p", "app-core", "--lib", "--locked", "--offline", "nr504_", "--", "--nocapture"],
  {
    cwd: root,
    env: environment,
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  },
);
if (result.error || result.status !== 0) {
  process.stdout.write(result.stdout ?? "");
  process.stderr.write(result.stderr ?? "");
  throw new Error("NR-504 integration failed");
}
const passed = Number(result.stdout.match(/test result: ok\. (\d+) passed/u)?.[1] ?? 0);
assert(passed >= 5);
const loops = [
  ...result.stdout.matchAll(
    /NR504_LOOP provider=(Deepseek|Openai) chapter_first=(false|true) mode=(\d+) maximum=(false|true) agent=(\d+) compactor=(\d+) requested=(\d+) maximum_body=(\d+) publication_fixture=(false|true) external=0/gu,
  ),
].map((row) => ({
  provider: row[1],
  chapterFirst: row[2] === "true",
  mode: Number(row[3]),
  maximum: row[4] === "true",
  agent: Number(row[5]),
  compactor: Number(row[6]),
  requested: Number(row[7]),
  maximumNativeBodyBytes: Number(row[8]),
  publicationFixture: row[9] === "true",
}));
assert.equal(loops.length, 18);
assert.equal(
  new Set(loops.map((row) => [row.provider, row.chapterFirst, row.mode, row.maximum].join(":")))
    .size,
  18,
);
assert.equal(loops.filter((row) => row.maximum).length, 2);
for (const row of loops) {
  assert(row.maximumNativeBodyBytes > 0 && row.maximumNativeBodyBytes <= 768 * 1024);
  assert.equal(row.agent, row.maximum ? 8 : 5);
  assert.equal(row.compactor, row.maximum ? 1 : 0);
  assert.equal(row.requested, row.maximum ? 33792 : 20480);
  assert.equal(row.publicationFixture, row.maximum);
  assert(row.mode >= 0 && row.mode <= 3);
}
const reserveCases = [
  ...result.stdout.matchAll(
    /NR504_RESERVE provider=(Deepseek|Openai) oversized=(false|true) exact=(\d+) reserved=(\d+) executed=(\d+) external=0/gu,
  ),
].map((row) => ({
  provider: row[1],
  oversized: row[2] === "true",
  exactNativeBytes: Number(row[3]),
  reservedNativeBytes: Number(row[4]),
  actualExecutions: Number(row[5]),
}));
assert.equal(reserveCases.length, 4);
assert.equal(new Set(reserveCases.map((row) => [row.provider, row.oversized].join(":"))).size, 4);
for (const row of reserveCases) {
  assert(row.exactNativeBytes <= 768 * 1024, "The unreserved request alone fits");
  assert.equal(row.reservedNativeBytes, row.exactNativeBytes + 2 * 64 * 1024 + 2048);
  assert.equal(row.reservedNativeBytes > 768 * 1024, row.oversized);
  assert.equal(row.actualExecutions, row.oversized ? 0 : 1);
}
const report = {
  schemaVersion: 1,
  story: "NR-504",
  result: "pass",
  completedAt: new Date().toISOString(),
  releaseScopeSha256: releaseScopeSha256(root, discoverReleaseSurface(root)),
  rustTestsPassed: passed,
  productionCommandCount: ipc.manifest.length,
  loops,
  reserveCases,
  loopModeLegend: [
    "both_committed",
    "first_pending_second_committed",
    "first_failed_second_committed",
    "both_committed_partial_final_stream",
  ],
  limits: {
    agent: 8,
    compactor: 1,
    requestedOutputTokens: 33792,
    reads: 4,
    mutations: 2,
    publication: 1,
    tools: 7,
    argumentBytes: 65536,
    resultBytes: 65536,
    nativeContinuationBytes: 524288,
    requestBytes: 786432,
  },
  realProviderExecuted: false,
  multiUserTestingExecuted: false,
  publicationProductionMounted: ipc.publicationCommands.length === 2,
  publicationExecutedByThisRegression: false,
  fullStoryQualityRequired: true,
  limitations: [
    "16 native loopback chains use the production entry and real read/write/source/disk/Task paths. Synthetic model outputs do not prove semantic correctness.",
    "These two historical budget loops deliberately use a cfg(test)-only publication stand-in. They generate no EPUB; the separate NR-505 tests exercise the actual production publication executor. Frozen NR-504 validation and quality files remain unchanged.",
    "Reserve cases use synthetic escaped history through actual native serialization and the same production preflight, followed by actual Task/executor/disk checks; these are not additional captured HTTP exchanges.",
    "No live Provider, native GUI end-to-end click-through, user test or NR-G6/NR-G7 approval is claimed.",
  ],
};
mkdirSync("target/qa/nr-504", { recursive: true });
writeFileSync(
  "target/qa/nr-504/regression-validation.json",
  JSON.stringify(report, null, 2) + "\n",
);
console.log(
  `[nr-504] PASS ${passed} directed tests, ${loops.length} native loops and ${reserveCases.length} exact-body reserve cases; full quality still required`,
);
