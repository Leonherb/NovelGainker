import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

if (process.argv.length !== 2) {
  console.error("[g2-live] This entry accepts no arguments or credential values.");
  process.exit(1);
}

const SECRET_ENVIRONMENT_NAME =
  /(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu;
const liveEnvironment = {
  ...process.env,
  CARGO_NET_OFFLINE: "true",
};
for (const name of Object.keys(liveEnvironment)) {
  if (SECRET_ENVIRONMENT_NAME.test(name)) {
    delete liveEnvironment[name];
  }
}

console.log(
  "[g2-live] START fixed DeepSeek/OpenAI Preset check using only Windows Credential Manager",
);
const testName =
  "credential_vault::tests::ng_207_live_provider_smoke_reads_only_fixed_windows_vault_targets";
const result = spawnSync(
  "cargo",
  [
    "test",
    "-p",
    "app-core",
    "--locked",
    "--offline",
    testName,
    "--",
    "--ignored",
    "--exact",
    "--nocapture",
  ],
  {
    cwd: process.cwd(),
    env: liveEnvironment,
    encoding: "utf8",
  },
);

if (result.error || result.status !== 0) {
  const combined = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  const stableFailure =
    combined.match(
      /G2 live (?:Vault borrow|Provider) failed for (?:deepseek|openai) with stable (?:code|kind) [A-Za-z0-9_.-]+/u,
    )?.[0] ?? "fixed Vault or Provider check failed";
  console.error(`[g2-live] FAIL ${stableFailure}`);
  process.exit(result.status ?? 1);
}

const combined = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
const evidenceLines = combined
  .split(/\r?\n/u)
  .filter((line) => line.startsWith("[g2-live] provider="));
if (
  evidenceLines.length !== 2 ||
  !evidenceLines.some((line) => line.includes("provider=deepseek")) ||
  !evidenceLines.some((line) => line.includes("provider=openai"))
) {
  console.error("[g2-live] FAIL redacted two-Provider evidence was incomplete");
  process.exit(1);
}
for (const line of evidenceLines) {
  console.log(line);
}

const evidenceRoot = resolve("target", "g2-evidence");
mkdirSync(evidenceRoot, { recursive: true });
const status = {
  schemaVersion: 1,
  gate: "G2",
  status: "executed-passed",
  executedAt: new Date().toISOString(),
  providers: [
    {
      providerId: "deepseek",
      presetVersion: 1,
      model: "deepseek-v4-pro",
      result: "passed",
    },
    {
      providerId: "openai",
      presetVersion: 1,
      model: "gpt-5.6-terra",
      result: "passed",
    },
  ],
};
writeFileSync(
  resolve(evidenceRoot, "live-smoke-status.json"),
  `${JSON.stringify(status, null, 2)}\n`,
  "utf8",
);
console.log("[g2-live] PASS redacted status saved under target/g2-evidence");
