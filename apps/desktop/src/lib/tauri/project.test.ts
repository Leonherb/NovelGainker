import { describe, expect, it, vi } from "vitest";
import {
  parseProjectCandidate,
  parseProjectCreatePreview,
  parseProjectSession,
  requestProjectCreate,
  requestProjectCreatePreview,
  requestProjectOpen,
  requestProjectOpenRecent,
  requestProjectScan,
} from "./project";

const candidate = {
  candidateId: "10000000-0000-4000-8000-000000000001",
  mode: "create",
  displayName: "我的小说",
  existingEntryCount: 0,
  markdownFileCount: 0,
  hasProjectMetadata: false,
  requiresMetadataConfirmation: false,
  canConfirm: true,

  plannedEntries: [
    { relativePath: "project.md", kind: "file" },
    { relativePath: ".novelgainker", kind: "directory" },
  ],
  conflicts: [],
} as const;

const previewContents = [
  ["project.md", "# 项目\n"],
  ["AGENTS.md", "# 约定\n"],
  [".novelgainker/project.json", "{}\n"],
] as const;

const createPreview = {
  schemaVersion: 1,
  previewHash: "a".repeat(64),
  projectId: "30000000-0000-4000-8000-000000000003",
  writingLanguage: "zh-CN",
  workflowPackVersion: "1.0.0",
  defaultProviderId: "openai",
  existingUnrelatedEntryCount: 0,
  directories: [
    "characters",
    "materials",
    "macguffins",
    "plot",
    "outlines",
    "world",
    "style",
    "assets",
    "chapters",
    "published",
    ".novelgainker",
  ],
  files: previewContents.map(([relativePath, content]) => ({
    relativePath,
    byteLength: new TextEncoder().encode(content).length,
    contentSha256: "b".repeat(64),
    content,
  })),
} as const;

const createInput = {
  schemaVersion: 1,
  title: "雾港来信",
  authorDisplayName: null,
  genre: "悬疑",
  premise: null,
} as const;

const session = {
  sessionId: "20000000-0000-4000-8000-000000000002",
  projectId: "30000000-0000-4000-8000-000000000003",
  displayName: "我的小说",
  schemaVersion: 1,
  writingLanguage: "zh-CN",
  markdownFileCount: 2,
  accessMode: "writer",
  recoveryStatus: "ready",
  recoveredTransactionCount: 0,
  requiredTransactionCount: 0,
  restoredCurrentFile: null,
  capabilities: {
    canRead: true,
    canWrite: true,
    canBrowseFiles: true,
    canSaveFiles: true,
  },
} as const;

describe("NG-104 project IPC contract", () => {
  it("accepts only the opaque candidate contract and cancellation", () => {
    expect(parseProjectCandidate(candidate)).toEqual(candidate);
    expect(parseProjectCandidate(null)).toBeNull();
    expect(() =>
      parseProjectCandidate({ ...candidate, root: "C:\\Users\\Leon\\secret" }),
    ).toThrow();
    expect(() =>
      parseProjectCandidate({
        ...candidate,

        plannedEntries: [{ relativePath: "../secret.md", kind: "file" }],
      }),
    ).toThrow();
  });
  it("accepts only the complete hash-bound scaffold preview", () => {
    expect(parseProjectCreatePreview(createPreview)).toEqual(createPreview);
    expect(() =>
      parseProjectCreatePreview({
        ...createPreview,
        absoluteRoot: "D:\\Novel",
      }),
    ).toThrow();
    expect(() =>
      parseProjectCreatePreview({
        ...createPreview,
        files: createPreview.files.slice(0, 2),
      }),
    ).toThrow();
    expect(() =>
      parseProjectCreatePreview({
        ...createPreview,
        files: createPreview.files.map((file, index) =>
          index === 0 ? { ...file, relativePath: "../project.md" } : file,
        ),
      }),
    ).toThrow();
  });

  it("accepts lock-aware sessions and rejects inconsistent capabilities", () => {
    expect(parseProjectSession(session)).toEqual(session);
    expect(() => parseProjectSession({ ...session, absolutePath: "D:\\Novel" })).toThrow();
    expect(() =>
      parseProjectSession({ ...session, restoredCurrentFile: "../secret.md" }),
    ).toThrow();
    expect(() =>
      parseProjectSession({
        ...session,
        capabilities: { ...session.capabilities, canSaveFiles: false },
      }),
    ).toThrow();
    expect(
      parseProjectSession({
        ...session,
        accessMode: "readOnly",
        recoveryStatus: "notChecked",
        capabilities: { ...session.capabilities, canWrite: false, canSaveFiles: false },
      }),
    ).toMatchObject({ accessMode: "readOnly", recoveryStatus: "notChecked" });
  });

  it("uses only controlled metadata, candidate IDs, Preview Hash and confirmation across IPC", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce(candidate)
      .mockResolvedValueOnce(createPreview)
      .mockResolvedValueOnce(session)
      .mockResolvedValueOnce(session)
      .mockResolvedValueOnce(session);

    await requestProjectScan("create", invoke);
    await requestProjectCreatePreview(candidate.candidateId, createInput, invoke);
    await requestProjectCreate(candidate.candidateId, createPreview.previewHash, invoke);
    await requestProjectOpen(candidate.candidateId, true, invoke);
    await requestProjectOpenRecent(session.projectId, invoke);

    expect(invoke.mock.calls).toEqual([
      ["project_scan", { mode: "create" }],
      ["project_create_preview", { candidateId: candidate.candidateId, input: createInput }],
      [
        "project_create",
        {
          candidateId: candidate.candidateId,
          previewHash: createPreview.previewHash,
          confirmed: true,
        },
      ],
      ["project_open", { candidateId: candidate.candidateId, confirmMetadata: true }],
      ["project_open_recent", { projectId: session.projectId }],
    ]);
    expect(JSON.stringify(invoke.mock.calls)).not.toMatch(/[A-Z]:\\/u);
  });

  it("maps malformed runtime output to a stable contract error", async () => {
    await expect(
      requestProjectScan("open", vi.fn().mockResolvedValue({ path: "D:\\Novel" })),
    ).rejects.toEqual({
      code: "contract.invalid_response",
      messageKey: "errors.invalidRuntimeResponse",
    });
  });
});
