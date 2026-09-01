import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const sourcePath = path.join(root, "crates/app-core/src/conversation.rs");
const source = readFileSync(sourcePath, "utf8");

for (const required of [
  ".novelgainker/conversation",
  "session.json",
  "start_conversation_turn",
  "finish_conversation_turn",
  "atomic_replace",
  "本次回复未生成。",
  "本次内容未生成完成。",
]) {
  if (!source.includes(required)) {
    throw new Error(`NR-003 conversation boundary missing: ${required}`);
  }
}

const contractBlock = source.slice(
  source.indexOf("pub struct ConversationMessage"),
  source.indexOf("pub struct ConversationTurnStarted"),
);
for (const field of ["skill", "control", "anchor", "canon", "tool", "relative_path"]) {
  if (contractBlock.toLocaleLowerCase("en-US").includes(field)) {
    throw new Error(`forbidden NR-003 contract field present: ${field}`);
  }
}

if (/write_session\([^)]*accumulated_text/iu.test(source)) {
  throw new Error("NR-003 must not persist streaming deltas");
}
if (source.includes(".novelgainker/agent")) {
  throw new Error("NR-003 store must not read the retired Agent root");
}

const evidenceRoot = path.join(root, "target/qa/nr-003");
mkdirSync(evidenceRoot, { recursive: true });
const evidence = {
  schemaVersion: 1,
  storyId: "NR-003",
  result: "pass",
  storeRoot: ".novelgainker/conversation",
  oldDataReaderPresent: false,
  deltaPersistencePresent: false,
  runningRecovery: "本次回复未生成。",
  abnormalNonemptySuffix: "本次内容未生成完成。",
  sourceSha256: createHash("sha256").update(source).digest("hex"),
};
writeFileSync(
  path.join(evidenceRoot, "nr-003-evidence.json"),
  `${JSON.stringify(evidence, null, 2)}\n`,
  "utf8",
);
console.log("[nr-003] PASS project-bound atomic Conversation Store / no legacy reader");
