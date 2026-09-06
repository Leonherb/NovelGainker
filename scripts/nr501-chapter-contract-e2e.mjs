import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { currentNr5Authorization, historicalNr4Approval, stateSection } from "./nr-stage-state.mjs";
import {
  discoverReleaseSurface,
  inspectIpcClosure,
  releaseScopeSha256,
} from "./nr-g1-gate-lib.mjs";

const root = process.cwd();
const read = (file) => readFileSync(file, "utf8");
const sha = (value) => createHash("sha256").update(value).digest("hex");
const checks = [];
for (const file of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"]) {
  const state = read(file);
  assert(historicalNr4Approval(state), `${file}: historical NR-4 authorization`);
  assert(currentNr5Authorization(state), `${file}: current NR-5 authorization`);
  for (const [section, key, value] of [
    ["current_gate", "nrG5Approved", "true"],
    ["current_gate", "product_approved", "true"],
    ["successor_packet", "product_approved", "true"],
    ["successor_packet", "implementation_started", "true"],
    ["successor_packet", "nr_g6_technical_pass_required", "true"],
    ["successor_packet", "nr_g7_human_approval_required", "true"],
  ]) {
    const block = stateSection(state, section);
    const field = `  ${key}: ${value}`;
    for (const replacement of [
      "",
      `  ${key}: false`,
      `${field}\n${field}`,
      `    ${key}: ${value}`,
    ]) {
      const invalid = state.replace(block, block.replace(field, replacement));
      assert(historicalNr4Approval(invalid), "current negative must preserve historical approval");
      assert(
        !currentNr5Authorization(invalid),
        `${section}.${key}: missing/false/duplicate/nested`,
      );
      checks.push(`${file}:${section}.${key}:${checks.length}`);
    }
  }
  for (const section of ["current_gate", "successor_packet"]) {
    assert(!currentNr5Authorization(`${state}\n${section}:\n${stateSection(state, section)}`));
    assert(!currentNr5Authorization(state.replace(`${section}:`, `${section}_missing:`)));
  }
  const historical = stateSection(state, "nr4_packet");
  const wrongHistory = state.replace(
    historical,
    historical.replace("product_approved: true", "product_approved: false"),
  );
  assert(
    currentNr5Authorization(wrongHistory),
    "historical negative must preserve current approval",
  );
  assert(!historicalNr4Approval(wrongHistory), "current approval cannot repair historical drift");
}
const report = "docs/reviews/novelist-agent-runtime-nr-g5-technical-submission.zh-CN.md";
const reportSha = sha(readFileSync(report));
assert.equal(
  reportSha,
  "6886e961034f33c2cb0947e12765729de12cdaa34c1eb1910a4f726ce7f6d4b8",
  "frozen NR-G5 report",
);
const manifest = "target/qa/nr-406/nr-g5-manifest.json";
if (existsSync(manifest))
  assert.equal(
    sha(readFileSync(manifest)),
    "16abad3cf40a8d93c7860b5b2a4ae827b1f0e84d3d1c834431b45e8d83b66f9b",
    "frozen local NR-G5 submission",
  );
const ipc = inspectIpcClosure(root);
assert.equal(
  ipc.manifest.length - ipc.publicationCommands.length - ipc.sessionCommands.length,
  28,
  "NR-501's unchanged surface; later authorized publication commands are checked separately",
);
const environment = { ...process.env, CARGO_NET_OFFLINE: "true" };
for (const key of Object.keys(environment)) {
  if (/(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu.test(key))
    delete environment[key];
}
const result = spawnSync(
  "cargo",
  ["test", "-p", "app-core", "--lib", "chapter_contract::tests::", "--locked", "--offline"],
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
  throw new Error("NR-501 chapter contract tests failed");
}
const passed = Number(result.stdout.match(/test result: ok\. (\d+) passed/u)?.[1] ?? 0);
assert(passed >= 10, "NR-501 must execute tests, not accept an empty filter");
mkdirSync("target/qa/nr-501", { recursive: true });
writeFileSync(
  "target/qa/nr-501/regression-validation.json",
  JSON.stringify(
    {
      schemaVersion: 1,
      story: "NR-501",
      result: "pass",
      evidenceScope: "chapter-contract-and-current-authorization",
      completedAt: new Date().toISOString(),
      releaseScopeSha256: releaseScopeSha256(root, discoverReleaseSurface(root)),
      frozenNrG5ReportSha256: reportSha,
      currentNrG5Approved: true,
      historicalNrG5SubmissionApproved: false,
      authorizationNegativeChecks: checks.length + 10,
      rustTestsPassed: passed,
      productionCommandCount: ipc.manifest.length,
      chapterProductionExecutorMounted: /CanonChangeCoordinator::new_nr5\(/u.test(
        read("apps/desktop/src-tauri/src/commands_current.rs"),
      ),
      completedNr501SnapshotChapterExecutorMounted: false,
      realProviderExecuted: false,
      limitations: [
        "Pure contracts do not prove model intent; NR-502 supplies durable Host issuance and transactions",
        "No publication runtime or real-user evidence claimed",
      ],
    },
    null,
    2,
  ) + "\n",
);
console.log(
  `[nr-501] PASS ${passed} chapter contract tests; ${checks.length + 10} authorization negatives; historical submission frozen`,
);
