import { createHash } from "node:crypto";
import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const ROOT = process.cwd();
const CONTRACT_ROOT = path.join(ROOT, "resources", "contracts", "agent-governance", "v1");
const MANIFEST_PATH = "contract-set.json";
const EXPECTED_RESOURCES = [
  "fixtures/invalid-cross-project.json",
  "fixtures/invalid-plaintext-full-chat.json",
  "fixtures/invalid-sensitive-canary.json",
  "fixtures/invalid-stale-confirmation.json",
  "fixtures/invalid-undo-over-limit.json",
  "fixtures/valid-persistence-snapshot.json",
  "fixtures/valid-transaction-plan.json",
  "governance-types.ts",
  "governance.schema.json",
  "risk-matrix.json",
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
      assert(entry.isFile(), `Unsupported governance resource: ${absolutePath}`);
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
    `Governance resource set drifted: ${actual.join(", ")}`,
  );
  const resources = [];
  for (const resourcePath of actual) {
    const bytes = await readFile(path.join(CONTRACT_ROOT, resourcePath));
    assert(!bytes.includes(0x0d), `CRLF forbidden: ${resourcePath}`);
    new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    resources.push({
      path: resourcePath,
      bytes: bytes.length,
      sha256: sha256(bytes),
    });
  }
  return resources;
}

async function validateMirrors() {
  const schema = JSON.parse(
    await readFile(path.join(CONTRACT_ROOT, "governance.schema.json"), "utf8"),
  );
  assert(
    schema.$schema === "https://json-schema.org/draft/2020-12/schema",
    "Governance JSON Schema dialect drifted",
  );
  assert(
    schema.$id === "https://novelgainker.local/schemas/agent/governance-v1.json",
    "Governance JSON Schema id drifted",
  );
  for (const name of [
    "AuthorizationDecisionV1",
    "TurnAuthorizationV1",
    "ConfirmationBindingV1",
    "HighRiskConfirmationV1",
    "FullChatPersistenceV1",
    "SessionPersistenceSnapshotV1",
    "UndoRecordV1",
    "UndoCatalogV1",
    "MultiFileTransactionPlanV1",
    "TransactionReceiptV1",
  ]) {
    assert(schema.$defs?.[name]?.additionalProperties === false, `Schema missing strict ${name}`);
  }

  const matrix = JSON.parse(await readFile(path.join(CONTRACT_ROOT, "risk-matrix.json"), "utf8"));
  assert(matrix.schemaVersion === 1, "Risk matrix version drifted");
  assert(matrix.policies.length === 3, "Authorization policy count drifted");
  assert(matrix.riskLevels.length === 4, "Risk level count drifted");
  assert(matrix.registeredTools.length === 6, "Registered Tool count drifted");
  assert(matrix.actions.length === 21, "Governed action count drifted");
  assert(
    new Set(matrix.registeredTools.map(({ id }) => id)).size === 6,
    "Registered Tool risk assignment is not unique",
  );
  assert(
    new Set(matrix.actions.map(({ id }) => id)).size === 21,
    "Governed action risk assignment is not unique",
  );

  const typescript = await readFile(path.join(CONTRACT_ROOT, "governance-types.ts"), "utf8");
  for (const declaration of [
    "type AuthorizationPolicyV1",
    "type RiskLevelV1",
    "interface TurnAuthorizationV1",
    "interface HighRiskConfirmationV1",
    "interface SessionPersistenceSnapshotV1",
    "interface MultiFileTransactionPlanV1",
    "interface UndoRecordV1",
    "windows_user_context",
  ]) {
    assert(typescript.includes(declaration), `TypeScript mirror missing ${declaration}`);
  }
  assert(!typescript.includes(": any"), "TypeScript mirror contains unsafe any");
  assert(
    !typescript.includes("plain" + "text"),
    "TypeScript mirror names an unsupported persistence mode",
  );

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
  assert(manifest.schemaVersion === 1, "Governance manifest version drifted");
  assert(
    manifest.contractSetId === "novelgainker.agent-governance-persistence-transaction-undo",
    "Governance contract set id drifted",
  );
  assert(manifest.version === "1.0.0", "Governance contract version drifted");
  assert(manifest.enabledAtRuntime === false, "NG-A004 contracts must remain runtime-disabled");

  if (mode === "--write") {
    manifest.resources = resources;
    manifest.contractSetHash = computedHash;
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  } else {
    assert(
      JSON.stringify(manifest.resources) === JSON.stringify(resources),
      "Governance lock is stale",
    );
    assert(manifest.contractSetHash === computedHash, "Governance contract set hash is stale");
  }
  await validateMirrors();
  console.log(
    `[agent-governance-contract-lock] PASS ${resources.length} locked resources, contract ${computedHash}`,
  );
}

await main();
