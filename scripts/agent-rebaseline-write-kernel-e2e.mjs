import { spawnSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const suites = [
  ["novelgainker-agent", "governance_contract_tests", "policy/risk/confirmation/undo rejection"],
  ["novelgainker-agent", "action_contract_tests", "tool/action/repair/provider-boundary rejection"],
  ["novelgainker-agent", "contract_tests", "state/budget/cancellation rejection"],
  ["app-core", "agent_write::tests", "path/hash/protected-target preflight"],
  ["app-core", "agent_transaction::tests", "atomic transaction/undo/rollback"],
  [
    "app-core",
    "provider_core::mock_contract_tests::common_cancellation",
    "dual-provider cancellation",
  ],
];
for (const [crate, filter, label] of suites) {
  const result = spawnSync(
    "cargo",
    ["test", "-p", crate, "--lib", filter, "--locked", "--offline"],
    { cwd: process.cwd(), env: { ...process.env, CARGO_NET_OFFLINE: "true" }, stdio: "inherit" },
  );
  if (result.status !== 0) process.exit(result.status ?? 1);
  console.log(`[agent-write-kernel] PASS ${label}`);
}
const evidence = {
  schemaVersion: 1,
  gate: "GAR1-unmounted-write-kernel",
  productionMounted: false,
  mappedNegativeScenarios: [
    "prompt-injection",
    "path-traversal-and-absolute-path",
    "unknown-tool-and-authority-escalation",
    "source-hash-drift",
    "confirmation-hash-drift-and-reuse",
    "mixed-risk-zero-partial-commit",
    "atomic-rollback",
    "undo-protection-and-expiry",
    "dual-provider-cancellation",
    "sensitive-canary-redaction",
  ],
  agentCanonWriteEntryMounted: false,
  passed: true,
};
const directory = path.join("target", "qa", "agent-write-kernel");
await mkdir(directory, { recursive: true });
await writeFile(path.join(directory, "evidence.json"), `${JSON.stringify(evidence, null, 2)}\n`);
console.log("[e2e:agent-write-kernel] PASS migrated AM2 negative regression matrix");
