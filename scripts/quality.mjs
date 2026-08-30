import { spawnSync } from "node:child_process";

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
  ["GAR1 bundled Skill Pack V2 lock", ["skills-v2:check"]],
  ["Agent Protocol V2 read runtime lock", ["agent-v2-read-runtime:check"]],
  ["Agent Protocol V2 Canon runtime lock", ["agent-v2-write-runtime:check"]],
  ["Unmounted Agent write Kernel locks", ["agent-write-kernel:check"]],
  ["Rust format check", ["format:rust"]],
  ["Rust Clippy", ["lint:rust"]],
  ["Rust tests", ["test:rust"]],
  ["Sprint 1 offline E2E", ["e2e:sprint1"]],
  ["Tauri debug build", ["build:test:debug"]],
  ["Debug desktop smoke test", ["smoke:desktop:debug"]],
  ["Tauri release build", ["build:test"]],
  ["Release desktop smoke test", ["smoke:desktop"]],
  ["Sprint 2 security E2E", ["e2e:sprint2"]],
  ["Unmounted Agent write safety Kernel", ["e2e:agent-write-kernel"]],
  ["Unmounted Canon/continuity safety Kernel", ["e2e:canon-kernel"]],
  ["NG-R101 project entry evidence", ["e2e:ngr101"]],
  ["NG-R102 Conversation/History evidence", ["e2e:ngr102"]],
  ["NG-R103 complete Skill Loader evidence", ["e2e:ngr103"]],
  ["NG-R104 Context scale evidence", ["e2e:ngr104"]],
  ["NG-R105 context-aware Agent loop evidence", ["e2e:ngr105"]],
  ["NG-R106 single-path production cutover evidence", ["e2e:ngr106"]],
  ["GAR1 technical submission", ["e2e:gar1"]],
  ["NG-R201 Conversation anchor evidence", ["e2e:ngr201"]],
  ["NG-R202 Canon proposal evidence", ["e2e:ngr202"]],
  ["NG-R203 Context Mark evidence", ["e2e:ngr203"]],
  ["NG-R204 conflict resolution evidence", ["e2e:ngr204"]],
  ["NG-R205 WritePlan transaction evidence", ["e2e:ngr205"]],
  ["NG-R206 dual-provider reconciliation evidence", ["e2e:ngr206"]],
  ["NG-R207 chapter anchoring evidence", ["e2e:ngr207"]],
  ["NG-R208 scale and retirement evidence", ["e2e:ngr208"]],
  ["GAR2 technical submission", ["e2e:gar2"]],
];

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
}

console.log("\n[quality] PASS NovelGainker local quality gate");
