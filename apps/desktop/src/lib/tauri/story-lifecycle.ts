import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError, type AppError } from "./health";
import {
  parseChapterDiffHunk,
  parseChapterPatchApplyResult,
  type ChapterDiffHunk,
  type ChapterPatchApplyResult,
} from "./story-chapter-patch";

export const STORY_CHAPTER_STATE_PREVIEW_COMMAND = "story_chapter_state_preview";
export const STORY_CHAPTER_LIFECYCLE_STATUS_LOAD_COMMAND = "story_chapter_lifecycle_status_load";
export const STORY_CHAPTER_STATE_APPLY_COMMAND = "story_chapter_state_apply";
export const STORY_LEDGER_UPDATE_PREVIEW_COMMAND = "story_ledger_update_preview";
export const STORY_LEDGER_UPDATE_APPLY_COMMAND = "story_ledger_update_apply";
export const STORY_CHAPTER_LIFECYCLE_SCHEMA_VERSION = 1;
export const STORY_LEDGER_RELATIVE_PATH = "materials/000.story-ledger.md";

export type ChapterLifecycleStatus = "draft" | "accepted" | "final";
export type LedgerCandidate = { summary: string; sourceIds: string[] };
export type LedgerPendingRecord = {
  schemaVersion: 1;
  pendingId: string;
  chapterId: string;
  chapterNumber: number;
  chapterTitle: string;
  chapterRelativePath: string;
  acceptedStatus: "accepted" | "final";
  sourceChapterHash: string;
  proposalHash: string;
  sourceManifestHash: string;
  requestHash: string;
  candidates: LedgerCandidate[];
};
export type LedgerPendingStatus = {
  record: LedgerPendingRecord;
  resolution: "pending" | "applied";
};
export type ChapterStatusPreview = {
  schemaVersion: 1;
  previewHash: string;
  targetRelativePath: string;
  expectedSourceHash: string;
  proposedContentHash: string;
  previousStatus: "draft" | "accepted";
  nextStatus: "accepted" | "final";
  createdPendingId: string | null;
  hunks: ChapterDiffHunk[];
  summary: { addedLines: number; removedLines: number; unchangedLines: number };
  readyToApply: true;
};
export type LedgerUpdatePreview = {
  schemaVersion: 1;
  previewHash: string;
  targetRelativePath: typeof STORY_LEDGER_RELATIVE_PATH;
  chapterRelativePath: string;
  expectedSourceHash: string;
  proposedContentHash: string;
  chapterContentHash: string;
  pendingIds: string[];
  hunks: ChapterDiffHunk[];
  summary: { addedLines: number; removedLines: number; unchangedLines: number };
  readyToApply: true;
};
export type StoryChapterLifecycleStatus = {
  schemaVersion: 1;
  targetRelativePath: string;
  chapterStatus: ChapterLifecycleStatus;
  ledgerPending: LedgerPendingStatus[];
  statusPreview: ChapterStatusPreview | null;
  statusApplied: ChapterPatchApplyResult | null;
  ledgerPreview: LedgerUpdatePreview | null;
  ledgerApplied: ChapterPatchApplyResult | null;
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);
const SHA256 = /^[0-9a-f]{64}$/u;
const TOKEN = /^[a-zA-Z0-9._-]{1,160}$/u;
const CHAPTER_PATH = /^chapters\/[0-9]{3,6}\.md$/u;
const PENDING_ID =
  /^ledger_pending_[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;
const STATUS_PREVIEW_KEYS = [
  "createdPendingId",
  "expectedSourceHash",
  "hunks",
  "nextStatus",
  "previousStatus",
  "previewHash",
  "proposedContentHash",
  "readyToApply",
  "schemaVersion",
  "summary",
  "targetRelativePath",
];
const LEDGER_PREVIEW_KEYS = [
  "chapterContentHash",
  "chapterRelativePath",
  "expectedSourceHash",
  "hunks",
  "pendingIds",
  "previewHash",
  "proposedContentHash",
  "readyToApply",
  "schemaVersion",
  "summary",
  "targetRelativePath",
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function exact(value: Record<string, unknown>, keys: string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function nonnegative(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0;
}

function safeText(value: unknown, maximum: number): value is string {
  return (
    typeof value === "string" &&
    value.trim().length > 0 &&
    new TextEncoder().encode(value).length <= maximum &&
    !value.includes("\u0000")
  );
}

function parseSummary(value: unknown) {
  if (
    !isRecord(value) ||
    !exact(value, ["addedLines", "removedLines", "unchangedLines"]) ||
    !nonnegative(value.addedLines) ||
    !nonnegative(value.removedLines) ||
    !nonnegative(value.unchangedLines)
  )
    throw new Error("Invalid lifecycle diff summary");
  return value as { addedLines: number; removedLines: number; unchangedLines: number };
}

function parseDiff(value: Record<string, unknown>) {
  if (!Array.isArray(value.hunks) || value.hunks.length === 0 || value.hunks.length > 16)
    throw new Error("Invalid lifecycle diff");
  const hunks = value.hunks.map(parseChapterDiffHunk);
  const summary = parseSummary(value.summary);
  const lines = hunks.flatMap((hunk) => hunk.lines);
  if (
    lines.filter((line) => line.kind === "added").length !== summary.addedLines ||
    lines.filter((line) => line.kind === "removed").length !== summary.removedLines ||
    lines.filter((line) => line.kind === "context").length !== summary.unchangedLines
  )
    throw new Error("Invalid lifecycle diff binding");
  return { hunks, summary };
}

export function parseChapterStatusPreview(value: unknown): ChapterStatusPreview {
  if (
    !isRecord(value) ||
    !exact(value, STATUS_PREVIEW_KEYS) ||
    value.schemaVersion !== 1 ||
    ![value.previewHash, value.expectedSourceHash, value.proposedContentHash].every(
      (hash) => typeof hash === "string" && SHA256.test(hash),
    ) ||
    !CHAPTER_PATH.test(value.targetRelativePath as string) ||
    !["draft", "accepted"].includes(value.previousStatus as string) ||
    !["accepted", "final"].includes(value.nextStatus as string) ||
    value.readyToApply !== true ||
    (value.createdPendingId !== null &&
      (typeof value.createdPendingId !== "string" || !PENDING_ID.test(value.createdPendingId))) ||
    (value.previousStatus === "draft" &&
      (value.nextStatus !== "accepted" || value.createdPendingId === null)) ||
    (value.previousStatus === "accepted" &&
      (value.nextStatus !== "final" || value.createdPendingId !== null))
  )
    throw new Error("Invalid chapter status preview");
  const { hunks, summary } = parseDiff(value);
  return { ...(value as ChapterStatusPreview), hunks, summary };
}

export function parseLedgerUpdatePreview(value: unknown): LedgerUpdatePreview {
  if (
    !isRecord(value) ||
    !exact(value, LEDGER_PREVIEW_KEYS) ||
    value.schemaVersion !== 1 ||
    value.targetRelativePath !== STORY_LEDGER_RELATIVE_PATH ||
    !CHAPTER_PATH.test(value.chapterRelativePath as string) ||
    ![
      value.previewHash,
      value.expectedSourceHash,
      value.proposedContentHash,
      value.chapterContentHash,
    ].every((hash) => typeof hash === "string" && SHA256.test(hash)) ||
    !Array.isArray(value.pendingIds) ||
    value.pendingIds.length === 0 ||
    value.pendingIds.length > 64 ||
    !value.pendingIds.every((id) => typeof id === "string" && PENDING_ID.test(id)) ||
    new Set(value.pendingIds).size !== value.pendingIds.length ||
    value.readyToApply !== true
  )
    throw new Error("Invalid Ledger update preview");
  const { hunks, summary } = parseDiff(value);
  return { ...(value as LedgerUpdatePreview), hunks, summary };
}

function parseCandidate(value: unknown): LedgerCandidate {
  if (
    !isRecord(value) ||
    !exact(value, ["sourceIds", "summary"]) ||
    !safeText(value.summary, 4_096) ||
    !Array.isArray(value.sourceIds) ||
    value.sourceIds.length > 16 ||
    !value.sourceIds.every((item) => safeText(item, 256))
  )
    throw new Error("Invalid Ledger candidate");
  return value as LedgerCandidate;
}

function parsePendingRecord(value: unknown): LedgerPendingRecord {
  if (
    !isRecord(value) ||
    !exact(value, [
      "acceptedStatus",
      "candidates",
      "chapterId",
      "chapterNumber",
      "chapterRelativePath",
      "chapterTitle",
      "pendingId",
      "proposalHash",
      "requestHash",
      "schemaVersion",
      "sourceChapterHash",
      "sourceManifestHash",
    ]) ||
    value.schemaVersion !== 1 ||
    typeof value.pendingId !== "string" ||
    !PENDING_ID.test(value.pendingId) ||
    !safeText(value.chapterId, 256) ||
    !Number.isSafeInteger(value.chapterNumber) ||
    Number(value.chapterNumber) < 1 ||
    !safeText(value.chapterTitle, 4_096) ||
    !CHAPTER_PATH.test(value.chapterRelativePath as string) ||
    !["accepted", "final"].includes(value.acceptedStatus as string) ||
    ![
      value.sourceChapterHash,
      value.proposalHash,
      value.sourceManifestHash,
      value.requestHash,
    ].every((hash) => typeof hash === "string" && SHA256.test(hash)) ||
    !Array.isArray(value.candidates) ||
    value.candidates.length > 64
  )
    throw new Error("Invalid Ledger pending record");
  return { ...(value as LedgerPendingRecord), candidates: value.candidates.map(parseCandidate) };
}

function parsePendingStatus(value: unknown): LedgerPendingStatus {
  if (
    !isRecord(value) ||
    !exact(value, ["record", "resolution"]) ||
    !["pending", "applied"].includes(value.resolution as string)
  )
    throw new Error("Invalid Ledger pending status");
  return {
    record: parsePendingRecord(value.record),
    resolution: value.resolution as "pending" | "applied",
  };
}

export function parseStoryChapterLifecycleStatus(value: unknown): StoryChapterLifecycleStatus {
  if (
    !isRecord(value) ||
    !exact(value, [
      "chapterStatus",
      "ledgerApplied",
      "ledgerPending",
      "ledgerPreview",
      "schemaVersion",
      "statusApplied",
      "statusPreview",
      "targetRelativePath",
    ]) ||
    value.schemaVersion !== 1 ||
    !CHAPTER_PATH.test(value.targetRelativePath as string) ||
    !["draft", "accepted", "final"].includes(value.chapterStatus as string) ||
    !Array.isArray(value.ledgerPending) ||
    value.ledgerPending.length > 64
  )
    throw new Error("Invalid chapter lifecycle status");
  const statusPreview =
    value.statusPreview === null ? null : parseChapterStatusPreview(value.statusPreview);
  const statusApplied =
    value.statusApplied === null ? null : parseChapterPatchApplyResult(value.statusApplied);
  const ledgerPreview =
    value.ledgerPreview === null ? null : parseLedgerUpdatePreview(value.ledgerPreview);
  const ledgerApplied =
    value.ledgerApplied === null ? null : parseChapterPatchApplyResult(value.ledgerApplied);
  if (statusPreview !== null && statusPreview.targetRelativePath !== value.targetRelativePath)
    throw new Error("Invalid status target binding");
  if (
    statusApplied !== null &&
    (statusPreview === null ||
      statusApplied.file.relativePath !== statusPreview.targetRelativePath ||
      statusApplied.file.contentHash !== statusPreview.proposedContentHash)
  )
    throw new Error("Invalid status receipt binding");
  if (ledgerPreview !== null && ledgerPreview.chapterRelativePath !== value.targetRelativePath)
    throw new Error("Invalid Ledger chapter binding");
  if (
    ledgerApplied !== null &&
    (ledgerPreview === null ||
      ledgerApplied.file.relativePath !== STORY_LEDGER_RELATIVE_PATH ||
      ledgerApplied.file.contentHash !== ledgerPreview.proposedContentHash)
  )
    throw new Error("Invalid Ledger receipt binding");
  return {
    schemaVersion: 1,
    targetRelativePath: value.targetRelativePath as string,
    chapterStatus: value.chapterStatus as ChapterLifecycleStatus,
    ledgerPending: value.ledgerPending.map(parsePendingStatus),
    statusPreview,
    statusApplied,
    ledgerPreview,
    ledgerApplied,
  };
}

function contractError(): AppError {
  return { code: "contract.invalid_response", messageKey: "errors.invalidRuntimeResponse" };
}

function validateTarget(path: string): void {
  if (!CHAPTER_PATH.test(path))
    throw {
      code: "workflow.input_invalid",
      messageKey: "errors.workflowInputInvalid",
    } satisfies AppError;
}

async function strictInvoke<T>(
  command: string,
  args: Record<string, unknown>,
  parse: (value: unknown) => T,
  invokeCommand: InvokeCommand,
): Promise<T> {
  try {
    return parse(await invokeCommand(command, args));
  } catch (error: unknown) {
    if (error instanceof Error) throw contractError();
    throw normalizeAppError(error);
  }
}

export async function requestStoryChapterStatePreview(
  sessionId: string,
  targetRelativePath: string,
  nextStatus: "accepted" | "final",
  binding: { runId: string; proposalHash: string } | null,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ChapterStatusPreview> {
  validateTarget(targetRelativePath);
  if (
    (nextStatus === "accepted" &&
      (binding === null || !TOKEN.test(binding.runId) || !SHA256.test(binding.proposalHash))) ||
    (nextStatus === "final" && binding !== null)
  )
    throw {
      code: "workflow.input_invalid",
      messageKey: "errors.workflowInputInvalid",
    } satisfies AppError;
  const preview = await strictInvoke(
    STORY_CHAPTER_STATE_PREVIEW_COMMAND,
    {
      sessionId,
      input: {
        schemaVersion: 1,
        targetRelativePath,
        nextStatus,
        runId: binding?.runId ?? null,
        proposalHash: binding?.proposalHash ?? null,
      },
    },
    parseChapterStatusPreview,
    invokeCommand,
  );
  if (preview.targetRelativePath !== targetRelativePath || preview.nextStatus !== nextStatus)
    throw contractError();
  return preview;
}

export async function requestStoryChapterStateApply(
  sessionId: string,
  preview: ChapterStatusPreview,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ChapterPatchApplyResult> {
  const applied = await strictInvoke(
    STORY_CHAPTER_STATE_APPLY_COMMAND,
    {
      sessionId,
      input: { schemaVersion: 1, previewHash: preview.previewHash, confirmed: true },
    },
    parseChapterPatchApplyResult,
    invokeCommand,
  );
  if (
    applied.file.relativePath !== preview.targetRelativePath ||
    applied.file.contentHash !== preview.proposedContentHash
  )
    throw contractError();
  return applied;
}

export async function requestStoryChapterLifecycleStatus(
  sessionId: string,
  targetRelativePath: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<StoryChapterLifecycleStatus> {
  validateTarget(targetRelativePath);
  const status = await strictInvoke(
    STORY_CHAPTER_LIFECYCLE_STATUS_LOAD_COMMAND,
    { sessionId, targetRelativePath },
    parseStoryChapterLifecycleStatus,
    invokeCommand,
  );
  if (status.targetRelativePath !== targetRelativePath) throw contractError();
  return status;
}

export async function requestStoryLedgerUpdatePreview(
  sessionId: string,
  chapterRelativePath: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<LedgerUpdatePreview> {
  validateTarget(chapterRelativePath);
  const preview = await strictInvoke(
    STORY_LEDGER_UPDATE_PREVIEW_COMMAND,
    {
      sessionId,
      input: { schemaVersion: 1, chapterRelativePath },
    },
    parseLedgerUpdatePreview,
    invokeCommand,
  );
  if (preview.chapterRelativePath !== chapterRelativePath) throw contractError();
  return preview;
}

export async function requestStoryLedgerUpdateApply(
  sessionId: string,
  preview: LedgerUpdatePreview,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ChapterPatchApplyResult> {
  const applied = await strictInvoke(
    STORY_LEDGER_UPDATE_APPLY_COMMAND,
    {
      sessionId,
      input: { schemaVersion: 1, previewHash: preview.previewHash, confirmed: true },
    },
    parseChapterPatchApplyResult,
    invokeCommand,
  );
  if (
    applied.file.relativePath !== STORY_LEDGER_RELATIVE_PATH ||
    applied.file.contentHash !== preview.proposedContentHash
  )
    throw contractError();
  return applied;
}
