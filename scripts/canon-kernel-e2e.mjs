import { spawnSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

for (const args of [
  ["test", "-p", "novelgainker-workflows", "--locked", "--offline"],
  ["test", "-p", "app-core", "--lib", "ng_50", "--locked", "--offline"],
]) {
  const result = spawnSync("cargo", args, {
    cwd: process.cwd(),
    env: { ...process.env, CARGO_NET_OFFLINE: "true" },
    stdio: "inherit",
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
const directory = path.join("target", "qa", "canon-kernel");
await mkdir(directory, { recursive: true });
await writeFile(
  path.join(directory, "evidence.json"),
  `${JSON.stringify(
    {
      schemaVersion: 1,
      productionMounted: false,
      coverage: [
        "serializer",
        "schema",
        "transaction",
        "recovery",
        "context",
        "continuity",
        "ledger",
      ],
      passed: true,
    },
    null,
    2,
  )}\n`,
);
console.log("[e2e:canon-kernel] PASS unmounted Canon/continuity safety kernels");
