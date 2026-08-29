import { createHash } from "node:crypto";
import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const ROOT = process.cwd();
const CONTRACT_ROOT = path.join(ROOT, "resources", "contracts", "agent-runtime", "v2");
const MANIFEST_PATH = "contract-set.json";
const EXPECTED_RESOURCES = ["agent-write-action.schema.json"];

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function contractSetHash(resources) {
  const hash = createHash("sha256");
  for (const resource of resources) {
    hash.update(resource.path, "utf8");
    hash.update(Buffer.from([0]));
    hash.update(resource.sha256, "utf8");
    hash.update("\n", "utf8");
  }
  return hash.digest("hex");
}

async function materializeResources() {
  const entries = await readdir(CONTRACT_ROOT, { withFileTypes: true });
  const files = entries
    .filter((entry) => entry.isFile() && entry.name !== MANIFEST_PATH)
    .map((entry) => entry.name)
    .sort();
  assert(JSON.stringify(files) === JSON.stringify(EXPECTED_RESOURCES), "AM2 resource set drifted");
  return Promise.all(
    files.map(async (resourcePath) => {
      const bytes = await readFile(path.join(CONTRACT_ROOT, resourcePath));
      assert(!bytes.includes(0x0d), `CRLF forbidden: ${resourcePath}`);
      new TextDecoder("utf-8", { fatal: true }).decode(bytes);
      return { path: resourcePath, bytes: bytes.length, sha256: sha256(bytes) };
    }),
  );
}

function walkObjects(value, visit) {
  if (Array.isArray(value)) value.forEach((item) => walkObjects(item, visit));
  else if (value && typeof value === "object") {
    visit(value);
    Object.values(value).forEach((item) => walkObjects(item, visit));
  }
}

async function validateSchema() {
  const source = await readFile(path.join(CONTRACT_ROOT, EXPECTED_RESOURCES[0]), "utf8");
  const schema = JSON.parse(source);
  assert(schema.$schema === "https://json-schema.org/draft/2020-12/schema", "AM2 dialect drifted");
  assert(schema.oneOf.length === 3, "AM2 action variants drifted");
  assert(schema.$defs.ToolRequest.oneOf.length === 6, "AM2 tool union drifted");
  for (const toolId of [
    "project.list_markdown",
    "project.read_markdown",
    "project.search_markdown",
    "project.validate_markdown",
    "project.create_markdown",
    "project.patch_markdown",
  ])
    assert(source.includes(toolId), `AM2 tool missing: ${toolId}`);
  const toolVariant = schema.oneOf.find(
    (variant) => variant.properties?.action?.const === "request_tools",
  );
  assert(toolVariant.properties.toolRequests.maxItems === 4, "AM2 batch cap drifted");
  walkObjects(schema, (object) => {
    if (object.type === "object" && object.properties)
      assert(object.additionalProperties === false, "AM2 object is not strict");
  });
}

async function main() {
  const mode = process.argv[2];
  assert(mode === "--write" || mode === "--check", "Use --write or --check");
  const resources = await materializeResources();
  const hash = contractSetHash(resources);
  const manifestPath = path.join(CONTRACT_ROOT, MANIFEST_PATH);
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  assert(manifest.schemaVersion === 2, "AM2 manifest version drifted");
  assert(
    manifest.contractSetId === "novelgainker.agent-runtime-governed-write",
    "AM2 contract id drifted",
  );
  assert(manifest.version === "2.0.0", "AM2 contract version drifted");
  assert(manifest.enabledAtRuntime === true, "AM2 runtime must be enabled");
  assert(manifest.readToolCount === 4 && manifest.writeToolCount === 2, "AM2 tool counts drifted");
  if (mode === "--write") {
    manifest.resources = resources;
    manifest.contractSetHash = hash;
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  } else {
    assert(JSON.stringify(manifest.resources) === JSON.stringify(resources), "AM2 lock is stale");
    assert(manifest.contractSetHash === hash, "AM2 contract hash is stale");
  }
  await validateSchema();
  console.log(`[agent-am2-contract-lock] PASS ${resources.length} resource, contract ${hash}`);
}

await main();
