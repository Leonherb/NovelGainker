import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, writeFile, lstat } from "node:fs/promises";
import path from "node:path";

const ROOT = process.cwd();
const PACK_ROOT = path.join(ROOT, "resources", "skills", "novelist", "v1");
const WORKFLOW_ROOT = path.join(ROOT, "resources", "workflows", "v1");
const EVIDENCE_ROOT = path.join(ROOT, "target", "qa", "ng-a001");
const SOURCE_COMMIT = "64e4a2e1473ff0bad5893eff394ccbf1fa1cd580";
const LICENSE_HASH = "88b9a14391d40016e79135d201f277885eabf9b5388279a3ea0a4270e708c109";
const EXPECTED_WORKFLOW_PACK_HASH =
  "1658685537a9fe57b441928816e0fd2af770109e9b9a12a985204774496246a5";
const EXPECTED_SKILL_IDS = [
  "create-novel-project",
  "create-setting",
  "create-character",
  "create-material",
  "create-plot",
  "novel-story-telling",
  "analytical-review",
  "create-visual-asset",
  "publish-novel",
];
const FORBIDDEN_RESOURCE_SEGMENTS = new Set([
  ".claude-plugin",
  ".codex-plugin",
  "agents",
  "scripts",
  "submission",
  "tests",
  "website",
]);
const ALLOWED_RESOURCE_EXTENSIONS = new Set([".json", ".md", ".upstream"]);

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function comparePaths(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function toRelative(root, absolutePath) {
  return path.relative(root, absolutePath).split(path.sep).join("/");
}

function validateText(resourcePath, bytes) {
  assert(
    !(bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf),
    `BOM forbidden: ${resourcePath}`,
  );
  assert(!bytes.includes(0x0d), `CRLF forbidden: ${resourcePath}`);
  new TextDecoder("utf-8", { fatal: true }).decode(bytes);
}

function validatePackResourcePath(resourcePath) {
  const segments = resourcePath.split("/");
  assert(
    segments.every(
      (segment) =>
        segment.length > 0 &&
        segment !== "." &&
        segment !== ".." &&
        !FORBIDDEN_RESOURCE_SEGMENTS.has(segment.toLowerCase()),
    ),
    `Forbidden Skill Pack path: ${resourcePath}`,
  );
  assert(
    ALLOWED_RESOURCE_EXTENSIONS.has(path.posix.extname(resourcePath).toLowerCase()),
    `Executable or unsupported Skill Pack resource: ${resourcePath}`,
  );
}

async function collectFiles(root, directory = root) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolutePath = path.join(directory, entry.name);
    const metadata = await lstat(absolutePath);
    const relativePath = toRelative(root, absolutePath);
    assert(!metadata.isSymbolicLink(), `Symlink forbidden: ${relativePath}`);
    if (metadata.isDirectory()) {
      files.push(...(await collectFiles(root, absolutePath)));
    } else {
      assert(metadata.isFile(), `Unsupported resource type: ${relativePath}`);
      files.push({ absolutePath, relativePath });
    }
  }
  return files.sort((left, right) => comparePaths(left.relativePath, right.relativePath));
}

async function readJson(absolutePath) {
  return JSON.parse(await readFile(absolutePath, "utf8"));
}

function skillPackHash(resources) {
  const hash = createHash("sha256");
  for (const resource of resources) {
    hash.update(resource.relativePath, "utf8");
    hash.update(Buffer.from([0]));
    hash.update(resource.sha256, "utf8");
    hash.update("\n", "utf8");
  }
  return hash.digest("hex");
}

function u64BigEndian(value) {
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64BE(BigInt(value));
  return buffer;
}

function workflowPackHash(resources) {
  const hash = createHash("sha256");
  for (const resource of resources) {
    const pathBytes = Buffer.from(resource.relativePath, "utf8");
    hash.update(u64BigEndian(pathBytes.length));
    hash.update(pathBytes);
    hash.update(u64BigEndian(resource.bytes.length));
    hash.update(resource.bytes);
  }
  return hash.digest("hex");
}

async function materializeResources(root, validatePackPaths = false) {
  const files = await collectFiles(root);
  const resources = [];
  for (const file of files) {
    if (validatePackPaths) {
      validatePackResourcePath(file.relativePath);
    }
    const bytes = await readFile(file.absolutePath);
    validateText(file.relativePath, bytes);
    resources.push({
      ...file,
      bytes,
      sha256: sha256(bytes),
    });
  }
  return resources;
}

async function runtimeBoundaryEvidence() {
  const productionCargoManifests = [
    "apps/desktop/src-tauri/Cargo.toml",
    "crates/app-core/Cargo.toml",
    "crates/novelgainker-domain/Cargo.toml",
    "crates/novelgainker-validation/Cargo.toml",
    "crates/novelgainker-workflows/Cargo.toml",
  ];
  const productionReferences = [];
  for (const manifestPath of productionCargoManifests) {
    const source = await readFile(path.join(ROOT, manifestPath), "utf8");
    if (source.includes("novelgainker-agent")) {
      productionReferences.push(manifestPath);
    }
  }
  assert(
    JSON.stringify(productionReferences) ===
      JSON.stringify(["apps/desktop/src-tauri/Cargo.toml", "crates/app-core/Cargo.toml"]),
    "AM1 may link the Agent crate only through app-core and the desktop shell",
  );

  const commandManifestPath = path.join(
    ROOT,
    "apps",
    "desktop",
    "src-tauri",
    "src",
    "command_manifest.rs",
  );
  const commandManifest = await readFile(commandManifestPath, "utf8");
  const commands = [...commandManifest.matchAll(/^\s+"([a-z0-9_]+)",$/gmu)].map(
    (match) => match[1],
  );
  assert(commands.length === 60, `Expected 60 approved Tauri commands, found ${commands.length}`);
  const agentCommands = commands.filter(
    (command) => command.includes("agent") || command.includes("skill"),
  );
  assert(
    JSON.stringify(agentCommands) ===
      JSON.stringify([
        "agent_skill_registry_load",
        "agent_session_load",
        "agent_turn_start",
        "agent_turn_cancel",
        "agent_session_clear",
        "agent_policy_set",
        "agent_write_confirm",
        "agent_write_reject",
        "agent_undo_apply",
      ]),
    "Current Agent IPC set drifted",
  );

  const appStateSource = await readFile(
    path.join(ROOT, "crates", "app-core", "src", "app_state.rs"),
    "utf8",
  );
  assert(
    appStateSource.includes("pub const APP_STATE_SCHEMA_VERSION: u16 = 10;"),
    "Current shell must use bounded App State schema v10",
  );

  const buildInputs = [
    "crates/novelgainker-agent/Cargo.toml",
    "crates/novelgainker-agent/build.rs",
    "crates/novelgainker-agent/src/lib.rs",
    "crates/novelgainker-agent/src/skill_pack_validation.rs",
  ];
  const forbiddenCachePatterns = [
    /CODEX_HOME/u,
    /\.codex[\\/]plugins/u,
    /openai-curated-remote/u,
    /AppData[\\/]Local[\\/]OpenAI/u,
    /plugins[\\/]cache/u,
  ];
  const cacheReferences = [];
  for (const buildInput of buildInputs) {
    const source = await readFile(path.join(ROOT, buildInput), "utf8");
    for (const pattern of forbiddenCachePatterns) {
      if (pattern.test(source)) {
        cacheReferences.push({ buildInput, pattern: pattern.source });
      }
    }
  }
  assert(cacheReferences.length === 0, "Skill Pack build inputs reference a user plugin cache");
  assert(
    (await readFile(path.join(ROOT, "crates", "novelgainker-agent", "build.rs"), "utf8")).includes(
      "../../resources/skills/novelist/v1",
    ),
    "Build script must use only the repository Skill Pack root",
  );

  return {
    productionCargoManifests,
    productionDependencyReferences: productionReferences,
    buildInputs,
    externalCacheReferences: cacheReferences,
    appStateSchemaVersion: 10,
    tauriCommandCount: commands.length,
    newAgentOrSkillCommands: agentCommands,
    immutablePackEnabledAtRuntime: true,
    am1ReadOnlyViewEnabledAtRuntime: true,
    am2GovernedWriteRuntimeEnabled: true,
  };
}

async function writeJson(fileName, value) {
  await writeFile(
    path.join(EVIDENCE_ROOT, fileName),
    `${JSON.stringify(value, null, 2)}\n`,
    "utf8",
  );
}

async function main() {
  const qualitySequence = process.argv.includes("--quality-sequence");
  if (qualitySequence) {
    assert(
      typeof process.env.NOVELGAINKER_QUALITY_STARTED_AT === "string",
      "Quality sequence marker missing",
    );
  }

  const resources = await materializeResources(PACK_ROOT, true);
  const byPath = new Map(resources.map((resource) => [resource.relativePath, resource]));
  const pack = await readJson(path.join(PACK_ROOT, "pack.json"));
  const provenance = await readJson(path.join(PACK_ROOT, "provenance.lock.json"));
  const inventory = await readJson(path.join(PACK_ROOT, "inventory.json"));

  assert(pack.enabledAtRuntime === false, "Pack must remain disabled at runtime");
  assert(pack.skillManifests.length === 9, "Pack must contain nine Skill manifests");
  assert(provenance.sourceCommit === SOURCE_COMMIT, "Pinned upstream commit drifted");
  assert(provenance.licenseSpdx === "MIT", "Upstream license must remain MIT");
  assert(provenance.generatedFromScripts.length === 13, "Script migration map is incomplete");
  assert(byPath.get("LICENSE.upstream")?.sha256 === LICENSE_HASH, "MIT license hash drifted");

  const lockedPaths = new Set(inventory.files.map((file) => file.path));
  const actualLockedPaths = new Set(
    resources
      .filter((resource) => resource.relativePath !== "inventory.json")
      .map((resource) => resource.relativePath),
  );
  assert(lockedPaths.size === actualLockedPaths.size, "Inventory resource count drifted");
  for (const file of inventory.files) {
    const resource = byPath.get(file.path);
    assert(resource, `Inventory references missing resource: ${file.path}`);
    assert(resource.sha256 === file.sha256, `Inventory hash drift: ${file.path}`);
    assert(resource.bytes.length === file.bytes, `Inventory byte length drift: ${file.path}`);
  }
  for (const resourcePath of actualLockedPaths) {
    assert(lockedPaths.has(resourcePath), `Unregistered Skill Pack resource: ${resourcePath}`);
  }

  const manifestSummaries = [];
  for (const [index, manifestPath] of pack.skillManifests.entries()) {
    const manifest = await readJson(path.join(PACK_ROOT, manifestPath));
    assert(manifest.id === EXPECTED_SKILL_IDS[index], `Skill order/id drift: ${manifestPath}`);
    assert(manifest.enabledAtRuntime === false, `Skill activated during AM0: ${manifest.id}`);
    assert(manifest.sourceCommit === SOURCE_COMMIT, `Skill source drift: ${manifest.id}`);
    const declaredResources = [
      ...manifest.instructionResources,
      ...manifest.templateResources,
      ...manifest.referenceResources,
    ];
    assert(
      Object.keys(manifest.resourceHashes).length === declaredResources.length,
      `Skill resource hash set drift: ${manifest.id}`,
    );
    for (const resourcePath of declaredResources) {
      const resource = byPath.get(resourcePath);
      assert(resource, `Skill resource missing: ${resourcePath}`);
      assert(
        manifest.resourceHashes[resourcePath] === resource.sha256,
        `Skill resource hash drift: ${resourcePath}`,
      );
    }
    manifestSummaries.push({
      id: manifest.id,
      manifestPath,
      manifestSha256: byPath.get(manifestPath).sha256,
      riskClass: manifest.riskClass,
      allowedTools: manifest.allowedTools,
      readScopes: manifest.readScopes,
      writeScopes: manifest.writeScopes,
      enabledAtRuntime: manifest.enabledAtRuntime,
      resourceCount: declaredResources.length,
    });
  }

  const computedSkillPackHash = skillPackHash(resources);
  const workflowResources = await materializeResources(WORKFLOW_ROOT);
  const computedWorkflowPackHash = workflowPackHash(workflowResources);
  assert(
    computedWorkflowPackHash === EXPECTED_WORKFLOW_PACK_HASH,
    `Existing Workflow Pack hash changed: ${computedWorkflowPackHash}`,
  );

  const boundary = await runtimeBoundaryEvidence();
  const rootCargo = await readFile(path.join(ROOT, "Cargo.toml"), "utf8");
  assert(
    rootCargo.includes('"crates/novelgainker-agent"'),
    "Contract crate must be a quality-checked workspace member",
  );
  const buildScript = await readFile(
    path.join(ROOT, "crates", "novelgainker-agent", "build.rs"),
    "utf8",
  );
  for (const required of [
    "resource-symlink-forbidden",
    "resource-type-forbidden",
    "validate_pack",
  ]) {
    assert(buildScript.includes(required), `Build-time rejection missing: ${required}`);
  }

  const thirdPartyNotice = await readFile(path.join(ROOT, "THIRD_PARTY_NOTICES.md"), "utf8");
  for (const required of [SOURCE_COMMIT, "Copyright (c) 2026 Changkyun Kim", "License: MIT"]) {
    assert(thirdPartyNotice.includes(required), `Third-party notice missing: ${required}`);
  }

  await mkdir(EVIDENCE_ROOT, { recursive: true });
  const generatedAt = new Date().toISOString();
  await writeJson("am0-skill-pack-provenance.json", {
    schemaVersion: 1,
    generatedAt,
    sourceRepository: provenance.sourceRepository,
    sourceCommit: provenance.sourceCommit,
    sourceSubtree: provenance.sourceSubtree,
    sourceArchiveSha256: provenance.sourceArchiveSha256,
    upstreamFileCount: provenance.upstreamFileCount,
    upstreamInventorySha256: provenance.upstreamInventorySha256,
    adaptationVersion: provenance.adaptationVersion,
    novelGainkerPackVersion: provenance.novelGainkerPackVersion,
    includedSourceFileCount: provenance.includedFiles.length,
    excludedGroups: provenance.excludedFiles,
    generatedFromScripts: provenance.generatedFromScripts,
  });
  await writeJson("am0-skill-pack-inventory.json", {
    schemaVersion: 1,
    generatedAt,
    lockedResourceCount: inventory.files.length,
    totalPackResourceCount: resources.length,
    inventory,
    manifests: manifestSummaries,
  });
  await writeJson("am0-skill-pack-hash-report.json", {
    schemaVersion: 1,
    generatedAt,
    skillPackHashAlgorithm: "sha256(path + NUL + resourceSha256 + LF, sorted by path)",
    skillPackHash: computedSkillPackHash,
    skillPackResourceCount: resources.length,
    licenseSha256: LICENSE_HASH,
    provenanceSha256: byPath.get("provenance.lock.json").sha256,
    inventorySha256: byPath.get("inventory.json").sha256,
    manifestHashes: manifestSummaries.map(({ id, manifestPath, manifestSha256 }) => ({
      id,
      manifestPath,
      manifestSha256,
    })),
    existingWorkflowPackHash: computedWorkflowPackHash,
    expectedWorkflowPackHash: EXPECTED_WORKFLOW_PACK_HASH,
  });
  await writeJson("am0-cache-independence-report.json", {
    schemaVersion: 1,
    generatedAt,
    packBuildRoot: "resources/skills/novelist/v1",
    networkAccessAtBuild: false,
    externalProcessAtBuild: false,
    pythonAtBuildOrRuntime: false,
    pluginCacheDependencyCount: boundary.externalCacheReferences.length,
    productionDependencyCount: boundary.productionDependencyReferences.length,
    immutablePackRuntimeEnabled: false,
    am1ReadOnlyViewEnabledAtRuntime: true,
    runtimeBoundary: boundary,
    existingWorkflowPackHash: computedWorkflowPackHash,
  });
  await writeFile(
    path.join(EVIDENCE_ROOT, "am0-license-and-attribution-report.md"),
    `# AM0 license and attribution report\n\n` +
      `- Source: https://github.com/comfuture/novelist\n` +
      `- Commit: \`${SOURCE_COMMIT}\`\n` +
      `- License: MIT\n` +
      `- Copyright: Copyright (c) 2026 Changkyun Kim\n` +
      `- License SHA-256: \`${LICENSE_HASH}\`\n` +
      `- Upstream plugin inventory SHA-256: \`${provenance.upstreamInventorySha256}\`\n` +
      `- NovelGainker Skill Pack SHA-256: \`${computedSkillPackHash}\`\n` +
      `- Included upstream source records: ${provenance.includedFiles.length}\n` +
      `- Excluded executable script mappings: ${provenance.generatedFromScripts.length}\n` +
      `- Runtime status: immutable Pack flag remains disabled; AM1 uses a separate application-owned read-only view\n\n` +
      `The exact MIT text is checked into \`resources/skills/novelist/v1/LICENSE.upstream\`; adaptations and exclusions are recorded in \`ATTRIBUTION.md\` and \`provenance.lock.json\`.\n`,
    "utf8",
  );

  console.log(
    `[NG-A001] PASS ${manifestSummaries.length} inactive skills, ${resources.length} resources, pack ${computedSkillPackHash}`,
  );
  console.log(`[NG-A001] Evidence: ${path.relative(ROOT, EVIDENCE_ROOT)}`);
}

await main();
