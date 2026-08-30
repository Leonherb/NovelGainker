import { spawnSync } from "node:child_process";

for (const [script, label] of [
  ["scripts/agent-migration-action-contract-lock.mjs", "Action contract"],
  ["scripts/agent-migration-governance-contract-lock.mjs", "Governance contract"],
  ["scripts/agent-migration-am2-contract-lock.mjs", "unmounted write runtime contract"],
]) {
  const result = spawnSync(process.execPath, [script, "--check"], {
    cwd: process.cwd(),
    env: { ...process.env, CARGO_NET_OFFLINE: "true" },
    stdio: "inherit",
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
  console.log(`[agent-write-kernel] PASS ${label}`);
}
