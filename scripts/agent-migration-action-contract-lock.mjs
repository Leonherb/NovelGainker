import { createHash } from "node:crypto";
import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const ROOT = process.cwd();
const CONTRACT_ROOT = path.join(ROOT, "resources", "contracts", "agent-action", "v1");
const MANIFEST_PATH = "contract-set.json";
const EXPECTED_RESOURCES = [
  "action-tool.schema.json",
  "action-types.ts",
  "fixtures/deepseek-native-tool-call.json",
  "fixtures/invalid-absolute-path.json",
  "fixtures/invalid-duplicate-call-id.json",
  "fixtures/invalid-provider-authority.json",
  "fixtures/invalid-unknown-field.json",
  "fixtures/invalid-unknown-tool.json",
  "fixtures/openai-native-tool-call.json",
  "fixtures/strict-json-action.json",
];

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function comparePaths(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

async function collectFiles(directory = CONTRACT_ROOT) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await collectFiles(absolutePath)));
    else {
      assert(entry.isFile(), `Unsupported action contract resource: ${absolutePath}`);
      files.push(path.relative(CONTRACT_ROOT, absolutePath).split(path.sep).join("/"));
    }
  }
  return files.sort(comparePaths);
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
  const files = await collectFiles();
  const actual = files.filter((resourcePath) => resourcePath !== MANIFEST_PATH);
  assert(
    JSON.stringify(actual) === JSON.stringify(EXPECTED_RESOURCES),
    `Action contract resource set drifted: ${actual.join(", ")}`,
  );
  const resources = [];
  for (const resourcePath of actual) {
    const bytes = await readFile(path.join(CONTRACT_ROOT, resourcePath));
    assert(!bytes.includes(0x0d), `CRLF forbidden: ${resourcePath}`);
    new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    resources.push({ path: resourcePath, bytes: bytes.length, sha256: sha256(bytes) });
  }
  return resources;
}

async function validateMirrors() {
  const schema = JSON.parse(
    await readFile(path.join(CONTRACT_ROOT, "action-tool.schema.json"), "utf8"),
  );
  assert(
    schema.$schema === "https://json-schema.org/draft/2020-12/schema",
    "Action JSON Schema dialect drifted",
  );
  assert(
    schema.$id === "https://novelgainker.local/schemas/agent/action-tool-v1.json",
    "Action JSON Schema id drifted",
  );
  for (const name of [
    "ToolArgumentsV1",
    "ModelToolRequestV1",
    "ProjectToolBindingV1",
    "ToolRequestV1",
    "ToolResultDataV1",
    "ToolResultV1",
    "AgentActionRecordContextV1",
    "AgentActionRecordV1",
  ]) {
    assert(schema.$defs?.[name]?.additionalProperties === false, `Schema missing strict ${name}`);
  }
  for (const variant of schema.$defs.AgentActionPayloadV1.oneOf) {
    assert(variant.additionalProperties === false, "Action payload variant is not strict");
    assert(variant.properties.schemaVersion.const === 1, "Action version is not fixed");
  }

  const typescript = await readFile(path.join(CONTRACT_ROOT, "action-types.ts"), "utf8");
  for (const declaration of [
    "type AgentActionPayloadV1",
    "interface ToolRequestV1",
    "interface ToolResultV1",
    "interface AgentActionRecordV1",
    'trust: "untrusted_tool_data"',
    "redacted: true",
  ]) {
    assert(typescript.includes(declaration), `TypeScript mirror missing ${declaration}`);
  }
  assert(!typescript.includes(": any"), "Action TypeScript mirror must not contain any");

  for (const fixture of EXPECTED_RESOURCES.filter((item) => item.startsWith("fixtures/"))) {
    const value = JSON.parse(await readFile(path.join(CONTRACT_ROOT, fixture), "utf8"));
    assert(value.schemaVersion === 1, `Fixture version drifted: ${fixture}`);
  }
}

async function main() {
  const mode = process.argv[2];
  assert(mode === "--write" || mode === "--check", "Use --write or --check");
  const resources = await materializeResources();
  const computedHash = contractSetHash(resources);
  const manifestPath = path.join(CONTRACT_ROOT, MANIFEST_PATH);
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  assert(manifest.schemaVersion === 1, "Action manifest schema version drifted");
  assert(
    manifest.contractSetId === "novelgainker.agent-action-tool-provider",
    "Action contract set id drifted",
  );
  assert(manifest.version === "1.0.0", "Action contract set version drifted");
  assert(manifest.enabledAtRuntime === false, "NG-A003 contracts must remain runtime-disabled");

  if (mode === "--write") {
    manifest.resources = resources;
    manifest.contractSetHash = computedHash;
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  } else {
    assert(
      JSON.stringify(manifest.resources) === JSON.stringify(resources),
      "Action lock is stale",
    );
    assert(manifest.contractSetHash === computedHash, "Action contract set hash is stale");
  }
  await validateMirrors();
  console.log(
    `[agent-action-contract-lock] PASS ${resources.length} locked resources, contract ${computedHash}`,
  );
}

await main();
