import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const read = (relative) => readFileSync(path.join(root, relative), "utf8");
const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

const retired = [
  "apps/desktop/src-tauri/src/agent_commands.rs",
  "apps/desktop/src/components/AgentConversation.tsx",
  "apps/desktop/src/lib/tauri/agent.ts",
  "crates/novelgainker-agent/Cargo.toml",
  "crates/novelgainker-workflows/Cargo.toml",
  "crates/app-core/src/legacy_retirement.rs",
  "resources/contracts/agent/v1/contract-set.json",
  "resources/contracts/agent-action/v1/contract-set.json",
  "resources/contracts/agent-governance/v1/contract-set.json",
  "resources/contracts/agent-v2/v2/contract-set.json",
];
for (const relative of retired) {
  assert(!existsSync(path.join(root, relative)), `retired surface still exists: ${relative}`);
}

const domain = read("crates/novelgainker-domain/src/lib.rs");
const project = read("crates/app-core/src/project.rs");
const appState = read("crates/app-core/src/app_state_current.rs");
const commands = read("apps/desktop/src-tauri/src/commands_current.rs");
const projectClient = read("apps/desktop/src/lib/tauri/project.ts");
const currentSources = [domain, project, appState, commands, projectClient].join("\n");

assert(
  domain.includes("pub const PROJECT_SCHEMA_VERSION: u16 = 2;"),
  "current project metadata schema is not v2",
);
assert(project.includes("struct ProjectMetadataEnvelope"), "bounded metadata envelope is missing");
assert(project.includes("reset_v1_project_state"), "one-time clean-slate path is missing");
assert(project.includes("ensure_safe_reset_tree"), "clean-slate path safety check is missing");
assert(
  appState.includes("DELETE FROM project_files WHERE project_id IN"),
  "same-root old project-file cleanup is missing",
);
assert(
  !currentSources.includes("workflow" + "PackVersion"),
  "retired metadata field remains in current production code",
);
assert(
  !currentSources.includes("WORKFLOW" + "_PACK_VERSION"),
  "retired metadata constant remains in current production code",
);
assert(
  project.includes("resources/skills/novelist/v1/fixtures/project.template.md"),
  "preserved Novelist project scaffold is missing",
);

const cargoEnvironment = { ...process.env, CARGO_NET_OFFLINE: "true" };
const testRuns = [
  [
    "project-module",
    ["test", "-p", "app-core", "project::tests", "--lib", "--locked", "--offline"],
  ],
  [
    "global-app-state",
    ["test", "-p", "app-core", "--test", "nr_g1_r02_clean_slate", "--locked", "--offline"],
  ],
];
for (const [name, args] of testRuns) {
  const result = spawnSync("cargo", args, {
    cwd: root,
    env: cargoEnvironment,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    process.stderr.write(result.stdout ?? "");
    process.stderr.write(result.stderr ?? "");
    throw new Error(`R02 offline regression failed: ${name}`);
  }
}

const evidenceRoot = path.join(root, "target/qa/nr-002");
mkdirSync(evidenceRoot, { recursive: true });
const evidence = {
  schemaVersion: 1,
  storyId: "NR-G1-R02",
  result: "pass",
  projectSchemaVersion: 2,
  oneTimeCleanSlate: true,
  conversationStartsBlank: true,
  canonMutationCount: 0,
  automatedExternalRequests: 0,
  tests: testRuns.map(([name]) => name),
  sourceSha256: createHash("sha256").update(currentSources).digest("hex"),
};
writeFileSync(
  path.join(evidenceRoot, "nr-002-evidence.json"),
  `${JSON.stringify(evidence, null, 2)}\n`,
  "utf8",
);
console.log("[nr-002] PASS project schema v1->v2 one-time clean slate");
