import { describe, expect, it, vi } from "vitest";
import {
  parseSearchReport,
  parseValidationReport,
  requestProjectDraftValidation,
  requestProjectSearch,
} from "./analysis";

const sessionId = "20000000-0000-4000-8000-000000000002";
const validation = {
  relativePath: "chapters/001.md",
  contentHash: "a".repeat(64),
  diagnostics: [
    {
      code: "chapter.draft_heading_missing",
      severity: "error",
      relativePath: "chapters/001.md",
      line: 6,
      column: 1,
      message: "章节缺少严格的 ## Draft 标题。",
      recovery: "请在 Frontmatter 后单独添加一行 ## Draft，并把章节正文放在其下。",
    },
  ],
  eligibleForAiApply: false,
  eligibleForFinal: false,
  eligibleForEpub: false,
} as const;
const searchReport = {
  matches: [
    {
      relativePath: "chapters/001.md",
      line: 8,
      column: 3,
      snippet: "月光落在旧书页上。",
    },
  ],
  scannedFileCount: 3,
  skippedFileCount: 0,
  scannedBytes: 1024,
  truncated: false,
} as const;

describe("NG-105 validation and offline-search IPC contracts", () => {
  it("accepts stable diagnostics and rejects mismatched eligibility or content-bearing fields", () => {
    expect(parseValidationReport(validation)).toEqual(validation);
    expect(() => parseValidationReport({ ...validation, eligibleForAiApply: true })).toThrow();
    expect(() =>
      parseValidationReport({
        ...validation,
        diagnostics: [{ ...validation.diagnostics[0], prompt: "private manuscript" }],
      }),
    ).toThrow();
    expect(() =>
      parseValidationReport({ ...validation, absolutePath: "D:\\Novel\\chapters\\001.md" }),
    ).toThrow();
  });

  it("does not treat a valid draft as authorized for publication", () => {
    const valid = {
      ...validation,
      diagnostics: [],
      eligibleForAiApply: true,
      eligibleForFinal: true,
    };
    expect(parseValidationReport(valid).eligibleForEpub).toBe(false);
    expect(() => parseValidationReport({ ...valid, eligibleForEpub: true })).toThrow();
  });

  it("accepts only bounded relative-path search matches", () => {
    expect(parseSearchReport(searchReport)).toEqual(searchReport);
    expect(() =>
      parseSearchReport({
        ...searchReport,
        matches: [{ ...searchReport.matches[0], relativePath: "../secret.md" }],
      }),
    ).toThrow();
    expect(() =>
      parseSearchReport({
        ...searchReport,
        matches: [{ ...searchReport.matches[0], snippet: "a".repeat(163) }],
      }),
    ).toThrow();
  });

  it("sends draft validation and search only to their exact scoped commands", async () => {
    const invoke = vi.fn().mockResolvedValueOnce(validation).mockResolvedValueOnce(searchReport);
    const draft = "---\ntitle: 第一章\nwritingLanguage: zh-CN\n---\n正文";

    await requestProjectDraftValidation(sessionId, "chapters/001.md", draft, invoke);
    await requestProjectSearch(sessionId, "月光", false, invoke);

    expect(invoke.mock.calls).toEqual([
      ["project_validate_draft", { sessionId, relativePath: "chapters/001.md", content: draft }],
      ["project_search", { sessionId, query: "月光", caseSensitive: false }],
    ]);
    expect(JSON.stringify(invoke.mock.calls)).not.toMatch(/[A-Z]:\\/u);
  });

  it("never reflects a rejected query or manuscript body through AppError", async () => {
    const privateQuery = "绝密查询";
    const privateBody = "未发表正文";
    const invoke = vi
      .fn()
      .mockRejectedValueOnce(new Error(privateQuery))
      .mockRejectedValueOnce(new Error(privateBody));

    await expect(requestProjectSearch(sessionId, privateQuery, false, invoke)).rejects.toEqual({
      code: "runtime.unknown",
      messageKey: "errors.unknown",
    });
    await expect(
      requestProjectDraftValidation(sessionId, "chapters/001.md", privateBody, invoke),
    ).rejects.toEqual({
      code: "runtime.unknown",
      messageKey: "errors.unknown",
    });
  });
});
