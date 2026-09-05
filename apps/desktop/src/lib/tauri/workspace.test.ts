import { describe, expect, it, vi } from "vitest";
import {
  MAX_EDITABLE_FILE_BYTES,
  parseProjectFileDocument,
  parseProjectFileIndex,
  parseProjectFileSaveResult,
  parseProjectFileWatchStatus,
  requestProjectFile,
  requestProjectFileIndex,
  requestProjectFileSave,
  requestProjectFileWatch,
} from "./workspace";

const sessionId = "20000000-0000-4000-8000-000000000002";
const index = {
  maxEditableFileBytes: MAX_EDITABLE_FILE_BYTES,
  nodes: [
    {
      name: "project.md",
      relativePath: "project.md",
      kind: "markdown",
      sizeBytes: 10,
      readOnly: false,
      children: [],
    },
    {
      name: "chapters",
      relativePath: "chapters",
      kind: "directory",
      sizeBytes: null,
      readOnly: false,
      children: [
        {
          name: "001.md",
          relativePath: "chapters/001.md",
          kind: "markdown",
          sizeBytes: 20,
          readOnly: false,
          children: [],
        },
      ],
    },
    {
      name: ".novelgainker",
      relativePath: ".novelgainker",
      kind: "directory",
      sizeBytes: null,
      readOnly: true,
      children: [
        {
          name: "project.json",
          relativePath: ".novelgainker/project.json",
          kind: "metadata",
          sizeBytes: 100,
          readOnly: true,
          children: [],
        },
      ],
    },
  ],
} as const;
const document = {
  fileId: "10000000-0000-4000-8000-000000000001",
  editorRevision: 1,
  relativePath: "chapters/001.md",
  content: "# 第一章\n",
  contentHash: "a".repeat(64),
  modifiedTimeMs: 1_800_000_000_000,
  sizeBytes: 14,
  kind: "markdown",
  readOnly: false,
  bom: "none",
  lineEnding: "lf",
} as const;

const watchStatus = {
  relativePath: "chapters/001.md",
  state: "modified",
  observedHash: "c".repeat(64),
  modifiedTimeMs: 1_800_000_000_050,
} as const;

const saveResult = {
  relativePath: "chapters/001.md",
  contentHash: "b".repeat(64),
  modifiedTimeMs: 1_800_000_000_100,
  sizeBytes: 20,
  bom: "none",
  lineEnding: "lf",
  diagnosticsTriggered: true,
  diagnosticState: "ready",
  validation: {
    relativePath: "chapters/001.md",
    contentHash: "b".repeat(64),
    diagnostics: [],
    eligibleForAiApply: true,
    eligibleForFinal: true,
    eligibleForEpub: true,
  },
} as const;
describe("NG-105 workspace IPC contract", () => {
  it("accepts the bounded recursive index and controlled metadata", () => {
    expect(parseProjectFileIndex(index)).toEqual(index);
  });

  it("rejects absolute, traversal, published and unknown contract fields", () => {
    expect(() =>
      parseProjectFileIndex({
        ...index,
        nodes: [{ ...index.nodes[0], relativePath: "../secret.md" }],
      }),
    ).toThrow();
    expect(() =>
      parseProjectFileIndex({
        ...index,
        rootPath: "D:\\Novel",
      }),
    ).toThrow();
    expect(() =>
      parseProjectFileDocument({
        ...document,
        absolutePath: "D:\\Novel\\chapters\\001.md",
      }),
    ).toThrow();
  });

  it("accepts a strict document with content hash and encoding metadata", () => {
    expect(parseProjectFileDocument(document)).toEqual(document);
    expect(
      parseProjectFileDocument({
        ...document,
        relativePath: ".novelgainker/project.json",
        kind: "metadata",
        readOnly: true,
      }),
    ).toMatchObject({ kind: "metadata", readOnly: true });
  });

  it("sends only opaque session IDs and relative paths", async () => {
    const invoke = vi.fn().mockResolvedValueOnce(index).mockResolvedValueOnce(document);

    await requestProjectFileIndex(sessionId, invoke);
    await requestProjectFile(sessionId, "chapters/001.md", invoke);

    expect(invoke.mock.calls).toEqual([
      ["project_list_files", { sessionId }],
      ["project_read_file", { sessionId, relativePath: "chapters/001.md" }],
    ]);
    expect(JSON.stringify(invoke.mock.calls)).not.toMatch(/[A-Z]:\\/u);
    expect(JSON.stringify(invoke.mock.calls)).not.toContain(document.content);
  });

  it("accepts only strict Hash Watcher states", () => {
    expect(parseProjectFileWatchStatus(watchStatus)).toEqual(watchStatus);
    expect(
      parseProjectFileWatchStatus({
        relativePath: "chapters/001.md",
        state: "missing",
        observedHash: null,
        modifiedTimeMs: 0,
      }),
    ).toMatchObject({ state: "missing", observedHash: null });
    expect(() => parseProjectFileWatchStatus({ ...watchStatus, observedHash: null })).toThrow();
    expect(() => parseProjectFileWatchStatus({ ...watchStatus, content: "private" })).toThrow();
  });

  it("sends only the current relative path and expected Hash to the watcher", async () => {
    const invoke = vi.fn().mockResolvedValue(watchStatus);

    await requestProjectFileWatch(sessionId, "chapters/001.md", document.contentHash, invoke);

    expect(invoke).toHaveBeenCalledWith("project_watch_file", {
      sessionId,
      relativePath: "chapters/001.md",
      expectedHash: document.contentHash,
    });
    expect(JSON.stringify(invoke.mock.calls)).not.toContain(document.content);
  });

  it("accepts only a ready validation tied to the saved path and hash", () => {
    expect(parseProjectFileSaveResult(saveResult)).toEqual(saveResult);
    expect(() => parseProjectFileSaveResult({ ...saveResult, lineEnding: "mixed" })).toThrow();
    expect(() =>
      parseProjectFileSaveResult({
        ...saveResult,
        validation: { ...saveResult.validation, contentHash: "c".repeat(64) },
      }),
    ).toThrow();
    expect(() =>
      parseProjectFileSaveResult({ ...saveResult, absolutePath: "D:\\Novel\\chapters\\001.md" }),
    ).toThrow();
  });

  it("sends exactly the current content and source hash to the scoped save command", async () => {
    const nextContent = "# 第一章\n\n手动修改。";
    const invoke = vi.fn().mockResolvedValue(saveResult);

    await requestProjectFileSave(
      sessionId,
      "chapters/001.md",
      nextContent,
      document.contentHash,
      invoke,
    );

    expect(invoke).toHaveBeenCalledWith("project_save_file", {
      sessionId,
      relativePath: "chapters/001.md",
      content: nextContent,
      sourceHash: document.contentHash,
    });
    expect(JSON.stringify(invoke.mock.calls)).not.toMatch(/[A-Z]:\\/u);
  });
  it("maps malformed or content-bearing errors to the controlled AppError contract", async () => {
    await expect(
      requestProjectFile(
        sessionId,
        "chapters/001.md",
        vi.fn().mockRejectedValue(new Error("C:\\Users\\Leon\\draft.md # private body")),
      ),
    ).rejects.toEqual({
      code: "runtime.unknown",
      messageKey: "errors.unknown",
    });
  });
});
