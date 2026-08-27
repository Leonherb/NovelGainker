import { describe, expect, it, vi } from "vitest";
import {
  parseStoryContinuityReport,
  requestStoryContinuityCheck,
  requestStoryContinuityWarningConfirm,
} from "./story-continuity";

const HASH = "a".repeat(64);
const result = (ruleId: string, severity: "blocking" | "warning" | "unknown" | "passed") => ({
  ruleId,
  severity,
  title: "规则",
  explanation: "本地解释",
  evidence: ["evidence=value"],
  sourceIds: [],
  suggestedAction: "人工复核。",
});
function report(): import("./story-continuity").StoryContinuityReport {
  const results = Array.from({ length: 17 }, (_, index) =>
    result(`rule.${index}`, index === 0 ? "warning" : index === 1 ? "unknown" : "passed"),
  );
  return {
    schemaVersion: 1,
    reportHash: HASH,
    proposalHash: HASH,
    sourceManifestHash: HASH,
    currentManifestHash: HASH,
    generationTargetHash: null,
    currentTargetHash: null,
    checkedAt: 10,
    localOnly: true,
    results,
    summary: { blocking: 0, warning: 1, unknown: 1, passed: 15 },
    acknowledgedWarningRuleIds: [],
    acknowledgedUnknownRuleIds: [],
    warningsConfirmed: false,
    unknownsConfirmed: false,
    reviewReady: false,
    writeEnabled: false,
  };
}
const input = {
  runId: "run_fixture",
  proposalHash: HASH,
  action: "create" as const,
  chapterNumber: 2,
  chapterTitle: "潮门",
  selectedSourcePaths: ["plot/main.md"],
  reviewedManifestHash: HASH,
  readerMentalModel: {
    readerKnows: "已知",
    readerInfers: "",
    readerMisreads: "",
    readerQuestions: "",
    chapterPromises: "",
    forbiddenReveals: "",
  },
};

describe("NG-504 continuity strict IPC", () => {
  it("accepts four severities but never accepts writeEnabled or fake unknown-as-passed", () => {
    expect(parseStoryContinuityReport(report()).summary.unknown).toBe(1);
    expect(() => parseStoryContinuityReport({ ...report(), writeEnabled: true })).toThrow();
    expect(() =>
      parseStoryContinuityReport({ ...report(), results: report().results.slice(0, 16) }),
    ).toThrow();
  });
  it("sends only reviewed local context and no Provider configuration", async () => {
    const invokeCommand = vi.fn().mockResolvedValue(report());
    await requestStoryContinuityCheck("session_fixture", input, invokeCommand);
    expect(invokeCommand).toHaveBeenCalledWith("story_continuity_check", expect.any(Object));
    expect(JSON.stringify(invokeCommand.mock.calls)).not.toMatch(
      /apiKey|endpoint|baseUrl|modelId/iu,
    );
  });
  it("confirms exactly the current warning and unknown IDs without reclassifying unknown", async () => {
    const confirmed = {
      ...report(),
      acknowledgedWarningRuleIds: ["rule.0"],
      acknowledgedUnknownRuleIds: ["rule.1"],
      warningsConfirmed: true,
      unknownsConfirmed: true,
      reviewReady: true,
    };
    const invokeCommand = vi.fn().mockResolvedValue(confirmed);
    await requestStoryContinuityWarningConfirm(
      "session_fixture",
      "run_fixture",
      report(),
      invokeCommand,
    );
    expect(invokeCommand).toHaveBeenCalledWith("story_continuity_warning_confirm", {
      sessionId: "session_fixture",
      input: {
        schemaVersion: 1,
        runId: "run_fixture",
        reportHash: HASH,
        warningRuleIds: ["rule.0"],
        unknownRuleIds: ["rule.1"],
        confirmed: true,
      },
    });
    expect(parseStoryContinuityReport(confirmed).summary.unknown).toBe(1);
  });
});
