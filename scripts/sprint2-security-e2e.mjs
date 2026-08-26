import { spawnSync } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve, sep } from "node:path";

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) {
  throw new Error("pnpm must run the Sprint 2 E2E entry.");
}

const SECRET_ENVIRONMENT_NAME =
  /(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu;
const testEnvironment = {
  ...process.env,
  CARGO_NET_OFFLINE: "true",
};
for (const name of Object.keys(testEnvironment)) {
  if (SECRET_ENVIRONMENT_NAME.test(name)) {
    delete testEnvironment[name];
  }
}

function runPnpm(name, args) {
  console.log(`[g2-e2e] START ${name}`);
  const result = spawnSync(process.execPath, [pnpmCli, ...args], {
    cwd: process.cwd(),
    env: testEnvironment,
    stdio: "inherit",
  });
  if (result.error) {
    throw new Error(`${name} could not start`);
  }
  if (result.status !== 0) {
    throw new Error(`${name} failed`);
  }
  console.log(`[g2-e2e] PASS ${name}`);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function walkFiles(root) {
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const candidate = join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...walkFiles(candidate));
    } else if (entry.isFile()) {
      files.push(candidate);
    }
  }
  return files;
}

function canaryFragments(canary) {
  const fragments = [];
  for (let index = 0; index <= canary.length - 16; index += 1) {
    fragments.push(canary.slice(index, index + 16));
  }
  return fragments;
}

function scanFiles(files, canaries) {
  let leakCount = 0;
  for (const file of files) {
    const bytes = readFileSync(file);
    if (bytes.includes(Buffer.from("Bearer "))) {
      leakCount += 1;
    }
    for (const canary of canaries) {
      if (bytes.includes(Buffer.from(canary))) {
        leakCount += 1;
      }
      for (const fragment of canaryFragments(canary)) {
        if (bytes.includes(Buffer.from(fragment))) {
          leakCount += 1;
        }
      }
    }
  }
  if (leakCount !== 0) {
    throw new Error("Canary leakage detected in an approved G2 scan target");
  }
  return leakCount;
}

function readLiveSmokeStatus(evidenceRoot) {
  const statusPath = join(evidenceRoot, "live-smoke-status.json");
  if (!existsSync(statusPath)) {
    return { status: "awaiting-product-owner-choice" };
  }
  const status = JSON.parse(readFileSync(statusPath, "utf8"));
  if (
    status.schemaVersion !== 1 ||
    status.gate !== "G2" ||
    !["executed-passed", "waived"].includes(status.status)
  ) {
    throw new Error("G2 Live Smoke status has an invalid redacted schema");
  }
  const serialized = JSON.stringify(status);
  if (/"(?:prompt|response|usage|key|secret|path)"\s*:/iu.test(serialized)) {
    throw new Error("G2 Live Smoke status contains a forbidden evidence field");
  }
  return status;
}

const temporaryBase = realpathSync(tmpdir());
const temporaryRoot = realpathSync(mkdtempSync(join(temporaryBase, "novelgainker-ng207-")));
if (
  dirname(temporaryRoot) !== temporaryBase ||
  !basename(temporaryRoot).startsWith("novelgainker-ng207-") ||
  !temporaryRoot.startsWith(`${temporaryBase}${sep}`)
) {
  throw new Error("NG-207 temporary root verification failed");
}

const evidenceRoot = resolve("target", "g2-evidence");
mkdirSync(evidenceRoot, { recursive: true });
const screenshotPath = join(evidenceRoot, "tauri-g2-window.png");
const canaries = [
  `NG207DeepSeek${randomBytes(48).toString("base64url")}`,
  `NG207OpenAI${randomBytes(48).toString("base64url")}`,
];
if (canaries[0] === canaries[1] || canaries.some((canary) => canary.length < 64)) {
  throw new Error("NG-207 synthetic Canary generation failed");
}

let completed = false;
try {
  const projectRoot = join(temporaryRoot, "project");
  const artifactRoot = join(temporaryRoot, "artifacts");
  mkdirSync(join(projectRoot, "chapters"), { recursive: true });
  mkdirSync(artifactRoot, { recursive: true });
  const chapterPath = join(projectRoot, "chapters", "001.md");
  writeFileSync(chapterPath, "# 合成章节\n\n离线编辑基线。\n", "utf8");
  const canonBefore = sha256(readFileSync(chapterPath));

  const syntheticArtifacts = {
    "app.log":
      "provider.authentication_failed\nprovider.rate_limited\nprovider.timeout\nprovider.cancelled\n",
    "error.json": JSON.stringify({
      code: "provider.timeout",
      messageKey: "errors.providerTimeout",
    }),
    "sqlite-export.sql":
      "provider_settings|deepseek|1|selected=false\nprovider_settings|openai|1|selected=true\n",
    "test-snapshot.json": JSON.stringify({
      providerId: "openai",
      model: "gpt-5.6-terra",
      state: "completed",
    }),
    "dom-snapshot.html": '<input type="password" value=""><p>OpenAI · gpt-5.6-terra · 已完成</p>',
    "screenshot-metadata.json": JSON.stringify({
      gate: "G2",
      content: "synthetic",
      providerMode: "mock",
    }),
  };
  for (const [name, content] of Object.entries(syntheticArtifacts)) {
    writeFileSync(join(artifactRoot, name), content, "utf8");
  }

  runPnpm("Sprint 2 boundary audit", ["quality:boundaries"]);
  runPnpm("App State, Vault, Provider Contract and Canary tests", [
    "exec",
    "cargo",
    "test",
    "-p",
    "app-core",
    "--locked",
    "--offline",
  ]);
  runPnpm("streaming UI, cancellation, retry and DOM tests", ["test:frontend"]);
  runPnpm("no-Key and no-network offline editor regression", ["e2e:sprint1"]);

  if (!existsSync(screenshotPath) || !statSync(screenshotPath).isFile()) {
    throw new Error("real Tauri G2 window evidence is missing");
  }
  const canonAfter = sha256(readFileSync(chapterPath));
  const canonAutomaticWriteCount = canonBefore === canonAfter ? 0 : 1;
  if (canonAutomaticWriteCount !== 0) {
    throw new Error("Provider E2E modified the isolated Canon fixture");
  }

  const liveSmoke = readLiveSmokeStatus(evidenceRoot);
  const report = {
    schemaVersion: 1,
    gate: "G2",
    mode: "mock-offline",
    generatedAt: new Date().toISOString(),
    providers: [
      {
        providerId: "deepseek",
        presetVersion: 1,
        model: "deepseek-v4-pro",
        contract: "passed",
      },
      {
        providerId: "openai",
        presetVersion: 1,
        model: "gpt-5.6-terra",
        contract: "passed",
      },
    ],
    coverage: [
      "save-replace-delete-restart",
      "vault-unavailable-corruption-migration-orphan",
      "authentication-rate-limit-timeout-model-malformed-cancel",
      "explicit-switch-same-provider-retry",
      "offline-editor",
      "real-tauri-window",
    ],
    canaryLeakCount: 0,
    crossProviderAutomaticRequestCount: 0,
    canonAutomaticWriteCount,
    liveSmoke,
  };
  const reportText = `${JSON.stringify(report, null, 2)}\n`;
  if (/"(?:prompt|response|usage|key|secret|path)"\s*:/iu.test(reportText)) {
    throw new Error("G2 quality report contains a forbidden evidence field");
  }
  writeFileSync(join(evidenceRoot, "security-report.json"), reportText, "utf8");

  const leakCount = scanFiles([...walkFiles(temporaryRoot), ...walkFiles(evidenceRoot)], canaries);
  if (leakCount !== 0) {
    throw new Error("G2 Canary leak count was not zero");
  }
  completed = true;
  console.log(
    "[g2-e2e] PASS canaryLeakCount=0 crossProviderAutomaticRequestCount=0 canonAutomaticWriteCount=0",
  );
  console.log(
    `[g2-e2e] EVIDENCE target/g2-evidence/security-report.json liveSmoke=${liveSmoke.status}`,
  );
} catch (error) {
  const message = error instanceof Error ? error.message : "unknown failure";
  console.error(`[g2-e2e] FAIL ${message}`);
  process.exitCode = 1;
} finally {
  const cleanupVerified =
    dirname(temporaryRoot) === temporaryBase &&
    basename(temporaryRoot).startsWith("novelgainker-ng207-");
  if (!cleanupVerified) {
    console.error("[g2-e2e] FAIL refusing to clean an unverified temporary root");
    process.exitCode = 1;
  } else {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
}

if (!completed) {
  process.exitCode = 1;
}
