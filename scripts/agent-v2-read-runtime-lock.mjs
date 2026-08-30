import { createHash } from "node:crypto";
import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const ROOT = path.resolve("resources", "contracts", "agent-v2", "v2");
const MANIFEST = "contract-set.json";
const EXPECTED = [
  "agent-control-action.schema.json",
  "canon-proposal.schema.json",
  "contract-types.ts",
  "memory-compaction.schema.json",
  "persistence.schema.json",
  "skill-selection.schema.json",
];

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function aggregate(resources) {
  const hash = createHash("sha256");
  for (const resource of resources) {
    hash.update(resource.path, "utf8");
    hash.update(Buffer.from([0]));
    hash.update(resource.sha256, "utf8");
    hash.update("\n", "utf8");
  }
  return hash.digest("hex");
}

function strictObjects(value) {
  if (Array.isArray(value)) return value.forEach(strictObjects);
  if (!value || typeof value !== "object") return;
  if (value.type === "object" && value.properties) {
    assert(value.additionalProperties === false, "agent-v2.schema-object-not-strict");
  }
  Object.values(value).forEach(strictObjects);
}

async function validateContracts() {
  const selection = JSON.parse(
    await readFile(path.join(ROOT, "skill-selection.schema.json"), "utf8"),
  );
  const action = JSON.parse(
    await readFile(path.join(ROOT, "agent-control-action.schema.json"), "utf8"),
  );
  const canon = JSON.parse(await readFile(path.join(ROOT, "canon-proposal.schema.json"), "utf8"));
  const persistence = JSON.parse(
    await readFile(path.join(ROOT, "persistence.schema.json"), "utf8"),
  );
  const compaction = JSON.parse(
    await readFile(path.join(ROOT, "memory-compaction.schema.json"), "utf8"),
  );
  const types = await readFile(path.join(ROOT, "contract-types.ts"), "utf8");
  [selection, action, canon, persistence, compaction].forEach((schema) => {
    assert(
      schema.$schema === "https://json-schema.org/draft/2020-12/schema",
      "agent-v2.schema-dialect-drift",
    );
    strictObjects(schema);
  });
  assert(
    selection.properties.selectedSkillIds.maxItems === 4,
    "agent-v2.skill-selection-cap-drift",
  );
  const start = action.oneOf.find((item) => item.properties?.action?.const === "start_response");
  assert(
    start && !Object.hasOwn(start.properties, "body") && !Object.hasOwn(start.properties, "text"),
    "agent-v2.control-leaks-conversation-body",
  );
  assert(start.properties.markSuggestions.maxItems === 8, "agent-v2.gar2-mark-cap-drift");
  const submitCanon = action.oneOf.find(
    (item) => item.properties?.action?.const === "submit_canon_proposal",
  );
  assert(submitCanon, "agent-v2.canon-action-missing");
  assert(canon.properties.facts.maxItems === 32, "agent-v2.canon-fact-cap-drift");
  assert(persistence.properties.sourceRefs.maxItems === 48, "agent-v2.source-cap-drift");
  assert(
    compaction.properties.baseMemoryHash.pattern === "^[a-f0-9]{64}$",
    "agent-v2.compaction-hash-drift",
  );
  for (const declaration of [
    "SkillSelectionV2",
    "AgentControlActionV2",
    "CanonProposalV1",
    "ConversationMessageV2",
  ]) {
    assert(
      types.includes(`type ${declaration}`),
      `agent-v2.typescript-mirror-missing:${declaration}`,
    );
  }
  assert(!types.includes(": any"), "agent-v2.typescript-any-forbidden");
}

async function main() {
  const mode = process.argv[2];
  assert(mode === "--write" || mode === "--check", "Use --write or --check");
  const files = (await readdir(ROOT, { withFileTypes: true }))
    .filter((entry) => entry.isFile() && entry.name !== MANIFEST)
    .map((entry) => entry.name)
    .sort();
  assert(JSON.stringify(files) === JSON.stringify(EXPECTED), "agent-v2.resource-set-drift");
  const resources = [];
  for (const resourcePath of files) {
    const bytes = await readFile(path.join(ROOT, resourcePath));
    assert(!bytes.includes(0x0d), `agent-v2.crlf-forbidden:${resourcePath}`);
    new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    resources.push({ path: resourcePath, bytes: bytes.length, sha256: sha256(bytes) });
  }
  const contractSetSha256 = aggregate(resources);
  const manifestPath = path.join(ROOT, MANIFEST);
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  assert(manifest.schemaVersion === 2, "agent-v2.manifest-version-drift");
  assert(manifest.contractSetId === "novelgainker.agent-v2", "agent-v2.manifest-id-drift");
  assert(manifest.contractSetVersion === "2.0.0", "agent-v2.contract-version-drift");
  assert(manifest.runtimeEnabled === true, "agent-v2.runtime-must-be-enabled");
  if (mode === "--write") {
    manifest.resources = resources;
    manifest.contractSetSha256 = contractSetSha256;
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  } else {
    assert(JSON.stringify(manifest.resources) === JSON.stringify(resources), "agent-v2.lock-stale");
    assert(manifest.contractSetSha256 === contractSetSha256, "agent-v2.aggregate-hash-stale");
  }
  await validateContracts();
  console.log(
    `[agent-v2-read-runtime-lock] ${mode === "--check" ? "PASS" : "UPDATED"} ${resources.length} resources, contract ${contractSetSha256}`,
  );
}

await main();
