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
  ["AM0 NG-A001 Skill Pack lock", ["skills:check"]],
  ["AM0 NG-A002 Agent contract lock", ["agent-contracts:check"]],
  ["AM0 NG-A003 Action contract lock", ["agent-actions:check"]],
  ["AM0 NG-A004 Governance contract lock", ["agent-governance:check"]],
  ["AM1 read-only Agent runtime contract lock", ["agent-runtime:check"]],
  ["AM2 governed-write Agent runtime contract lock", ["agent-write-runtime:check"]],
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
  ["NG-502 Reader Mental Model E2E", ["e2e:ng502"]],
  ["NG-503 Chapter Proposal E2E", ["e2e:ng503"]],
  ["NG-504 local continuity E2E", ["e2e:ng504"]],
  ["NG-505 recoverable chapter apply E2E", ["e2e:ng505"]],
  ["NG-506 lifecycle and Ledger E2E", ["e2e:ng506"]],
  ["NG-507 dual-Provider Storytelling G5 E2E", ["e2e:ng507", "--", "--quality-sequence"]],
  ["NG-A001 inactive Skill Pack evidence", ["e2e:nga001", "--", "--quality-sequence"]],
  ["NG-A002 Session/Turn contract evidence", ["e2e:nga002", "--", "--quality-sequence"]],
  ["NG-A003 Action/Tool/Provider contract evidence", ["e2e:nga003", "--", "--quality-sequence"]],
  ["NG-A004 Authorization/Persistence/Undo evidence", ["e2e:nga004", "--", "--quality-sequence"]],
  ["NG-A101 unified Agent timeline evidence", ["e2e:nga101", "--", "--quality-sequence"]],
  ["NG-A102 bounded read-only tools evidence", ["e2e:nga102", "--", "--quality-sequence"]],
  ["NG-A103 Skill Router and Tool Loop evidence", ["e2e:nga103", "--", "--quality-sequence"]],
  [
    "NG-A104 bounded recovery and dual-Provider evidence",
    ["e2e:nga104", "--", "--quality-sequence"],
  ],
  ["GA1 technical submission", ["e2e:ga1", "--", "--quality-sequence"]],
  ["NG-A201 governed Markdown preview evidence", ["e2e:nga201", "--", "--quality-sequence"]],
  ["NG-A202 atomic transaction and Undo evidence", ["e2e:nga202", "--", "--quality-sequence"]],
  ["NG-A203 authorization and Pending evidence", ["e2e:nga203", "--", "--quality-sequence"]],
  ["NG-A204 Skill-routed write loop evidence", ["e2e:nga204", "--", "--quality-sequence"]],
  ["NG-A205 Timeline Diff and Undo evidence", ["e2e:nga205", "--", "--quality-sequence"]],
  ["NG-A206 unified entry and dual-Provider evidence", ["e2e:nga206", "--", "--quality-sequence"]],
  ["GA2 technical submission", ["e2e:ga2", "--", "--quality-sequence"]],
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
