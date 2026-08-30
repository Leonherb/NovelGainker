import { spawnSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const result = spawnSync(
  "cargo",
  ["run", "-p", "app-core", "--example", "gar1_scale", "--locked", "--offline"],
  { cwd: process.cwd(), encoding: "utf8", env: { ...process.env, CARGO_NET_OFFLINE: "true" } },
);
if (result.status !== 0) {
  process.stderr.write(result.stderr ?? "");
  process.stdout.write(result.stdout ?? "");
  process.exit(result.status ?? 1);
}
const line = (result.stdout ?? "")
  .split(/\r?\n/u)
  .find((value) => value.startsWith("GAR1_SCALE_EVIDENCE:"));
if (!line) throw new Error("GAR1 scale evidence missing");
const evidence = JSON.parse(line.slice("GAR1_SCALE_EVIDENCE:".length));
if (evidence.passed !== true) throw new Error("GAR1 scale evidence failed");
const directory = path.join("target", "qa", "ng-r104");
await mkdir(directory, { recursive: true });
await writeFile(
  path.join(directory, "ng-r104-evidence.json"),
  `${JSON.stringify(evidence, null, 2)}\n`,
  "utf8",
);
console.log(
  `[e2e:ngr104] PASS ${evidence.fixture.chapterCount} chapters, ${evidence.fixture.chineseCharacterCount} Chinese chars, p95 ${evidence.performance.hotQueryP95Ms.toFixed(3)} ms`,
);
