import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const workspaceRoot = process.cwd();
const evidencePath = resolve(workspaceRoot, "target/g1-evidence/offline-e2e.json");
const sensitiveEnvironmentName = /(openai|deepseek|api.?key|access.?token|auth.?token)/i;
const offlineEnvironment = Object.fromEntries(
  Object.entries(process.env).filter(([name]) => !sensitiveEnvironmentName.test(name)),
);

Object.assign(offlineEnvironment, {
  CARGO_NET_OFFLINE: "true",
  HTTP_PROXY: "http://127.0.0.1:9",
  HTTPS_PROXY: "http://127.0.0.1:9",
  ALL_PROXY: "http://127.0.0.1:9",
  NO_PROXY: "",
});

const steps = [
  {
    name: "static no-network and capability boundary",
    command: process.execPath,
    args: ["scripts/check-sprint0-boundaries.mjs"],
  },
  {
    name: "isolated offline editor round trip",
    command: "cargo",
    args: [
      "test",
      "-p",
      "app-core",
      "--test",
      "sprint1_offline_e2e",
      "--locked",
      "--offline",
      "--",
      "--nocapture",
    ],
  },
  {
    name: "aborted-save recovery",
    command: "cargo",
    args: [
      "test",
      "-p",
      "app-core",
      "--locked",
      "--offline",
      "recovery::tests::ng_104_recovers_a_temporary_file_left_by_an_aborted_process",
    ],
  },
  {
    name: "cross-process writer termination",
    command: "cargo",
    args: [
      "test",
      "-p",
      "app-core",
      "--locked",
      "--offline",
      "coordination::tests::ng_104_windows_mutex_coordinates_processes_and_survives_process_termination",
    ],
  },
];

for (const step of steps) {
  console.log(`[g1-e2e] START ${step.name}`);
  const result = spawnSync(step.command, step.args, {
    cwd: workspaceRoot,
    env: offlineEnvironment,
    stdio: "inherit",
  });
  if (result.error) {
    console.error(`[g1-e2e] FAIL ${step.name}: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    console.error(`[g1-e2e] FAIL ${step.name}`);
    process.exit(result.status ?? 1);
  }
  console.log(`[g1-e2e] PASS ${step.name}`);
}

const evidence = JSON.parse(readFileSync(evidencePath, "utf8"));
if (
  evidence.schemaVersion !== 1 ||
  evidence.story !== "NG-107" ||
  evidence.externalNetworkRequests !== 0 ||
  evidence.providerKeysConfigured !== false ||
  evidence.queuedAiRequests !== 0 ||
  !Object.values(evidence.checks ?? {}).every((value) => value === true)
) {
  throw new Error("NG-107 evidence contract is incomplete or invalid.");
}

const serializedEvidence = JSON.stringify(evidence);
if (
  /[A-Za-z]:[\\/]/.test(serializedEvidence) ||
  /novelgainker-ng107-e2e-/i.test(serializedEvidence)
) {
  throw new Error("NG-107 evidence must not retain machine or fixture paths.");
}

console.log(
  "[g1-e2e] PASS no Provider keys, no network capability, zero external requests, zero queued AI requests",
);
