import { spawnSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const result = spawnSync(
  "cargo",
  ["run", "-p", "app-core", "--example", "gar2_scale", "--locked", "--offline"],
  {
    cwd: process.cwd(),
    encoding: "utf8",
    env: { ...process.env, CARGO_NET_OFFLINE: "true" },
  },
);
if (result.status !== 0) {
  process.stderr.write(result.stderr ?? "");
  process.stdout.write(result.stdout ?? "");
  process.exit(result.status ?? 1);
}
const line = (result.stdout ?? "")
  .split(/\r?\n/u)
  .find((value) => value.startsWith("GAR2_SCALE_EVIDENCE:"));
if (!line) throw new Error("GAR2 scale evidence missing");
const evidence = JSON.parse(line.slice("GAR2_SCALE_EVIDENCE:".length));
if (
  evidence.passed !== true ||
  evidence.fixture.chapterCount < 120 ||
  evidence.fixture.chineseCharacterCount < 500_000 ||
  evidence.fixture.truthQueryCount < 48 ||
  evidence.fixture.writeScenarioCount < 72 ||
  evidence.budgetObservation.compositionCalls !== 0 ||
  evidence.budgetObservation.segmentCount !== 0
) {
  throw new Error("GAR2 scale evidence failed frozen thresholds");
}
const directory = path.join("target", "qa", "ng-r208");
await mkdir(directory, { recursive: true });
await writeFile(
  path.join(directory, "ng-r208-evidence.json"),
  `${JSON.stringify(evidence, null, 2)}\n`,
  "utf8",
);
console.log(
  `[e2e:ngr208] PASS ${evidence.fixture.chapterCount} chapters, ${evidence.fixture.chineseCharacterCount} Chinese chars, ${evidence.fixture.writeScenarioCount} write scenarios`,
);
