import { describe, expect, it, vi } from "vitest";
import {
  parseChapterProposal,
  parseStoryChapterRun,
  requestStoryChapterGenerateStart,
} from "./story-chapter";

const MANIFEST_HASH = "a".repeat(64);
const REQUEST_HASH = "b".repeat(64);

function proposal(overrides: Record<string, unknown> = {}) {
  return {
    proposalVersion: 1,
    proposalHash: "c".repeat(64),
    action: "create",
    targetChapter: {
      chapterNumber: 2,
      chapterTitle: "潮痕",
      relativePath: "chapters/002.md",
    },
    title: "潮痕",
    draftMarkdown: "潮水退去，石阶上留下发光的纹路。",
    intentSummary: "推进潮门异响线索。",
    readerModelDelta: {
      readerKnows: "读者看见了发光纹路。",
      readerInfers: "纹路可能与潮门有关。",
      readerMisreads: "",
      readerQuestions: "谁留下了纹路？",
      chapterPromises: "继续追查纹路。",
      forbiddenReveals: "",
    },
    continuityClaims: [{ claim: "潮门已关闭", sourceIds: ["project:project.md"] }],
    ledgerCandidates: [{ summary: "石阶出现发光纹路", sourceIds: ["project:project.md"] }],
    sourceManifestHash: MANIFEST_HASH,
    currentManifestHash: MANIFEST_HASH,
    requestHash: REQUEST_HASH,
    providerId: "deepseek",
    modelId: "deepseek-v4-pro",
    startedAt: 100,
    generatedAt: 101,
    finishReason: "stop",
    responseComplete: true,
    sourceStatus: "current",
    continuityCheckVersion: null,
    writeEnabled: false,
    ...overrides,
  };
}

function generatingRun() {
  return {
    schemaVersion: 1,
    runId: "10000000-0000-4000-8000-000000000001",
    state: "generating",
    providerId: "deepseek",
    modelId: "deepseek-v4-pro",
    sourceManifestHash: MANIFEST_HASH,
    requestHash: REQUEST_HASH,
    startedAt: 100,
    completedAt: null,
    repairAttempted: false,
    canCancel: true,
    canRetry: false,
    proposal: null,
    invalidResponsePreview: null,
    error: null,
  };
}

describe("NG-503 Story Chapter strict IPC", () => {
  it("accepts only a complete current read-only proposal with fixed provider/model trace", () => {
    expect(parseChapterProposal(proposal())).toMatchObject({
      writeEnabled: false,
      continuityCheckVersion: null,
      providerId: "deepseek",
      modelId: "deepseek-v4-pro",
      sourceStatus: "current",
    });
    expect(() => parseChapterProposal(proposal({ writeEnabled: true }))).toThrow();
    expect(() => parseChapterProposal(proposal({ applyPath: "chapters/002.md" }))).toThrow();
    expect(() =>
      parseChapterProposal(proposal({ sourceStatus: "stale", currentManifestHash: MANIFEST_HASH })),
    ).toThrow();
    expect(() => parseChapterProposal(proposal({ modelId: "user-selected-model" }))).toThrow();
  });

  it("rejects impossible run states and any secret-shaped extra field", () => {
    expect(parseStoryChapterRun(generatingRun())).toMatchObject({
      state: "generating",
      canCancel: true,
    });
    expect(() => parseStoryChapterRun({ ...generatingRun(), apiKey: "secret" })).toThrow();
    expect(() =>
      parseStoryChapterRun({ ...generatingRun(), state: "completed", canCancel: false }),
    ).toThrow();
  });

  it("sends reviewed context and hashes but no key, endpoint, or model override", async () => {
    const invokeCommand = vi.fn().mockResolvedValue(generatingRun());
    await requestStoryChapterGenerateStart(
      "session_fixture",
      {
        action: "create",
        chapterNumber: 2,
        chapterTitle: "潮痕",
        selectedSourcePaths: ["plot/main.md"],
        reviewedManifestHash: MANIFEST_HASH,
        readerMentalModel: {
          readerKnows: "潮门已关闭。",
          readerInfers: "",
          readerMisreads: "",
          readerQuestions: "",
          chapterPromises: "推进异响线索。",
          forbiddenReveals: "",
        },
        expectedRequestHash: REQUEST_HASH,
      },
      invokeCommand,
    );
    expect(invokeCommand).toHaveBeenCalledWith("story_chapter_generate_start", {
      sessionId: "session_fixture",
      input: expect.objectContaining({
        schemaVersion: 1,
        expectedRequestHash: REQUEST_HASH,
        expectedProviderId: null,
        confirmed: true,
      }),
    });
    expect(JSON.stringify(invokeCommand.mock.calls)).not.toMatch(
      /apiKey|baseUrl|endpoint|modelId/iu,
    );
  });
});
