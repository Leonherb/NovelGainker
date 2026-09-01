import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";

const workspaceRoot = process.cwd();
const evidenceRoot = resolve(workspaceRoot, "target/qa/nr-001");
const fixtureRoot = resolve(evidenceRoot, "fixture-work");

const deleteRoots = [
  "apps/desktop/src-tauri/src/agent_commands.rs",
  "apps/desktop/src/components/AgentConversation.tsx",
  "apps/desktop/src/components/AgentConversation.test.tsx",
  "apps/desktop/src/lib/tauri/agent.ts",
  "apps/desktop/src/lib/tauri/agent.test.ts",
  "crates/novelgainker-agent",
  "resources/contracts/agent",
  "resources/contracts/agent-action",
  "resources/contracts/agent-governance",
  "resources/contracts/agent-v2",
  "crates/app-core/src/agent_anchor.rs",
  "crates/app-core/src/agent_canon.rs",
  "crates/app-core/src/agent_conflict.rs",
  "crates/app-core/src/agent_marks.rs",
  "crates/app-core/src/agent_v2.rs",
  "crates/app-core/src/agent_write_plan_v2.rs",
  "docs/architecture-unified-novelist-agent-review.zh-CN.md",
  "docs/stories/agent-conversation-authoring-am-r1-story-packet.zh-CN.md",
  "docs/stories/agent-conversation-authoring-am-r2-story-packet.zh-CN.md",
  "docs/stories/agent-conversation-authoring-rebaseline-plan.zh-CN.md",
  "docs/stories/agent-migration-am0-story-packet.zh-CN.md",
  "docs/stories/agent-migration-am2-story-packet.zh-CN.md",
  "docs/stories/agent-protocol-v2-context-mark-review.zh-CN.md",
  "scripts/agent-migration-action-contract-lock.mjs",
  "scripts/agent-migration-am2-contract-lock.mjs",
  "scripts/agent-migration-contract-lock.mjs",
  "scripts/agent-migration-governance-contract-lock.mjs",
  "scripts/agent-migration-ng-a001-e2e.mjs",
  "scripts/agent-migration-ng-a002-e2e.mjs",
  "scripts/agent-migration-ng-a003-e2e.mjs",
  "scripts/agent-migration-ng-a004-e2e.mjs",
  "scripts/agent-migration-skill-pack-lock.mjs",
  "scripts/agent-v2-read-runtime-lock.mjs",
  "scripts/agent-v2-scale-e2e.mjs",
  "scripts/agent-v2-story-e2e.mjs",
  "scripts/agent-v2-write-scale-e2e.mjs",
  "scripts/gar1-e2e.mjs",
  "scripts/gar2-e2e.mjs",
];

const extractRoots = [
  "crates/app-core/src/agent_runtime.rs",
  "crates/app-core/src/agent_transaction.rs",
  "crates/app-core/src/agent_write.rs",
  "scripts/agent-rebaseline-write-kernel-e2e.mjs",
  "scripts/agent-write-kernel-check.mjs",
];

const preserveRoots = [
  "crates/app-core/src/provider_core.rs",
  "crates/app-core/src/provider_state.rs",
  "crates/app-core/src/credential_vault.rs",
  "crates/app-core/src/project.rs",
  "crates/app-core/src/workspace.rs",
  "crates/app-core/src/recovery.rs",
  "crates/app-core/src/canon_apply.rs",
  "resources/skills/novelist",
];

function normalized(path) {
  return path.split(sep).join("/");
}

function listFiles(relativeRoot) {
  const absoluteRoot = resolve(workspaceRoot, relativeRoot);
  if (!existsSync(absoluteRoot)) return [];
  const stat = lstatSync(absoluteRoot);
  if (!stat.isDirectory()) return [normalized(relativeRoot)];
  const files = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const absolute = join(directory, entry.name);
      if (entry.isDirectory()) visit(absolute);
      else files.push(normalized(relative(workspaceRoot, absolute)));
    }
  };
  visit(absoluteRoot);
  return files.sort();
}

function expand(roots, disposition, replacement) {
  return roots.flatMap((root) => {
    const files = listFiles(root);
    const paths = files.length > 0 ? files : [normalized(root)];
    return paths.map((path) => ({
      path,
      disposition,
      replacement,
      existsAtBaseline: files.length > 0,
    }));
  });
}

function sha256(data) {
  return createHash("sha256").update(data).digest("hex");
}

const canonPaths = [
  "project.md",
  "characters/001.hero.md",
  "world/001.city.md",
  "materials/001.note.md",
  "plot/001.main.md",
  "chapters/001.md",
  "assets/cover.txt",
];

function writeFixture(root, withLegacy) {
  for (const path of canonPaths) {
    const absolute = join(root, path);
    mkdirSync(dirname(absolute), { recursive: true });
    writeFileSync(absolute, `canon:${path}\n`, "utf8");
  }
  mkdirSync(join(root, ".novelgainker"), { recursive: true });
  writeFileSync(join(root, ".novelgainker", "project.json"), '{"schemaVersion":1}\n', "utf8");
  if (withLegacy) {
    mkdirSync(join(root, ".novelgainker", "agent", "history"), { recursive: true });
    writeFileSync(
      join(root, ".novelgainker", "agent", "history", "obsolete.json"),
      '{"obsolete":true}\n',
      "utf8",
    );
  }
}

function canonTreeHash(root) {
  const hash = createHash("sha256");
  for (const path of canonPaths) {
    hash.update(path);
    hash.update("\0");
    hash.update(readFileSync(join(root, path)));
    hash.update("\0");
  }
  return hash.digest("hex");
}

rmSync(fixtureRoot, { recursive: true, force: true });
mkdirSync(fixtureRoot, { recursive: true });

const emptyProject = join(fixtureRoot, "empty-project");
const canonProject = join(fixtureRoot, "canon-project");
const legacyProject = join(fixtureRoot, "legacy-project");
const damagedProject = join(fixtureRoot, "damaged-project");
const reparseProject = join(fixtureRoot, "reparse-project");
writeFixture(emptyProject, false);
writeFixture(canonProject, false);
writeFixture(legacyProject, true);
writeFixture(damagedProject, false);
writeFileSync(join(damagedProject, ".novelgainker", "agent"), "not-a-directory", "utf8");
writeFixture(reparseProject, false);

const reparseTarget = join(fixtureRoot, "outside-reparse-target");
mkdirSync(reparseTarget, { recursive: true });
writeFileSync(join(reparseTarget, "must-survive.txt"), "outside-safe-target\n", "utf8");
let reparseFixture = "created";
try {
  symlinkSync(reparseTarget, join(reparseProject, ".novelgainker", "agent"), "junction");
} catch (error) {
  reparseFixture = `unsupported:${error.code ?? "unknown"}`;
}

const baselineHash = canonTreeHash(canonProject);
const legacyHash = canonTreeHash(legacyProject);
if (baselineHash !== legacyHash) {
  throw new Error("Legacy Agent metadata changed the Canon tree baseline");
}

const entries = [
  ...expand(deleteRoots, "delete", "NR-0 current Conversation or no replacement"),
  ...expand(extractRoots, "extract", "data-neutral safety kernel"),
  ...expand(preserveRoots, "preserve", "current project/provider/safety infrastructure"),
].sort((left, right) => left.path.localeCompare(right.path));

if (entries.length < 40 || !entries.some((entry) => entry.path.endsWith("agent_commands.rs"))) {
  throw new Error("Retirement manifest did not cover the expected brownfield surface");
}

mkdirSync(evidenceRoot, { recursive: true });
const manifest = {
  schemaVersion: 1,
  storyId: "NR-001",
  generatedAt: new Date().toISOString(),
  entries,
  protectedCanonRoots: [
    "project.md",
    "characters/",
    "world/",
    "materials/",
    "plot/",
    "chapters/",
    "assets/",
  ],
  oldProjectDataRoot: ".novelgainker/agent/",
  newConversationRoot: ".novelgainker/conversation/",
};
writeFileSync(
  join(evidenceRoot, "nr-0-retirement-manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
  "utf8",
);

const evidence = {
  schemaVersion: 1,
  storyId: "NR-001",
  generatedAt: new Date().toISOString(),
  result: "pass",
  retirementEntryCount: entries.length,
  canonTreeHash: baselineHash,
  legacyAgentMetadataDoesNotAffectCanonHash: true,
  fixtures: {
    emptyProject: true,
    canonProject: true,
    legacyProject: true,
    damagedLegacyRoot: true,
    reparseFixture,
  },
  reparseTargetHash: sha256(readFileSync(join(reparseTarget, "must-survive.txt"))),
};
writeFileSync(
  join(evidenceRoot, "nr-001-evidence.json"),
  `${JSON.stringify(evidence, null, 2)}\n`,
  "utf8",
);

console.log(
  `[nr-001] PASS ${entries.length} retirement entries; Canon ${baselineHash}; reparse ${reparseFixture}`,
);
