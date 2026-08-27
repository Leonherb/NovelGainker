import { describe, expect, it, vi } from "vitest";
import {
  parseChapterStatusPreview,
  parseLedgerUpdatePreview,
  parseStoryChapterLifecycleStatus,
  requestStoryChapterStateApply,
  requestStoryChapterStatePreview,
  requestStoryLedgerUpdateApply,
  requestStoryLedgerUpdatePreview,
  STORY_LEDGER_RELATIVE_PATH,
  type ChapterStatusPreview,
  type LedgerUpdatePreview,
} from "./story-lifecycle";

const statusPreview: ChapterStatusPreview = {
  schemaVersion: 1,
  previewHash: "a".repeat(64),
  targetRelativePath: "chapters/003.md",
  expectedSourceHash: "b".repeat(64),
  proposedContentHash: "c".repeat(64),
  previousStatus: "draft",
  nextStatus: "accepted",
  createdPendingId: "ledger_pending_aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
  hunks: [
    {
      oldStart: 8,
      oldLineCount: 1,
      newStart: 8,
      newLineCount: 1,
      lines: [
        { kind: "removed", oldLineNumber: 8, newLineNumber: null, content: "status: draft" },
        {
          kind: "added",
          oldLineNumber: null,
          newLineNumber: 8,
          content: "status: accepted",
        },
      ],
    },
  ],
  summary: { addedLines: 1, removedLines: 1, unchangedLines: 0 },
  readyToApply: true,
};

const ledgerPreview: LedgerUpdatePreview = {
  schemaVersion: 1,
  previewHash: "d".repeat(64),
  targetRelativePath: STORY_LEDGER_RELATIVE_PATH,
  chapterRelativePath: "chapters/003.md",
  expectedSourceHash: "e".repeat(64),
  proposedContentHash: "f".repeat(64),
  chapterContentHash: "c".repeat(64),
  pendingIds: ["ledger_pending_aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"],
  hunks: [
    {
      oldStart: 0,
      oldLineCount: 0,
      newStart: 12,
      newLineCount: 1,
      lines: [
        {
          kind: "added",
          oldLineNumber: null,
          newLineNumber: 12,
          content: "## 第 3 章《潮痕》状态更新",
        },
      ],
    },
  ],
  summary: { addedLines: 1, removedLines: 0, unchangedLines: 0 },
  readyToApply: true,
};

function receipt(path: string, hash: string) {
  return {
    transactionId: "bbbbbbbb-cccc-4ddd-8eee-ffffffffffff",
    file: {
      relativePath: path,
      contentHash: hash,
      modifiedTimeMs: 1_800_000_000_100,
      sizeBytes: 20,
      bom: "none",
      lineEnding: "lf",
      diagnosticsTriggered: true,
      diagnosticState: "ready",
      validation: {
        relativePath: path,
        contentHash: hash,
        diagnostics: [],
        eligibleForAiApply: true,
        eligibleForFinal: true,
        eligibleForEpub: true,
      },
    },
  };
}

describe("NG-506 chapter lifecycle IPC contract", () => {
  it("accepts only the approved status matrix and fixed Ledger target", () => {
    expect(parseChapterStatusPreview(statusPreview)).toEqual(statusPreview);
    expect(parseLedgerUpdatePreview(ledgerPreview)).toEqual(ledgerPreview);
    expect(() => parseChapterStatusPreview({ ...statusPreview, nextStatus: "final" })).toThrow();
    expect(() =>
      parseLedgerUpdatePreview({ ...ledgerPreview, targetRelativePath: "materials/other.md" }),
    ).toThrow();
    expect(() => parseLedgerUpdatePreview({ ...ledgerPreview, targetContent: "secret" })).toThrow();
  });

  it("recovers visible pending state from a strict status response", () => {
    const value = {
      schemaVersion: 1,
      targetRelativePath: "chapters/003.md",
      chapterStatus: "accepted",
      ledgerPending: [
        {
          resolution: "pending",
          record: {
            schemaVersion: 1,
            pendingId: "ledger_pending_aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
            chapterId: "chapter_11111111-2222-4333-8444-555555555555",
            chapterNumber: 3,
            chapterTitle: "潮痕",
            chapterRelativePath: "chapters/003.md",
            acceptedStatus: "accepted",
            sourceChapterHash: "b".repeat(64),
            proposalHash: "1".repeat(64),
            sourceManifestHash: "2".repeat(64),
            requestHash: "3".repeat(64),
            candidates: [{ summary: "钥匙已交接。", sourceIds: ["chapter:003"] }],
          },
        },
      ],
      statusPreview,
      statusApplied: receipt("chapters/003.md", "c".repeat(64)),
      ledgerPreview: null,
      ledgerApplied: null,
    };
    expect(parseStoryChapterLifecycleStatus(value)).toEqual(value);
    expect(() =>
      parseStoryChapterLifecycleStatus({
        ...value,
        ledgerPending: [{ ...value.ledgerPending[0], hiddenBody: "secret" }],
      }),
    ).toThrow();
  });

  it("uses separate minimal confirmations for chapter status and Ledger writes", async () => {
    const statusReceipt = receipt("chapters/003.md", "c".repeat(64));
    const ledgerReceipt = receipt(STORY_LEDGER_RELATIVE_PATH, "f".repeat(64));
    const invoke = vi
      .fn()
      .mockResolvedValueOnce(statusPreview)
      .mockResolvedValueOnce(statusReceipt)
      .mockResolvedValueOnce(ledgerPreview)
      .mockResolvedValueOnce(ledgerReceipt);

    await requestStoryChapterStatePreview(
      "session-1",
      "chapters/003.md",
      "accepted",
      { runId: "run-1", proposalHash: "1".repeat(64) },
      invoke,
    );
    await requestStoryChapterStateApply("session-1", statusPreview, invoke);
    await requestStoryLedgerUpdatePreview("session-1", "chapters/003.md", invoke);
    await requestStoryLedgerUpdateApply("session-1", ledgerPreview, invoke);

    expect(invoke).toHaveBeenNthCalledWith(1, "story_chapter_state_preview", {
      sessionId: "session-1",
      input: {
        schemaVersion: 1,
        targetRelativePath: "chapters/003.md",
        nextStatus: "accepted",
        runId: "run-1",
        proposalHash: "1".repeat(64),
      },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "story_chapter_state_apply", {
      sessionId: "session-1",
      input: { schemaVersion: 1, previewHash: "a".repeat(64), confirmed: true },
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "story_ledger_update_preview", {
      sessionId: "session-1",
      input: { schemaVersion: 1, chapterRelativePath: "chapters/003.md" },
    });
    expect(invoke).toHaveBeenNthCalledWith(4, "story_ledger_update_apply", {
      sessionId: "session-1",
      input: { schemaVersion: 1, previewHash: "d".repeat(64), confirmed: true },
    });
    expect(JSON.stringify(invoke.mock.calls)).not.toContain("钥匙已交接");
  });
});
