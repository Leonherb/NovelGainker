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
  ["Rust format check", ["format:rust"]],
  ["Rust Clippy", ["lint:rust"]],
  ["Rust tests", ["test:rust"]],
  ["Sprint 1 offline E2E", ["e2e:sprint1"]],
  ["Tauri debug build", ["build:test:debug"]],
  ["Debug desktop smoke test", ["smoke:desktop:debug"]],
  ["Tauri release build", ["build:test"]],
  ["Release desktop smoke test", ["smoke:desktop"]],
  ["Sprint 2 security E2E", ["e2e:sprint2"]],
  ["Sprint 3 safe apply E2E", ["e2e:sprint3"]],
  ["NG-401 offline project-create E2E", ["e2e:ng401"]],
  ["NG-402 setting-create E2E", ["e2e:ng402"]],
  ["NG-403 material-create E2E", ["e2e:ng403"]],
  ["NG-404 plot-create E2E", ["e2e:ng404"]],
  ["NG-405 character-v2 E2E", ["e2e:ng405"]],
  ["NG-406 workflow Router E2E", ["e2e:ng406"]],
  ["Sprint 4 G4 evidence E2E", ["e2e:ng407", "--", "--quality-sequence"]],
  ["NG-501 Context Builder E2E", ["e2e:ng501"]],
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
