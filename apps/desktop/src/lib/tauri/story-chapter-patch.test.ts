import { describe, expect, it, vi } from "vitest";
import {
  parseChapterPatchApplyResult,
  parseChapterPatchPreview,
  parseChapterPatchStatus,
  requestStoryChapterPatchApply,
  requestStoryChapterPatchPreview,
  type ChapterPatchPreview,
} from "./story-chapter-patch";

const preview: ChapterPatchPreview = {
  schemaVersion: 1,
  previewHash: "a".repeat(64),
  proposalHash: "b".repeat(64),
  continuityReportHash: "c".repeat(64),
  sourceManifestHash: "d".repeat(64),
  currentManifestHash: "d".repeat(64),
  requestHash: "e".repeat(64),
  action: "create",
  targetRelativePath: "chapters/003.md",
  operation: "create",
  expectedSourceHash: null,
  proposedContentHash: "f".repeat(64),
  chapterId: "chapter_11111111-2222-4333-8444-555555555555",
  previousStatus: null,
  nextStatus: "draft",
  acceptedOrFinalUnlockConfirmed: false,
  hunks: [
    {
      oldStart: 0,
      oldLineCount: 0,
      newStart: 1,
      newLineCount: 1,
      lines: [
        {
          kind: "added",
          oldLineNumber: null,
          newLineNumber: 1,
          content: "新正文。",
        },
      ],
    },
  ],
  summary: { addedLines: 1, removedLines: 0, unchangedLines: 0 },
  readyToApply: true,
};

const applied = {
  transactionId: "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
  file: {
    relativePath: "chapters/003.md",
    contentHash: "f".repeat(64),
    modifiedTimeMs: 1_800_000_000_100,
    sizeBytes: 20,
    bom: "none",
    lineEnding: "lf",
    diagnosticsTriggered: true,
    diagnosticState: "ready",
    validation: {
      relativePath: "chapters/003.md",
      contentHash: "f".repeat(64),
      diagnostics: [],
      eligibleForAiApply: true,
      eligibleForFinal: true,
      eligibleForEpub: true,
    },
  },
} as const;

const input = {
  runId: "run-1",
  proposalHash: "b".repeat(64),
  continuityReportHash: "c".repeat(64),
  action: "create" as const,
  chapterNumber: 3,
  chapterTitle: "潮痕",
  selectedSourcePaths: ["plot/main.md"],
  reviewedManifestHash: "d".repeat(64),
  readerMentalModel: {
    readerKnows: "",
    readerInfers: "",
    readerMisreads: "",
    readerQuestions: "",
    chapterPromises: "",
    forbiddenReveals: "",
  },
  acceptedOrFinalUnlockConfirmed: false,
};

describe("NG-505 chapter patch IPC contract", () => {
  it("accepts a strictly bound line diff and transaction receipt", () => {
    expect(parseChapterPatchPreview(preview)).toEqual(preview);
    expect(parseChapterPatchApplyResult(applied)).toEqual(applied);
    expect(parseChapterPatchStatus({ schemaVersion: 1, preview, applied })).toEqual({
      schemaVersion: 1,
      preview,
      applied,
    });
  });

  it("rejects unknown fields, forged diff counts and an unbound receipt", () => {
    expect(() => parseChapterPatchPreview({ ...preview, targetContent: "secret" })).toThrow();
    expect(() =>
      parseChapterPatchPreview({
        ...preview,
        summary: { ...preview.summary, addedLines: 2 },
      }),
    ).toThrow();
    expect(() =>
      parseChapterPatchStatus({
        schemaVersion: 1,
        preview,
        applied: {
          ...applied,
          file: { ...applied.file, contentHash: "0".repeat(64) },
        },
      }),
    ).toThrow();
  });

  it("sends the reviewed request for preview and only hashes for explicit apply", async () => {
    const invoke = vi.fn().mockResolvedValueOnce(preview).mockResolvedValueOnce(applied);
    await expect(requestStoryChapterPatchPreview("session-1", input, invoke)).resolves.toEqual(
      preview,
    );
    expect(invoke).toHaveBeenNthCalledWith(1, "story_chapter_patch_preview", {
      sessionId: "session-1",
      input: {
        schemaVersion: 1,
        runId: "run-1",
        proposalHash: "b".repeat(64),
        continuityReportHash: "c".repeat(64),
        request: {
          schemaVersion: 1,
          context: {
            schemaVersion: 2,
            action: "create",
            chapterNumber: 3,
            chapterTitle: "潮痕",
            selectedSourcePaths: ["plot/main.md"],
          },
          reviewedManifestHash: "d".repeat(64),
          readerMentalModel: input.readerMentalModel,
        },
        acceptedOrFinalUnlockConfirmed: false,
      },
    });
    await expect(
      requestStoryChapterPatchApply("session-1", "run-1", preview, invoke),
    ).resolves.toEqual(applied);
    expect(invoke).toHaveBeenNthCalledWith(2, "story_chapter_patch_apply", {
      sessionId: "session-1",
      input: {
        schemaVersion: 1,
        runId: "run-1",
        proposalHash: "b".repeat(64),
        continuityReportHash: "c".repeat(64),
        previewHash: "a".repeat(64),
        confirmed: true,
      },
    });
    expect(JSON.stringify(invoke.mock.calls[1])).not.toContain("新正文");
  });
});
