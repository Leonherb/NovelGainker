import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync, rmdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const workspaceRoot = process.cwd();
const qualityScript = resolve(workspaceRoot, "scripts/quality.mjs");
const reportPath = resolve(workspaceRoot, "target/qa/ng-004-failure-gates.json");
const rustTestDirectory = resolve(workspaceRoot, "crates/app-core/tests");
const rustTestDirectoryExisted = existsSync(rustTestDirectory);

const probes = [
  {
    name: "frontend-format",
    path: "apps/desktop/src/__quality_format_probe__.ts",
    content: "export const badlyFormatted={answer:42}\n",
    expectedStage: "Frontend format check",
  },
  {
    name: "typescript-type",
    path: "apps/desktop/src/__quality_type_probe__.ts",
    content: "export const qualityTypeProbe: string = 42;\n",
    expectedStage: "TypeScript type check",
  },
  {
    name: "react-unit-test",
    path: "apps/desktop/src/__quality_react_probe__.test.ts",
    content:
      'import { expect, it } from "vitest";\n\nit("quality failure probe", () => {\n  expect(true).toBe(false);\n});\n',
    expectedStage: "React unit tests",
  },
  {
    name: "rust-test",
    path: "crates/app-core/tests/__quality_rust_probe__.rs",
    content:
      '#[test]\nfn quality_failure_probe() {\n    assert!(std::hint::black_box(false), "intentional quality probe");\n}\n',
    expectedStage: "Rust tests",
  },
];

const results = [];

for (const probe of probes) {
  const absolutePath = resolve(workspaceRoot, probe.path);
  if (existsSync(absolutePath)) {
    throw new Error(`Refusing to overwrite existing probe path: ${probe.path}`);
  }

  mkdirSync(dirname(absolutePath), { recursive: true });
  let run;
  try {
    writeFileSync(absolutePath, probe.content, "utf8");
    run = spawnSync(process.execPath, [qualityScript], {
      cwd: workspaceRoot,
      encoding: "utf8",
      env: process.env,
      maxBuffer: 20 * 1024 * 1024,
    });
  } finally {
    rmSync(absolutePath);
  }

  const output = `${run?.stdout ?? ""}\n${run?.stderr ?? ""}`;
  const expectedMarker = `[quality] FAIL ${probe.expectedStage}`;
  if (run?.status === 0 || !output.includes(expectedMarker)) {
    throw new Error(
      `${probe.name} did not block at ${probe.expectedStage}. Exit: ${run?.status}\n${output.slice(-2000)}`,
    );
  }

  results.push({
    name: probe.name,
    expectedStage: probe.expectedStage,
    exitCode: run.status,
    blocked: true,
  });
  console.log(
    `[gate-probe] PASS ${probe.name} blocked at ${probe.expectedStage} with exit ${run.status}`,
  );
}

if (!rustTestDirectoryExisted && existsSync(rustTestDirectory)) {
  rmdirSync(rustTestDirectory);
}

mkdirSync(dirname(reportPath), { recursive: true });
writeFileSync(
  reportPath,
  `${JSON.stringify({ generatedAt: new Date().toISOString(), results }, null, 2)}\n`,
  "utf8",
);
console.log(`[gate-probe] PASS report written to ${reportPath}`);
