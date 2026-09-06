import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { currentNr5Authorization, stateSection } from "./nr-stage-state.mjs";
import {
  discoverReleaseSurface,
  hasApprovedPublicationIpc,
  inspectIpcClosure,
  releaseScopeSha256,
} from "./nr-g1-gate-lib.mjs";

const root = process.cwd();
const digest = (file) => createHash("sha256").update(readFileSync(file)).digest("hex");
for (const file of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"]) {
  const status = readFileSync(file, "utf8");
  assert(currentNr5Authorization(status));
  assert(hasApprovedPublicationIpc(status));
  for (const invalid of [
    "",
    status.replace(
      'implementation_release_status: "released_continuous_with_quality_gates"',
      'implementation_release_status: "pending"',
    ),
    status.replace(
      stateSection(status, "current_gate"),
      stateSection(status, "current_gate").replace(
        "  product_approved: true",
        "  product_approved: false",
      ),
    ),
    status + "\nsuccessor_packet:\n  product_approved: true\n",
    status.replace("current_gate:\n", "current_gate:\n  product_approved: true\n"),
    status.replace("current_gate:\r\n", "current_gate:\r\n  product_approved: true\r\n"),
  ].filter((value) => value !== status)) {
    assert(
      !hasApprovedPublicationIpc(invalid),
      "Missing, duplicate or false current authority fails closed",
    );
  }
}
const ipc = inspectIpcClosure(root);
assert.equal(ipc.manifest.length - ipc.sessionCommands.length, 30);
assert.deepEqual(ipc.publicationCommands, ["publication_detail", "publication_list"]);
const frozen = [
  [
    "target/qa/nr-504/quality.json",
    "2d906ac8b22987e5569bc57b18a4afea4627f6ed2d188b22b8cc48dd3d2f4f12",
  ],
  [
    "target/qa/nr-504/validation.json",
    "87d5d82abc25085d3a13744ca7ecec091181546a5ab8b63e201fc33dd9940762",
  ],
  [
    "docs/reviews/novelist-agent-runtime-nr-g5-technical-submission.zh-CN.md",
    "6886e961034f33c2cb0947e12765729de12cdaa34c1eb1910a4f726ce7f6d4b8",
  ],
  [
    "target/qa/nr-406/nr-g5-manifest.json",
    "16abad3cf40a8d93c7860b5b2a4ae827b1f0e84d3d1c834431b45e8d83b66f9b",
  ],
];
for (const [file, sha256] of frozen) assert.equal(digest(file), sha256, file);
const inventory = JSON.parse(readFileSync("resources/skills/novelist/v1/inventory.json", "utf8"));
const pack = JSON.parse(readFileSync("resources/skills/novelist/v1/pack.json", "utf8"));
assert.equal(pack.projection.contentVersion, "1.4.0");
assert.equal(pack.capabilityManifests.length, 9);
assert.equal(inventory.fileCount, 37);
assert.equal(
  inventory.packSha256,
  "319be9d883968cfd225974c20eb890f79bd5a836c9996c8f699f7aa0658e0d2e",
);
const environment = { ...process.env, CARGO_NET_OFFLINE: "true" };
for (const key of Object.keys(environment))
  if (/(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu.test(key))
    delete environment[key];
function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    env: environment,
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  });
  if (result.error || result.status !== 0) {
    process.stdout.write(result.stdout ?? "");
    process.stderr.write(result.stderr ?? "");
    throw new Error("NR-505 directed verification failed");
  }
  return result.stdout;
}
const output = run("cargo", [
  "test",
  "-p",
  "app-core",
  "--lib",
  "--locked",
  "--offline",
  "nr505",
  "--",
  "--nocapture",
]);
const passed = Number(output.match(/test result: ok\. (\d+) passed/u)?.[1] ?? 0);
assert(passed >= 15);
const loops = [
  ...output.matchAll(
    /NR505_LOOP provider=(Deepseek|Openai) mode=(\d+) agent=(\d+) compactor=(\d+) requested=(\d+) maximum_body=(\d+) publication_fixture=false external=0/gu,
  ),
].map((row) => ({
  provider: row[1],
  mode: Number(row[2]),
  agent: Number(row[3]),
  compactor: Number(row[4]),
  requested: Number(row[5]),
  maximumNativeBodyBytes: Number(row[6]),
}));
assert.equal(loops.length, 8);
assert.equal(new Set(loops.map((row) => [row.provider, row.mode].join(":"))).size, 8);
for (const row of loops) {
  assert(row.mode >= 0 && row.mode <= 3);
  assert.equal(row.agent, 5);
  assert.equal(row.compactor, 0);
  assert.equal(row.requested, 20480);
}
const maximumLoops = [
  ...output.matchAll(
    /NR505_MAX provider=(Deepseek|Openai) chapter_first=true mode=0 maximum=true agent=(\d+) compactor=(\d+) requested=(\d+) maximum_body=(\d+) publication_fixture=false external=0/gu,
  ),
].map((row) => ({
  provider: row[1],
  agent: Number(row[2]),
  compactor: Number(row[3]),
  requested: Number(row[4]),
  maximumNativeBodyBytes: Number(row[5]),
}));
assert.equal(maximumLoops.length, 2);
assert.equal(new Set(maximumLoops.map((row) => row.provider)).size, 2);
for (const row of maximumLoops) {
  assert.equal(row.agent, 8);
  assert.equal(row.compactor, 1);
  assert.equal(row.requested, 33792);
}
for (const row of [...loops, ...maximumLoops])
  assert(row.maximumNativeBodyBytes > 0 && row.maximumNativeBodyBytes <= 768 * 1024);
const structure = JSON.parse(
  run("powershell", [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    "scripts/nr505-epub-structure-check.ps1",
  ]),
);
assert.equal(structure.result, "pass");
assert.equal(structure.validator, "independent-dotnet-zip-xml");
assert.equal(structure.completeEpubCertification, false);
assert.equal(structure.entries, 7);
assert.equal(structure.chapters, 2);
assert.equal(structure.artifactSha256, digest("target/qa/nr-505/epub-structure-fixture.epub"));
const report = {
  schemaVersion: 1,
  story: "NR-505",
  result: "pass",
  completedAt: new Date().toISOString(),
  releaseScopeSha256: releaseScopeSha256(root, discoverReleaseSurface(root)),
  rustTestsPassed: passed,
  productionCommandCount: ipc.manifest.length,
  publicationCommands: ipc.publicationCommands,
  packVersion: pack.projection.contentVersion,
  packSha256: inventory.packSha256,
  preservedFrozenEvidence: frozen.map(([file, sha256]) => ({ file, sha256 })),
  loops,
  maximumLoops,
  independentStructure: structure,
  loopModeLegend: [
    "generated",
    "missing_metadata_no_artifact",
    "generated_then_final_503",
    "generated_then_partial_stream",
  ],
  publicationProductionMounted: true,
  productionPublicationExecutorUsed: true,
  realProviderExecuted: false,
  externalNetworkRequests: 0,
  multiUserTestingExecuted: false,
  completeEpubCertification: false,
  fullStoryQualityRequired: true,
  limitations: [
    "Native loopback protocols execute actual method reads, chapter commits, publication, Task and final-response paths; synthetic responses do not prove model semantic correctness.",
    "Independent ZIP/XML structure checks are not complete EPUB certification or a reader compatibility test. PNG/JPEG checks are structural, not full pixel decoding.",
    "The 500-chapter container test is a builder boundary test, not native GUI or long-form reader qualification.",
    "No live credentials, external network, build distribution, native GUI click-through or user/Gate approval is claimed.",
  ],
};
mkdirSync("target/qa/nr-505", { recursive: true });
writeFileSync(
  "target/qa/nr-505/regression-validation.json",
  JSON.stringify(report, null, 2) + "\n",
);
console.log(
  "[nr-505] PASS " +
    passed +
    " directed tests, 10 production publication loopback chains and independent ZIP/XML checks; full quality still required",
);
