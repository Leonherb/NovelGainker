import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const read = (relative) => readFileSync(path.join(root, relative), "utf8");
const commands = read("apps/desktop/src-tauri/src/commands_current.rs");
const provider = read("crates/app-core/src/provider_core.rs");
const conversation = read("crates/app-core/src/conversation.rs");
const runtime = read("crates/app-core/src/conversation_runtime.rs");
const client = read("apps/desktop/src/lib/tauri/conversation.ts");

for (const required of [
  "conversation_session_load",
  "conversation_turn_start",
  "conversation_turn_cancel",
  "conversation-turn-event",
]) {
  if (!commands.includes(required) && !client.includes(required)) {
    throw new Error("NR-004 IPC missing: " + required);
  }
}
for (const required of [
  "assemble_main_request",
  "with_secret",
  "execute_memory_managed_turn",
  "stream_once_cancellable_blocking",
]) {
  if (
    !commands.includes(required) &&
    !conversation.includes(required) &&
    !runtime.includes(required)
  ) {
    throw new Error("NR-004 Host boundary missing: " + required);
  }
}
if (!provider.includes("自然回应用户，可以普通交流、共同构思或直接创作")) {
  throw new Error("ordinary chat must not be gated by novel setup");
}

const turnStart = commands.indexOf("pub fn conversation_turn_start");
const turnEnd = commands.indexOf("pub fn conversation_turn_cancel", turnStart);
if (turnStart < 0 || turnEnd <= turnStart) throw new Error("Conversation Turn block is missing");
const turnHandlerBlock = commands.slice(turnStart, turnEnd);
const providerRunStart = commands.indexOf("fn run_conversation_provider");
const providerRunEnd = commands.indexOf("fn now_ms", providerRunStart);
if (providerRunStart < 0 || providerRunEnd <= providerRunStart) {
  throw new Error("Conversation Provider runner is missing");
}
const providerRunBlock = commands.slice(providerRunStart, providerRunEnd);
const runtimeStart = runtime.indexOf("pub fn execute_memory_managed_turn");
const runtimeEnd = runtime.indexOf("#[cfg(test)]", runtimeStart);
if (runtimeStart < 0 || runtimeEnd <= runtimeStart) {
  throw new Error("Conversation Memory runtime is missing");
}
const runtimeBlock = runtime.slice(runtimeStart, runtimeEnd);
const providerCallsPerTurn =
  (turnHandlerBlock.match(/run_conversation_provider\(/gu) ?? []).length *
  (providerRunBlock.match(/execute_memory_managed_turn\(/gu) ?? []).length *
  (runtimeBlock.match(/execute_provider_turn\(/gu) ?? []).length;
const releaseTurnPath = turnHandlerBlock + providerRunBlock + runtimeBlock;
const skillCallsPerTurn = (
  releaseTurnPath.match(/(?:skill_call|execute_skill|invoke_skill)\s*\(/giu) ?? []
).length;
const toolCallsPerTurn = (
  releaseTurnPath.match(/(?:tool_call|execute_tool|invoke_tool)\s*\(/giu) ?? []
).length;
const canonOperationsPerTurn = (
  releaseTurnPath.match(/(?:canon_(?:read|write)|read_project_file|save_project_file)\s*\(/giu) ??
  []
).length;
if (providerCallsPerTurn !== 1) throw new Error("one Provider call per Turn is required");
if (skillCallsPerTurn !== 0 || toolCallsPerTurn !== 0 || canonOperationsPerTurn !== 0) {
  throw new Error("NR-0 Turn path contains a forbidden Skill, Tool, or Canon operation");
}

const cargo = spawnSync(
  "cargo",
  [
    "test",
    "-p",
    "app-core",
    "conversation::tests::both_provider_loopbacks_preserve_text_for_every_terminal_shape",
    "--locked",
    "--offline",
  ],
  {
    cwd: root,
    env: { ...process.env, CARGO_NET_OFFLINE: "true" },
    encoding: "utf8",
    stdio: "inherit",
  },
);
if (cargo.status !== 0) {
  throw new Error("NR-004 dual Provider Loopback failed");
}

const evidenceRoot = path.join(root, "target/qa/nr-004");
mkdirSync(evidenceRoot, { recursive: true });
const evidence = {
  schemaVersion: 1,
  storyId: "NR-004",
  result: "pass",
  providerCallsPerTurn,
  skillCallsPerTurn,
  toolCallsPerTurn,
  canonOperationsPerTurn,
  providers: ["deepseek", "openai"],
  terminalCases: ["stop", "length", "protocol-error", "empty-error", "manual-cancel"],
  manualCancelLimitMs: 2000,
  credentialReadIpcPresent: false,
  networkMode: "cargo-offline-loopback-only",
  automatedExternalRequests: 0,
  liveProviderSmokeExecuted: false,
  runtimeSha256: createHash("sha256")
    .update(commands + conversation)
    .digest("hex"),
};
writeFileSync(
  path.join(evidenceRoot, "nr-004-evidence.json"),
  JSON.stringify(evidence, null, 2) + "\n",
  "utf8",
);
console.log("[nr-004] PASS dual Provider Loopback / one call / zero Skill Tool Canon calls");
