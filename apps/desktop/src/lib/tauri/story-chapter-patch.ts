import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError, type AppError } from "./health";
import type { StoryContextRequestPreviewInput } from "./story-context";
import { parseProjectFileSaveResult, type ProjectFileSaveResult } from "./workspace";

export const STORY_CHAPTER_PATCH_PREVIEW_COMMAND = "story_chapter_patch_preview";
export const STORY_CHAPTER_PATCH_STATUS_LOAD_COMMAND = "story_chapter_patch_status_load";
export const STORY_CHAPTER_PATCH_APPLY_COMMAND = "story_chapter_patch_apply";
export const STORY_CHAPTER_PATCH_SCHEMA_VERSION = 1;

export type ChapterDiffLine = {
  kind: "context" | "added" | "removed";
  oldLineNumber: number | null;
  newLineNumber: number | null;
  content: string;
};
export type ChapterDiffHunk = {
  oldStart: number;
  oldLineCount: number;
  newStart: number;
  newLineCount: number;
  lines: ChapterDiffLine[];
};
export type ChapterPatchPreview = {
  schemaVersion: 1;
  previewHash: string;
  proposalHash: string;
  continuityReportHash: string;
  sourceManifestHash: string;
  currentManifestHash: string;
  requestHash: string;
  action: "create" | "revise";
  targetRelativePath: string;
  operation: "create" | "replace";
  expectedSourceHash: string | null;
  proposedContentHash: string;
  chapterId: string;
  previousStatus: "draft" | "accepted" | "final" | null;
  nextStatus: "draft";
  acceptedOrFinalUnlockConfirmed: boolean;
  hunks: ChapterDiffHunk[];
  summary: { addedLines: number; removedLines: number; unchangedLines: number };
  readyToApply: true;
};
export type ChapterPatchApplyResult = {
  transactionId: string;
  file: ProjectFileSaveResult;
};
export type ChapterPatchStatus = {
  schemaVersion: 1;
  preview: ChapterPatchPreview | null;
  applied: ChapterPatchApplyResult | null;
};
export type StoryChapterPatchPreviewInput = StoryContextRequestPreviewInput & {
  runId: string;
  proposalHash: string;
  continuityReportHash: string;
  acceptedOrFinalUnlockConfirmed: boolean;
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);
const SHA256 = /^[0-9a-f]{64}$/u;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;
const TOKEN = /^[a-zA-Z0-9._-]{1,160}$/u;
const TARGET_PATH = /^chapters\/[0-9]{3,6}\.md$/u;
const PREVIEW_KEYS = [
  "acceptedOrFinalUnlockConfirmed",
  "action",
  "chapterId",
  "continuityReportHash",
  "currentManifestHash",
  "expectedSourceHash",
  "hunks",
  "nextStatus",
  "operation",
  "previousStatus",
  "previewHash",
  "proposalHash",
  "proposedContentHash",
  "readyToApply",
  "requestHash",
  "schemaVersion",
  "sourceManifestHash",
  "summary",
  "targetRelativePath",
];
const HUNK_KEYS = ["lines", "newLineCount", "newStart", "oldLineCount", "oldStart"];
const LINE_KEYS = ["content", "kind", "newLineNumber", "oldLineNumber"];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function exact(value: Record<string, unknown>, keys: string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}
function count(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0;
}
function lineNumber(value: unknown): value is number | null {
  return value === null || (Number.isSafeInteger(value) && Number(value) >= 1);
}

function parseDiffLine(value: unknown): ChapterDiffLine {
  if (
    !isRecord(value) ||
    !exact(value, LINE_KEYS) ||
    !["context", "added", "removed"].includes(value.kind as string) ||
    !lineNumber(value.oldLineNumber) ||
    !lineNumber(value.newLineNumber) ||
    typeof value.content !== "string" ||
    new TextEncoder().encode(value.content).length > 128 * 1024 ||
    (value.kind === "added" && (value.oldLineNumber !== null || value.newLineNumber === null)) ||
    (value.kind === "removed" && (value.oldLineNumber === null || value.newLineNumber !== null)) ||
    (value.kind === "context" && (value.oldLineNumber === null || value.newLineNumber === null))
  )
    throw new Error("Invalid chapter diff line");
  return value as ChapterDiffLine;
}

export function parseChapterDiffHunk(value: unknown): ChapterDiffHunk {
  if (
    !isRecord(value) ||
    !exact(value, HUNK_KEYS) ||
    !count(value.oldStart) ||
    !count(value.oldLineCount) ||
    !count(value.newStart) ||
    !count(value.newLineCount) ||
    !Array.isArray(value.lines) ||
    value.lines.length > 32_768
  )
    throw new Error("Invalid chapter diff hunk");
  const lines = value.lines.map(parseDiffLine);
  const removed = lines.filter((line) => line.kind === "removed").length;
  const added = lines.filter((line) => line.kind === "added").length;
  const context = lines.filter((line) => line.kind === "context").length;
  if (
    removed + context !== value.oldLineCount ||
    added + context !== value.newLineCount ||
    (value.oldLineCount === 0 && value.oldStart !== 0) ||
    (value.newLineCount > 0 && Number(value.newStart) < 1)
  )
    throw new Error("Invalid chapter diff hunk counts");
  return { ...(value as ChapterDiffHunk), lines };
}

export function parseChapterPatchPreview(value: unknown): ChapterPatchPreview {
  if (
    !isRecord(value) ||
    !exact(value, PREVIEW_KEYS) ||
    value.schemaVersion !== 1 ||
    ![
      value.previewHash,
      value.proposalHash,
      value.continuityReportHash,
      value.sourceManifestHash,
      value.currentManifestHash,
      value.requestHash,
      value.proposedContentHash,
    ].every((hash) => typeof hash === "string" && SHA256.test(hash)) ||
    (value.action !== "create" && value.action !== "revise") ||
    (value.operation !== "create" && value.operation !== "replace") ||
    !TARGET_PATH.test(value.targetRelativePath as string) ||
    (value.expectedSourceHash !== null &&
      (typeof value.expectedSourceHash !== "string" || !SHA256.test(value.expectedSourceHash))) ||
    typeof value.chapterId !== "string" ||
    !/^chapter_[\p{L}\p{N}._-]{1,160}$/u.test(value.chapterId) ||
    ![null, "draft", "accepted", "final"].includes(value.previousStatus as string | null) ||
    value.nextStatus !== "draft" ||
    typeof value.acceptedOrFinalUnlockConfirmed !== "boolean" ||
    value.readyToApply !== true ||
    !Array.isArray(value.hunks) ||
    value.hunks.length === 0 ||
    value.hunks.length > 8 ||
    !isRecord(value.summary) ||
    !exact(value.summary, ["addedLines", "removedLines", "unchangedLines"]) ||
    !count(value.summary.addedLines) ||
    !count(value.summary.removedLines) ||
    !count(value.summary.unchangedLines)
  )
    throw new Error("Invalid chapter patch preview");
  const hunks = value.hunks.map(parseChapterDiffHunk);
  const addedLines = hunks
    .flatMap((hunk) => hunk.lines)
    .filter((line) => line.kind === "added").length;
  const removedLines = hunks
    .flatMap((hunk) => hunk.lines)
    .filter((line) => line.kind === "removed").length;
  const unchangedLines = hunks
    .flatMap((hunk) => hunk.lines)
    .filter((line) => line.kind === "context").length;
  if (
    value.summary.addedLines !== addedLines ||
    value.summary.removedLines !== removedLines ||
    value.summary.unchangedLines !== unchangedLines ||
    (value.action === "create" &&
      (value.operation !== "create" ||
        value.expectedSourceHash !== null ||
        value.previousStatus !== null)) ||
    (value.action === "revise" &&
      (value.operation !== "replace" ||
        value.expectedSourceHash === null ||
        value.previousStatus === null)) ||
    ((value.previousStatus === "accepted" || value.previousStatus === "final") &&
      !value.acceptedOrFinalUnlockConfirmed)
  )
    throw new Error("Invalid chapter patch bindings");
  return {
    ...(value as ChapterPatchPreview),
    hunks,
    summary: { addedLines, removedLines, unchangedLines },
  };
}

export function parseChapterPatchApplyResult(value: unknown): ChapterPatchApplyResult {
  if (
    !isRecord(value) ||
    !exact(value, ["file", "transactionId"]) ||
    typeof value.transactionId !== "string" ||
    !UUID.test(value.transactionId)
  )
    throw new Error("Invalid chapter patch apply result");
  return {
    transactionId: value.transactionId,
    file: parseProjectFileSaveResult(value.file),
  };
}

export function parseChapterPatchStatus(value: unknown): ChapterPatchStatus {
  if (
    !isRecord(value) ||
    !exact(value, ["applied", "preview", "schemaVersion"]) ||
    value.schemaVersion !== 1
  )
    throw new Error("Invalid chapter patch status");
  const preview = value.preview === null ? null : parseChapterPatchPreview(value.preview);
  const applied = value.applied === null ? null : parseChapterPatchApplyResult(value.applied);
  if (
    applied !== null &&
    (preview === null ||
      applied.file.relativePath !== preview.targetRelativePath ||
      applied.file.contentHash !== preview.proposedContentHash)
  )
    throw new Error("Invalid chapter patch receipt binding");
  return { schemaVersion: 1, preview, applied };
}

function requestPayload(input: StoryChapterPatchPreviewInput) {
  return {
    schemaVersion: 1,
    runId: input.runId,
    proposalHash: input.proposalHash,
    continuityReportHash: input.continuityReportHash,
    request: {
      schemaVersion: 1,
      context: {
        schemaVersion: 2,
        action: input.action,
        chapterNumber: input.chapterNumber,
        chapterTitle: input.chapterTitle.trim(),
        selectedSourcePaths: [...input.selectedSourcePaths].sort(),
      },
      reviewedManifestHash: input.reviewedManifestHash,
      readerMentalModel: { ...input.readerMentalModel },
    },
    acceptedOrFinalUnlockConfirmed: input.acceptedOrFinalUnlockConfirmed,
  };
}

function contractError(): AppError {
  return { code: "contract.invalid_response", messageKey: "errors.invalidRuntimeResponse" };
}

export async function requestStoryChapterPatchPreview(
  sessionId: string,
  input: StoryChapterPatchPreviewInput,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ChapterPatchPreview> {
  if (
    !TOKEN.test(input.runId) ||
    !SHA256.test(input.proposalHash) ||
    !SHA256.test(input.continuityReportHash) ||
    !SHA256.test(input.reviewedManifestHash)
  )
    throw {
      code: "workflow.input_invalid",
      messageKey: "errors.workflowInputInvalid",
    } satisfies AppError;
  try {
    const preview = parseChapterPatchPreview(
      await invokeCommand(STORY_CHAPTER_PATCH_PREVIEW_COMMAND, {
        sessionId,
        input: requestPayload(input),
      }),
    );
    if (
      preview.proposalHash !== input.proposalHash ||
      preview.continuityReportHash !== input.continuityReportHash ||
      preview.requestHash.length !== 64
    )
      throw new Error("Chapter patch preview binding mismatch");
    return preview;
  } catch (error: unknown) {
    if (error instanceof Error) throw contractError();
    throw normalizeAppError(error);
  }
}

export async function requestStoryChapterPatchStatus(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ChapterPatchStatus> {
  try {
    return parseChapterPatchStatus(
      await invokeCommand(STORY_CHAPTER_PATCH_STATUS_LOAD_COMMAND, { sessionId }),
    );
  } catch (error: unknown) {
    if (error instanceof Error) throw contractError();
    throw normalizeAppError(error);
  }
}

export async function requestStoryChapterPatchApply(
  sessionId: string,
  runId: string,
  preview: ChapterPatchPreview,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ChapterPatchApplyResult> {
  try {
    const applied = parseChapterPatchApplyResult(
      await invokeCommand(STORY_CHAPTER_PATCH_APPLY_COMMAND, {
        sessionId,
        input: {
          schemaVersion: 1,
          runId,
          proposalHash: preview.proposalHash,
          continuityReportHash: preview.continuityReportHash,
          previewHash: preview.previewHash,
          confirmed: true,
        },
      }),
    );
    if (
      applied.file.relativePath !== preview.targetRelativePath ||
      applied.file.contentHash !== preview.proposedContentHash
    )
      throw new Error("Chapter patch apply binding mismatch");
    return applied;
  } catch (error: unknown) {
    if (error instanceof Error) throw contractError();
    throw normalizeAppError(error);
  }
}
