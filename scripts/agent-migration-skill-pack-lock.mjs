import { createHash } from "node:crypto";
import { readdir, readFile, lstat, writeFile } from "node:fs/promises";
import path from "node:path";

const PACK_ROOT = path.resolve("resources", "skills", "novelist", "v1");
const INVENTORY_PATH = "inventory.json";
const ALLOWED_EXTENSIONS = new Set([".json", ".md", ".upstream"]);
const FORBIDDEN_SEGMENTS = new Set([
  ".claude-plugin",
  ".codex-plugin",
  "agents",
  "scripts",
  "submission",
  "tests",
  "website",
]);

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function normalizePath(absolutePath) {
  return path.relative(PACK_ROOT, absolutePath).split(path.sep).join("/");
}

function validateResourcePath(resourcePath) {
  const segments = resourcePath.split("/");
  if (
    resourcePath.length === 0 ||
    resourcePath.startsWith("/") ||
    resourcePath.includes("\\") ||
    resourcePath.includes(":") ||
    segments.some(
      (segment) =>
        segment.length === 0 ||
        segment === "." ||
        segment === ".." ||
        FORBIDDEN_SEGMENTS.has(segment.toLowerCase()),
    )
  ) {
    throw new Error(`skill-pack.resource-path-forbidden:${resourcePath}`);
  }
  if (!ALLOWED_EXTENSIONS.has(path.posix.extname(resourcePath).toLowerCase())) {
    throw new Error(`skill-pack.resource-extension-forbidden:${resourcePath}`);
  }
}

function validateText(resourcePath, bytes) {
  if (bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) {
    throw new Error(`skill-pack.resource-bom-forbidden:${resourcePath}`);
  }
  if (bytes.includes(0x0d)) {
    throw new Error(`skill-pack.resource-line-ending-invalid:${resourcePath}`);
  }
  new TextDecoder("utf-8", { fatal: true }).decode(bytes);
}

async function collectFiles(directory = PACK_ROOT) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const absolutePath = path.join(directory, entry.name);
    const metadata = await lstat(absolutePath);
    if (metadata.isSymbolicLink()) {
      throw new Error(`skill-pack.resource-symlink-forbidden:${normalizePath(absolutePath)}`);
    }
    if (metadata.isDirectory()) {
      files.push(...(await collectFiles(absolutePath)));
      continue;
    }
    if (!metadata.isFile()) {
      throw new Error(`skill-pack.resource-type-forbidden:${normalizePath(absolutePath)}`);
    }
    const resourcePath = normalizePath(absolutePath);
    validateResourcePath(resourcePath);
    files.push({ absolutePath, resourcePath });
  }
  return files.sort((left, right) =>
    left.resourcePath < right.resourcePath ? -1 : left.resourcePath > right.resourcePath ? 1 : 0,
  );
}

async function parseJson(resourcePath) {
  return JSON.parse(await readFile(path.join(PACK_ROOT, resourcePath), "utf8"));
}

function serializedJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

async function refreshManifestHashes(pack, checkOnly) {
  for (const manifestPath of pack.skillManifests) {
    const absoluteManifestPath = path.join(PACK_ROOT, manifestPath);
    const manifest = await parseJson(manifestPath);
    const resources = [
      ...manifest.instructionResources,
      ...manifest.templateResources,
      ...manifest.referenceResources,
    ];
    if (new Set(resources).size !== resources.length) {
      throw new Error(`skill-pack.manifest-resource-duplicate:${manifestPath}`);
    }
    const hashes = {};
    for (const resourcePath of [...resources].sort((left, right) =>
      left < right ? -1 : left > right ? 1 : 0,
    )) {
      validateResourcePath(resourcePath);
      const bytes = await readFile(path.join(PACK_ROOT, resourcePath));
      validateText(resourcePath, bytes);
      hashes[resourcePath] = sha256(bytes);
    }
    manifest.resourceHashes = hashes;
    const expected = serializedJson(manifest);
    const actual = await readFile(absoluteManifestPath, "utf8");
    if (checkOnly && actual !== expected) {
      throw new Error(`skill-pack.manifest-hash-lock-stale:${manifestPath}`);
    }
    if (!checkOnly && actual !== expected) {
      await writeFile(absoluteManifestPath, expected, "utf8");
    }
  }
}

async function buildInventory() {
  const files = (await collectFiles()).filter(
    ({ resourcePath }) => resourcePath !== INVENTORY_PATH,
  );
  const inventoryFiles = [];
  for (const { absolutePath, resourcePath } of files) {
    const bytes = await readFile(absolutePath);
    validateText(resourcePath, bytes);
    inventoryFiles.push({
      path: resourcePath,
      sha256: sha256(bytes),
      bytes: bytes.length,
    });
  }
  return {
    schemaVersion: 1,
    algorithm: "sha256",
    excludedResources: [INVENTORY_PATH],
    files: inventoryFiles,
  };
}

async function main() {
  const checkOnly = process.argv.includes("--check");
  const writeLock = process.argv.includes("--write");
  if (checkOnly === writeLock) {
    throw new Error("Use exactly one of --check or --write.");
  }

  const pack = await parseJson("pack.json");
  await refreshManifestHashes(pack, checkOnly);
  const inventory = await buildInventory();
  const expected = serializedJson(inventory);
  const inventoryAbsolutePath = path.join(PACK_ROOT, INVENTORY_PATH);
  let actual = null;
  try {
    actual = await readFile(inventoryAbsolutePath, "utf8");
  } catch (error) {
    if (error?.code !== "ENOENT") {
      throw error;
    }
  }

  if (checkOnly && actual !== expected) {
    throw new Error("skill-pack.inventory-lock-stale");
  }
  if (writeLock && actual !== expected) {
    await writeFile(inventoryAbsolutePath, expected, "utf8");
  }

  console.log(
    `[skill-pack-lock] ${checkOnly ? "PASS" : "UPDATED"} ${inventory.files.length} locked resources`,
  );
}

await main();
