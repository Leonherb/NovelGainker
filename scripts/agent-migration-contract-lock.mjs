import { createHash } from "node:crypto";
import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const ROOT = process.cwd();
const CONTRACT_ROOT = path.join(ROOT, "resources", "contracts", "agent", "v1");
const MANIFEST_PATH = "contract-set.json";
const EXPECTED_RESOURCES = [
  "contract-types.ts",
  "fixtures/cross-project-turn.json",
  "fixtures/valid-session.json",
  "fixtures/valid-turn.json",
  "session-turn.schema.json",
];

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

async function collectFiles(directory = CONTRACT_ROOT) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectFiles(absolutePath)));
    } else {
      assert(entry.isFile(), `Unsupported contract resource: ${absolutePath}`);
      files.push(path.relative(CONTRACT_ROOT, absolutePath).split(path.sep).join("/"));
    }
  }
  return files.sort(comparePaths);
}

function validateText(resourcePath, bytes) {
  assert(
    !(bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf),
    `BOM forbidden: ${resourcePath}`,
  );
  assert(!bytes.includes(0x0d), `CRLF forbidden: ${resourcePath}`);
  new TextDecoder("utf-8", { fatal: true }).decode(bytes);
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
    `Contract resource set drifted: ${actual.join(", ")}`,
  );
  const resources = [];
  for (const resourcePath of actual) {
    const bytes = await readFile(path.join(CONTRACT_ROOT, resourcePath));
    validateText(resourcePath, bytes);
    resources.push({ path: resourcePath, bytes: bytes.length, sha256: sha256(bytes) });
  }
  return resources;
}

async function validateMirrors() {
  const schema = JSON.parse(
    await readFile(path.join(CONTRACT_ROOT, "session-turn.schema.json"), "utf8"),
  );
  assert(
    schema.$schema === "https://json-schema.org/draft/2020-12/schema",
    "JSON Schema dialect drifted",
  );
  for (const name of [
    "ProjectAgentSessionV1",
    "AgentTurnV1",
    "ProjectAgentBasePolicyV1",
    "TurnBudgetV1",
  ]) {
    assert(schema.$defs?.[name]?.additionalProperties === false, `Schema missing strict ${name}`);
  }
  const typescript = await readFile(path.join(CONTRACT_ROOT, "contract-types.ts"), "utf8");
  for (const declaration of [
    "interface ProjectAgentSessionV1",
    "interface AgentTurnV1",
    "interface ProjectAgentBasePolicyV1",
    "allowsBypassChat: false",
    "maxWriteTransactions: 1",
  ]) {
    assert(typescript.includes(declaration), `TypeScript mirror missing ${declaration}`);
  }
  assert(!typescript.includes(": any"), "TypeScript mirror must not contain any");

  for (const fixture of ["valid-session.json", "valid-turn.json", "cross-project-turn.json"]) {
    const value = JSON.parse(await readFile(path.join(CONTRACT_ROOT, "fixtures", fixture), "utf8"));
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
  assert(manifest.schemaVersion === 1, "Contract manifest schema version drifted");
  assert(
    manifest.contractSetId === "novelgainker.project-agent-session-turn",
    "Contract set id drifted",
  );
  assert(manifest.version === "1.0.0", "Contract set version drifted");
  assert(manifest.enabledAtRuntime === false, "NG-A002 contracts must remain runtime-disabled");

  if (mode === "--write") {
    manifest.resources = resources;
    manifest.contractSetHash = computedHash;
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  } else {
    assert(
      JSON.stringify(manifest.resources) === JSON.stringify(resources),
      "Contract lock is stale",
    );
    assert(manifest.contractSetHash === computedHash, "Contract set hash is stale");
  }
  await validateMirrors();
  console.log(
    `[agent-contract-lock] PASS ${resources.length} locked resources, contract ${computedHash}`,
  );
}

await main();
