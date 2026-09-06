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
assert.equal(ipc.manifest.length - ipc.publicationCommands.length - ipc.sessionCommands.length, 28);
const packRoot = "resources/skills/novelist/v1";
const pack = JSON.parse(readFileSync(`${packRoot}/pack.json`, "utf8"));
// This script writes regression-validation.json, never the frozen NR-503 quality/validation.
assert.equal(
  pack.projection.contentVersion,
  ipc.publicationCommands.length === 2 ? "1.4.0" : "1.3.0",
);
assert.equal(pack.capabilityManifests.length, 9);
for (const capability of ["create-character", "create-setting", "create-material"])
  assert(readFileSync(`${packRoot}/manifests/${capability}.json`, "utf8").includes('"chapters"'));
const frozen = [
  [
    "target/qa/nr-501/quality.json",
    "84296222606edcf503a0da57bf4a49ff2ac23b0a410ebc14a28cb464ab1db215",
  ],
  [
    "target/qa/nr-502/quality.json",
    "356d8efe7f697fa795eb2845b4dc2bea6b10f1266446763b2942fd03d36595e1",
  ],
  [
    "target/qa/nr-502/validation.json",
    "b1860b662de9252e176e745850977cdd12ab20bae3af0105d235b0e809ea7f4e",
  ],
];
for (const [path, hash] of frozen)
  if (existsSync(path))
    assert.equal(
      createHash("sha256").update(readFileSync(path)).digest("hex"),
      hash,
      `Frozen evidence: ${path}`,
    );
const environment = { ...process.env, CARGO_NET_OFFLINE: "true" };
for (const key of Object.keys(environment))
  if (/(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu.test(key))
    delete environment[key];
const result = spawnSync(
  "cargo",
  ["test", "-p", "app-core", "--lib", "--locked", "--offline", "nr503_", "--", "--nocapture"],
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
  throw new Error("NR-503 focused integration failed");
}
const passed = Number(result.stdout.match(/test result: ok\. (\d+) passed/u)?.[1] ?? 0);
assert(passed >= 8);
const nativeLoops = [
  ...result.stdout.matchAll(
    /NR503_LOOP provider=(Deepseek|Openai) stale=(false|true) final_failure=(false|true) attempts=(\d+) maximum_body=(\d+) external=0/gu,
  ),
].map((row) => ({
  provider: row[1],
  staleSource: row[2] === "true",
  finalReplyFailure: row[3] === "true",
  attempts: Number(row[4]),
  maximumNativeBodyBytes: Number(row[5]),
}));
assert.equal(nativeLoops.length, 8);
assert.equal(
  new Set(
    nativeLoops.map((row) => [row.provider, row.staleSource, row.finalReplyFailure].join(":")),
  ).size,
  8,
);
for (const row of nativeLoops) {
  assert.equal(row.attempts, 3);
  assert(row.maximumNativeBodyBytes > 0 && row.maximumNativeBodyBytes <= 768 * 1024);
}
const dataVariants = [
  ...result.stdout.matchAll(
    /NR503_DATA kind=(characters|world|materials|plot|outlines) manual=(false|true) create=committed update=committed same=no_change undo=undone source=current external=0/gu,
  ),
].map((row) => `${row[1]}:${row[2]}`);
assert.equal(dataVariants.length, 10);
assert.equal(new Set(dataVariants).size, 10);
const report = {
  schemaVersion: 1,
  story: "NR-503",
  result: "pass",
  completedAt: new Date().toISOString(),
  releaseScopeSha256: releaseScopeSha256(root, discoverReleaseSurface(root)),
  evidenceScope: "offline-host-source-binding-transactions-and-native-http",
  rustTestsPassed: passed,
  nativeLoops,
  dataVariants,
  productionCommandCount: ipc.manifest.length,
  realProviderExecuted: false,
  multiUserTestingExecuted: false,
  fullStoryQualityRequired: true,
  limitations: [
    "Six-dimension structured report validation checks actual delivered receipts, exact quotes, lines, hashes and scope; it does not validate model semantics or parse arbitrary assistant prose.",
    "Report methods and actual read metadata are available to the model; live model compliance and interpretation require NR-6 multi-user evidence.",
    "Bounded reads never imply whole-book conflict-free coverage. Read-only reporting does not itself authorize Canon mutations.",
    "Synthetic native HTTP executes actual Host and disk effects; it is not a live Provider test or a native GUI click-through.",
    "No NR-G6/NR-G7 approval or publication runtime is claimed.",
  ],
};
mkdirSync("target/qa/nr-503", { recursive: true });
writeFileSync(
  "target/qa/nr-503/regression-validation.json",
  JSON.stringify(report, null, 2) + "\n",
);
console.log(
  `[nr-503] PASS ${passed} directed tests, ${nativeLoops.length} native HTTP loops and ${dataVariants.length} five-type/source variants; full quality still required`,
);
