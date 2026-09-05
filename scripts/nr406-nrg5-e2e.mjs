import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  discoverReleaseSurface,
  inspectIpcClosure,
  hasApprovedNr4Ipc,
  releaseScopeSha256,
} from "./nr-g1-gate-lib.mjs";

const root = process.cwd();
const read = (relative) => readFileSync(path.join(root, relative), "utf8");
const sha = (value) => createHash("sha256").update(value).digest("hex");
const out = path.join(root, "target/qa/nr-406");
mkdirSync(out, { recursive: true });
const source = read("crates/app-core/src/canon_change.rs");
const number = (name) => {
  const expression = source
    .match(new RegExp(`pub const ${name}: (?:usize|u16|u32|u64) = ([^;]+);`, "u"))?.[1]
    .replaceAll("_", "")
    .replaceAll(/\s/gu, "");
  if (!expression || !/^\d+(?:\*\d+)*$/u.test(expression))
    throw new Error(`Missing constant ${name}`);
  return expression.split("*").reduce((total, part) => total * Number(part), 1);
};
const ipc = inspectIpcClosure(root);
if (ipc.manifest.length !== 28 || ipc.nr4Commands.length !== 4)
  throw new Error("NR-406 IPC closure drift");
for (const relative of ["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"]) {
  const state = read(relative);
  if (
    !hasApprovedNr4Ipc(state) ||
    !state.includes("nrG4Approved: true") ||
    !state.includes("nrG5Approved: false") ||
    state.includes("real_provider_live_smoke_executed: true")
  ) {
    throw new Error(`NR-406 approval boundary drift: ${relative}`);
  }
}
const pack = JSON.parse(read("resources/skills/novelist/v1/pack.json"));
const inventory = JSON.parse(read("resources/skills/novelist/v1/inventory.json"));
const claims = {
  status: "NR-G5-technical-submission",
  nrG5Approved: false,
  nrG4Approved: true,
  productionCommandCount: ipc.manifest.length,
  conversationCommandCount: ipc.conversation.length,
  productionTools: ["novelist.read_context", "novelist.propose_change"],
  capabilityCount: pack.capabilityManifests.length,
  resourceCount: inventory.fileCount,
  projectionVersion: pack.projection.contentVersion,
  canonDirectories: ["characters", "world", "materials", "plot", "outlines"],
  maximumAgentAttempts: number("MAX_NR4_AGENT_ATTEMPTS"),
  maximumCompactorAttempts: 1,
  maximumReadRounds: number("MAX_NR4_READ_ROUNDS"),
  maximumMutationRounds: number("MAX_NR4_MUTATION_ROUNDS"),
  cumulativeRequestedOutputTokens: number("NR4_CUMULATIVE_OUTPUT_TOKENS"),
  maximumProviderBodyBytes: number("NR4_REQUEST_BYTES"),
  maximumContinuationBytes: number("NR4_CONTINUATION_BYTES"),
  maximumTargets: number("MAX_CHANGE_TARGETS"),
  maximumCandidateBytes: number("MAX_CHANGE_BODY_BYTES"),
  changeStoreBytes: number("MAX_CHANGE_STORE_BYTES"),
  maximumChangeRecords: number("MAX_CHANGE_RECORDS"),
  maximumPendingChanges: number("MAX_PENDING_CHANGES"),
  independentChapterSourceImplemented: false,
  realProviderLiveSmokeExecuted: false,
  providerSemanticQualityValidated: false,
  automatedExternalRequests: 0,
  liveSmokeDecision: "explicitly_waived_risk_accepted",
};
if (
  claims.capabilityCount !== 9 ||
  claims.resourceCount !== 37 ||
  claims.maximumAgentAttempts !== 4 ||
  claims.cumulativeRequestedOutputTokens !== 17408 ||
  claims.maximumProviderBodyBytes !== 262144
)
  throw new Error("NR-406 approved profile drift");
const releaseHash = releaseScopeSha256(root, discoverReleaseSurface(root));
const reportPath = "docs/reviews/novelist-agent-runtime-nr-g5-technical-submission.zh-CN.md";
const checkClaims = (actual) => {
  if (JSON.stringify(actual) !== JSON.stringify(claims))
    throw new Error("NR-G5 report machine claims drift");
};
if (existsSync(path.join(root, reportPath))) {
  const match = read(reportPath).match(
    /<!-- NR-G5-MANIFEST-CLAIMS:START -->\s*```json\s*([\s\S]*?)\s*```\s*<!-- NR-G5-MANIFEST-CLAIMS:END -->/u,
  );
  if (!match) throw new Error("NR-G5 report machine claims missing");
  checkClaims(JSON.parse(match[1]));
}
let negativeRejected = false;
try {
  checkClaims({ ...claims, nrG5Approved: true });
} catch {
  negativeRejected = true;
}
if (!negativeRejected) throw new Error("NR-G5 approval drift negative fixture accepted");

if (process.argv.includes("--submit")) {
  const validation = JSON.parse(read("target/qa/nr-406/validation.json"));
  const quality = JSON.parse(read("target/qa/nr-406/quality.json"));
  if (
    quality.result !== "pass" ||
    quality.releaseScopeSha256 !== releaseHash ||
    validation.releaseScopeSha256 !== releaseHash ||
    quality.startedAt > validation.completedAt ||
    quality.completedAt < validation.completedAt ||
    !quality.steps.every((step) => step.exitCode === 0)
  ) {
    throw new Error("NR-G5 requires a current complete quality run containing NR-406 validation");
  }
  checkClaims(validation.claims);
  const manifest = {
    ...validation,
    evidenceScope: "technical-submission",
    quality,
    qualitySha256: sha(read("target/qa/nr-406/quality.json")),
    nrG5Approved: false,
  };
  manifest.payloadSha256 = sha(JSON.stringify(manifest));
  writeFileSync(path.join(out, "nr-g5-manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  console.log("[nr-406] NR-G5 technical submission ready; product approval false");
} else {
  const environment = { ...process.env, CARGO_NET_OFFLINE: "true" };
  for (const key of Object.keys(environment)) {
    if (/(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu.test(key))
      delete environment[key];
  }
  const runs = [];
  let measured = "";
  for (const filter of [
    "canon_change",
    "conversation_runtime::tests::nr403::",
    "provider_core::tests::nr403",
    "canon_apply::tests",
    "recovery::tests",
    "coordination::tests",
    "task_store::tests",
  ]) {
    const args = [
      "test",
      "-p",
      "app-core",
      "--lib",
      filter,
      "--locked",
      "--offline",
      "--",
      "--nocapture",
    ];
    const result = spawnSync("cargo", args, {
      cwd: root,
      env: environment,
      encoding: "utf8",
      maxBuffer: 8 * 1024 * 1024,
    });
    if (result.error || result.status !== 0) {
      process.stderr.write(result.stderr ?? "");
      process.stderr.write(result.stdout ?? "");
      throw new Error(`NR-406 failed ${filter}`);
    }
    const counts = [
      ...result.stdout.matchAll(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored/gu),
    ];
    const passed = counts.reduce((total, match) => total + Number(match[1]), 0);
    if (passed === 0) throw new Error(`NR-406 zero executed tests: ${filter}`);
    runs.push({ command: "cargo", args, exitCode: result.status, passed });
    measured +=
      result.stdout
        .split("\n")
        .filter((line) => line.startsWith("NR406_"))
        .join("\n") + "\n";
  }
  const loops = [
    ...measured.matchAll(
      /NR406_LOOP provider=(Deepseek|Openai) attempts=(\d+) agent=(\d+) compactor=(\d+) requested=(\d+) maximum_body=(\d+) cap=(\d+) external=0/gu,
    ),
  ].map((match) => ({
    provider: match[1],
    attempts: +match[2],
    agent: +match[3],
    compactor: +match[4],
    requested: +match[5],
    maximumBody: +match[6],
    cap: +match[7],
  }));
  for (const provider of ["Deepseek", "Openai"]) {
    if (
      !loops.some(
        (loop) =>
          loop.provider === provider &&
          loop.attempts === 5 &&
          loop.agent === 4 &&
          loop.compactor === 1 &&
          loop.requested === 17408 &&
          loop.maximumBody <= loop.cap &&
          loop.cap === 262144,
      )
    ) {
      throw new Error(`NR-406 missing actual maximum chain for ${provider}`);
    }
  }
  for (const directory of claims.canonDirectories) {
    if (
      !measured.includes(
        `NR406_DIRECTORY kind=${directory} create=committed update=committed same=no_change undo=undone external=0`,
      )
    ) {
      throw new Error(`NR-406 missing actual directory round trip ${directory}`);
    }
  }
  const validation = {
    schemaVersion: 1,
    gateId: "NR-G5",
    evidenceScope: "validation-not-gate-approval",
    result: "pass",
    completedAt: new Date().toISOString(),
    claims,
    runs,
    loops,
    packSha256: inventory.packSha256,
    releaseScopeSha256: releaseHash,
    negativeReportDriftRejected: true,
    limitations: [
      "Scripted providers, not real model intent/compatibility validation",
      "Composite interruption tests include durable-state reconstruction; lower kernel also uses terminated subprocesses",
      "Independent chapter source deferred to NR-5; no NR-5 implementation",
    ],
  };
  writeFileSync(path.join(out, "validation.json"), JSON.stringify(validation, null, 2) + "\n");
  writeFileSync(path.join(out, "nr-g5-claims.json"), JSON.stringify(claims, null, 2) + "\n");
  console.log(
    `[nr-406] PASS ${runs.reduce((n, run) => n + run.passed, 0)} focused checks / five directories / two actual 4+1 chains / Gate approval false`,
  );
}
