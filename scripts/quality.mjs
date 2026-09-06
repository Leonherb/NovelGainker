import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { discoverReleaseSurface, releaseScopeSha256 } from "./nr-g1-gate-lib.mjs";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) {
  console.error("[quality] pnpm must run this script so npm_execpath is available.");
  process.exit(1);
}

const offlineEnvironment = {
  ...process.env,
  CARGO_NET_OFFLINE: "true",
  NOVELGAINKER_QUALITY_STARTED_AT: new Date().toISOString(),
};

const SECRET_ENVIRONMENT_NAME =
  /(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu;
for (const name of Object.keys(offlineEnvironment)) {
  if (SECRET_ENVIRONMENT_NAME.test(name)) {
    delete offlineEnvironment[name];
  }
}
const steps = [
  ["Frontend format check", ["format:check"]],
  ["Frontend lint", ["lint"]],
  ["TypeScript type check", ["typecheck"]],
  ["React unit tests", ["test:frontend"]],
  ["Workspace boundary checks", ["quality:boundaries"]],
  ["NR-001 retirement and Canon baseline", ["e2e:nr001"]],
  ["NR-002 clean slate cutover", ["e2e:nr002"]],
  ["NR-003 conversation store", ["e2e:nr003"]],
  ["NR-004 Provider Conversation", ["e2e:nr004"]],
  ["NR-005 conversation UI and close lifecycle", ["e2e:nr005"]],
  ["NR-006 final freeze and NR-G1 manifest", ["e2e:nr006"]],
  ["NR-106 scale evidence and NR-G2 manifest", ["e2e:nr106"]],
  ["NR-206 agent-loop evidence and NR-G3 manifest", ["e2e:nr206"]],
  ["NR-301 embedded Novelist Pack integrity", ["pack:nr301:check"]],
  ["NR-306 evidence and NR-G4 technical submission manifest", ["e2e:nr306"]],
  ["NR-406 Canon transactions and NR-G5 validation", ["e2e:nr406"]],
  ["NR-501 chapter contracts and current approval", ["e2e:nr501"]],
  ["Rust format check", ["format:rust"]],
  ["Rust Clippy", ["lint:rust"]],
  ["Rust tests", ["test:rust"]],
  ["NR-502 chapter commit integration evidence", ["e2e:nr502"]],
  ["NR-503 continuity and chapter-source evidence", ["e2e:nr503"]],
  ["NR-504 nonlinear loop and native budget evidence", ["e2e:nr504"]],
  ["NR-505 production publication and independent EPUB structure evidence", ["e2e:nr505"]],
  ["NR-506 actual desktop IPC twelve-chapter and fault chains", ["e2e:nr506"]],
  ["NR-601 session isolation and captured desktop runs", ["e2e:nr601"]],
  ["NR-602 branch archive clear and visible export", ["e2e:nr602"]],
  ["NR-603 explicit retry and attempt-scoped reconciliation", ["e2e:nr603"]],
  ["Sprint 1 offline E2E", ["e2e:sprint1"]],
  ["Tauri debug build", ["build:test:debug"]],
  ["Debug desktop smoke test", ["smoke:desktop:debug"]],
  ["Tauri release build", ["build:test"]],
  ["Release desktop smoke test", ["smoke:desktop"]],
  ["Unmounted Canon/continuity safety Kernel", ["e2e:canon-kernel"]],
];

const completedSteps = [];
for (const [name, args] of steps) {
  console.log(`\n[quality] START ${name}`);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: offlineEnvironment,
    stdio: "inherit",
  });

  if (result.error) {
    console.error(`[quality] FAIL ${name}: ${result.error.message}`);
    process.exit(1);
  }

  if (result.status !== 0) {
    console.error(`[quality] FAIL ${name}`);
    process.exit(result.status ?? 1);
  }

  console.log(`[quality] PASS ${name}`);
  completedSteps.push({ name, args, exitCode: result.status });
}

mkdirSync("target/qa/nr-603", { recursive: true });
writeFileSync(
  "target/qa/nr-603/regression-quality.json",
  JSON.stringify(
    {
      schemaVersion: 1,
      result: "pass",
      startedAt: offlineEnvironment.NOVELGAINKER_QUALITY_STARTED_AT,
      completedAt: new Date().toISOString(),
      steps: completedSteps,
      releaseScopeSha256: releaseScopeSha256(process.cwd(), discoverReleaseSurface(process.cwd())),
    },
    null,
    2,
  ) + "\n",
);
console.log("\n[quality] PASS NovelGainker local quality gate");
