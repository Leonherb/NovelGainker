import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  StoryContextCatalogEntry,
  StoryContextCatalogSnapshot,
  StoryContextManifest,
  StoryContextManifestSource,
  StoryContextRequestSnapshot,
  StoryReaderMentalModel,
} from "../lib/tauri/story-context";
import type { StoryChapterRun } from "../lib/tauri/story-chapter";
import type { StoryContinuityReport } from "../lib/tauri/story-continuity";
import type {
  ChapterPatchApplyResult,
  ChapterPatchPreview,
} from "../lib/tauri/story-chapter-patch";
import type {
  ChapterStatusPreview,
  LedgerPendingStatus,
  LedgerUpdatePreview,
  StoryChapterLifecycleStatus,
} from "../lib/tauri/story-lifecycle";

const storyContextMocks = vi.hoisted(() => ({
  build: vi.fn(),
  catalog: vi.fn(),
  preview: vi.fn(),
}));
const storyChapterMocks = vi.hoisted(() => ({
  cancel: vi.fn(),
  generate: vi.fn(),
  status: vi.fn(),
}));
const storyContinuityMocks = vi.hoisted(() => ({
  check: vi.fn(),
  confirm: vi.fn(),
  status: vi.fn(),
}));
const storyPatchMocks = vi.hoisted(() => ({
  apply: vi.fn(),
  preview: vi.fn(),
  status: vi.fn(),
}));
const storyLifecycleMocks = vi.hoisted(() => ({
  applyLedger: vi.fn(),
  applyState: vi.fn(),
  previewLedger: vi.fn(),
  previewState: vi.fn(),
  status: vi.fn(),
}));

vi.mock("../lib/tauri/story-context", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/tauri/story-context")>()),
  requestStoryContextBuild: storyContextMocks.build,
  requestStoryContextCatalog: storyContextMocks.catalog,
  requestStoryContextRequestPreview: storyContextMocks.preview,
}));

vi.mock("../lib/tauri/story-chapter", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/tauri/story-chapter")>()),
  requestStoryChapterCancel: storyChapterMocks.cancel,
  requestStoryChapterGenerateStart: storyChapterMocks.generate,
  requestStoryChapterStatus: storyChapterMocks.status,
}));

vi.mock("../lib/tauri/story-continuity", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/tauri/story-continuity")>()),
  requestStoryContinuityCheck: storyContinuityMocks.check,
  requestStoryContinuityStatus: storyContinuityMocks.status,
  requestStoryContinuityWarningConfirm: storyContinuityMocks.confirm,
}));

vi.mock("../lib/tauri/story-chapter-patch", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/tauri/story-chapter-patch")>()),
  requestStoryChapterPatchApply: storyPatchMocks.apply,
  requestStoryChapterPatchPreview: storyPatchMocks.preview,
  requestStoryChapterPatchStatus: storyPatchMocks.status,
}));

vi.mock("../lib/tauri/story-lifecycle", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/tauri/story-lifecycle")>()),
  requestStoryChapterLifecycleStatus: storyLifecycleMocks.status,
  requestStoryChapterStateApply: storyLifecycleMocks.applyState,
  requestStoryChapterStatePreview: storyLifecycleMocks.previewState,
  requestStoryLedgerUpdateApply: storyLifecycleMocks.applyLedger,
  requestStoryLedgerUpdatePreview: storyLifecycleMocks.previewLedger,
}));

import { StoryContextBuilder } from "./StoryContextBuilder";

const HASH_A = "a".repeat(64);
const HASH_B = "b".repeat(64);

function catalogSource(
  overrides: Partial<StoryContextCatalogEntry> = {},
): StoryContextCatalogEntry {
  return {
    sourceId: "project:plot/main.md",
    kind: "plot",
    relativePath: "plot/main.md",
    builtinResourceId: null,
    displayName: "主剧情",
    selectionReason: "defaultPlot",
    required: false,
    userDeselectable: true,
    defaultSelected: true,
    priority: 30,
    byteLength: 120,
    contentHash: HASH_A,
    modifiedTimeMs: 42,
    ...overrides,
  };
}

function manifestSource(
  catalog: StoryContextCatalogEntry,
  included: boolean,
): StoryContextManifestSource {
  return {
    sourceId: catalog.sourceId,
    kind: catalog.kind,
    relativePath: catalog.relativePath,
    builtinResourceId: catalog.builtinResourceId,
    displayName: catalog.displayName,
    selectionReason: catalog.selectionReason,
    required: catalog.required,
    userDeselectable: catalog.userDeselectable,
    priority: catalog.priority,
    sourceByteCount: catalog.byteLength,
    originalByteLength: catalog.byteLength,
    includedByteLength: included ? catalog.byteLength : 0,
    contentHash: catalog.contentHash,
    includedContentHash: included ? catalog.contentHash : HASH_B,
    truncation: included
      ? null
      : {
          reason: "notSelected",
          originalByteLength: catalog.byteLength,
          retainedByteLength: 0,
        },
    included,
    modifiedTimeMs: catalog.modifiedTimeMs,
  };
}

const requiredProtocol = catalogSource({
  sourceId: "builtin:storytelling/workflow",
  kind: "builtinProtocol",
  relativePath: null,
  builtinResourceId: "storytelling/workflow.md",
  displayName: "工作流协议",
  selectionReason: "required",
  required: true,
  userDeselectable: false,
  defaultSelected: true,
  priority: 1,
});
const projectRules = catalogSource({
  sourceId: "project:project.md",
  kind: "projectRules",
  relativePath: "project.md",
  displayName: "项目规则",
  selectionReason: "required",
  required: true,
  userDeselectable: false,
  defaultSelected: true,
  priority: 10,
});
const mainPlot = catalogSource();
const worldSource = catalogSource({
  sourceId: "project:world/潮汐城.md",
  kind: "world",
  relativePath: "world/潮汐城.md",
  displayName: "潮汐城",
  selectionReason: "available",
  defaultSelected: false,
  priority: 100,
  contentHash: HASH_B,
});

const catalog: StoryContextCatalogSnapshot = {
  schemaVersion: 2,
  workflowId: "novelgainker.storytelling",
  suggestedChapterNumber: 2,
  sources: [requiredProtocol, projectRules, mainPlot, worldSource],
};

const manifest: StoryContextManifest = {
  manifestVersion: 2,
  workflowId: "novelgainker.storytelling",
  workflowVersion: 1,
  action: "create",
  projectRootIdentity: "project_fixture",
  targetChapter: {
    chapterNumber: 2,
    chapterTitle: "潮痕",
    relativePath: "chapters/002.md",
  },
  selectedSources: [
    manifestSource(requiredProtocol, true),
    manifestSource(projectRules, true),
    manifestSource({ ...worldSource, selectionReason: "explicit", priority: 20 }, true),
  ],
  excludedSources: [manifestSource(mainPlot, false)],
  sourceOrder: [requiredProtocol.sourceId, projectRules.sourceId, worldSource.sourceId],
  budget: {
    requestLimitBytes: 96 * 1024,
    protocolBudgetBytes: 24 * 1024,
    projectContextBudgetBytes: 48 * 1024,
    safetyReserveBytes: 24 * 1024,
    singleSourceLimitBytes: 8 * 1024,
    protocolBytesUsed: 120,
    projectContextBytesUsed: 240,
    totalIncludedBytes: 360,
  },
  aggregateHash: "c".repeat(64),
  createdAt: 42,
};

const readerMentalModel: StoryReaderMentalModel = {
  readerKnows: "读者知道潮门已经关闭。",
  readerInfers: "",
  readerMisreads: "",
  readerQuestions: "",
  chapterPromises: "",
  forbiddenReveals: "",
};

function requestSnapshot(
  status: "current" | "needsReview" = "current",
): StoryContextRequestSnapshot {
  const currentManifest =
    status === "current" ? manifest : { ...manifest, aggregateHash: "f".repeat(64), createdAt: 43 };
  return {
    snapshotVersion: 1,
    manifest: currentManifest,
    readerMentalModel,
    readerModelHash: "d".repeat(64),
    requestHash: "e".repeat(64),
    sourceReview: {
      status,
      reviewedManifestHash: manifest.aggregateHash,
      currentManifestHash: currentManifest.aggregateHash,
    },
    budget: {
      requestLimitBytes: 96 * 1024,
      protocolBudgetBytes: 24 * 1024,
      projectContextBudgetBytes: 48 * 1024,
      safetyReserveBytes: 24 * 1024,
      singleSourceLimitBytes: 8 * 1024,
      queryResultLimitCount: 5,
      queryResultLimitBytes: 2 * 1024,
      recentChapterLimitCount: 2,
      maximumOutputTokens: 4_096,
      manifestProtocolBytes: 120,
      readerModelBytes: 180,
      requestEnvelopeReserveBytes: 512,
      protocolAndBriefBytesUsed: 812,
      projectContextBytesUsed: 240,
      requestBytesUsed: 1_052,
      requestBytesWithSafetyReserve: 25_628,
    },
    createdAt: currentManifest.createdAt,
  };
}

function completedChapterRun(): StoryChapterRun {
  return {
    schemaVersion: 1,
    runId: "run_fixture",
    state: "completed",
    providerId: "deepseek",
    modelId: "deepseek-v4-pro",
    sourceManifestHash: manifest.aggregateHash,
    requestHash: "e".repeat(64),
    startedAt: 100,
    completedAt: 101,
    repairAttempted: false,
    canCancel: false,
    canRetry: false,
    proposal: {
      proposalVersion: 1,
      proposalHash: "1".repeat(64),
      action: "create",
      targetChapter: manifest.targetChapter,
      title: "潮痕",
      draftMarkdown: "潮水退去，石阶上留下发光的纹路。",
      intentSummary: "推进潮门异响线索。",
      readerModelDelta: {
        ...readerMentalModel,
        readerKnows: "读者看见了发光纹路。",
      },
      continuityClaims: [{ claim: "潮门已关闭", sourceIds: ["project:project.md"] }],
      ledgerCandidates: [{ summary: "石阶出现发光纹路", sourceIds: ["project:project.md"] }],
      sourceManifestHash: manifest.aggregateHash,
      currentManifestHash: manifest.aggregateHash,
      requestHash: "e".repeat(64),
      providerId: "deepseek",
      modelId: "deepseek-v4-pro",
      startedAt: 100,
      generatedAt: 101,
      finishReason: "stop",
      responseComplete: true,
      sourceStatus: "current",
      continuityCheckVersion: null,
      writeEnabled: false,
    },
    invalidResponsePreview: null,
    error: null,
  };
}

function continuityReport(confirmed = false): StoryContinuityReport {
  const results = Array.from({ length: 17 }, (_, index) => ({
    ruleId: `rule.${index}`,
    severity:
      index === 0 ? ("warning" as const) : index === 1 ? ("unknown" as const) : ("passed" as const),
    title:
      index === 0 ? "禁揭示关键词扫描" : index === 1 ? "是否过早兑现未来纲要" : `通过规则 ${index}`,
    explanation: "固定本地规则解释。",
    evidence: ["fixture=value"],
    sourceIds: [],
    suggestedAction: "人工复核。",
  }));
  return {
    schemaVersion: 1,
    reportHash: "2".repeat(64),
    proposalHash: "1".repeat(64),
    sourceManifestHash: manifest.aggregateHash,
    currentManifestHash: manifest.aggregateHash,
    generationTargetHash: null,
    currentTargetHash: null,
    checkedAt: 102,
    localOnly: true,
    results,
    summary: { blocking: 0, warning: 1, unknown: 1, passed: 15 },
    acknowledgedWarningRuleIds: confirmed ? ["rule.0"] : [],
    acknowledgedUnknownRuleIds: confirmed ? ["rule.1"] : [],
    warningsConfirmed: confirmed,
    unknownsConfirmed: confirmed,
    reviewReady: confirmed,
    writeEnabled: false,
  };
}

function readyContinuityReport(): StoryContinuityReport {
  return continuityReport(true);
}

function chapterPatchPreview(): ChapterPatchPreview {
  return {
    schemaVersion: 1,
    previewHash: "3".repeat(64),
    proposalHash: "1".repeat(64),
    continuityReportHash: "2".repeat(64),
    sourceManifestHash: manifest.aggregateHash,
    currentManifestHash: manifest.aggregateHash,
    requestHash: "e".repeat(64),
    action: "create",
    targetRelativePath: "chapters/001.md",
    operation: "create",
    expectedSourceHash: null,
    proposedContentHash: "4".repeat(64),
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
            content: "潮水退去，石阶上留下发光的纹路。",
          },
        ],
      },
    ],
    summary: { addedLines: 1, removedLines: 0, unchangedLines: 0 },
    readyToApply: true,
  };
}

function chapterPatchApplied(): ChapterPatchApplyResult {
  return {
    transactionId: "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
    file: {
      relativePath: "chapters/001.md",
      contentHash: "4".repeat(64),
      modifiedTimeMs: 103,
      sizeBytes: 200,
      bom: "none",
      lineEnding: "lf",
      diagnosticsTriggered: true,
      diagnosticState: "ready",
      validation: {
        relativePath: "chapters/001.md",
        contentHash: "4".repeat(64),
        diagnostics: [],
        eligibleForAiApply: true,
        eligibleForFinal: true,
        eligibleForEpub: true,
      },
    },
  };
}

function lifecycleReceipt(path: string, hash: string): ChapterPatchApplyResult {
  const base = chapterPatchApplied();
  return {
    ...base,
    transactionId: "bbbbbbbb-cccc-4ddd-8eee-ffffffffffff",
    file: {
      ...base.file,
      relativePath: path,
      contentHash: hash,
      validation: {
        ...base.file.validation,
        relativePath: path,
        contentHash: hash,
      },
    },
  };
}

function chapterStatusPreview(): ChapterStatusPreview {
  return {
    schemaVersion: 1,
    previewHash: "5".repeat(64),
    targetRelativePath: "chapters/001.md",
    expectedSourceHash: "4".repeat(64),
    proposedContentHash: "6".repeat(64),
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
          {
            kind: "removed",
            oldLineNumber: 8,
            newLineNumber: null,
            content: "status: draft",
          },
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
}

function ledgerUpdatePreview(): LedgerUpdatePreview {
  return {
    schemaVersion: 1,
    previewHash: "7".repeat(64),
    targetRelativePath: "materials/000.story-ledger.md",
    chapterRelativePath: "chapters/001.md",
    expectedSourceHash: "8".repeat(64),
    proposedContentHash: "9".repeat(64),
    chapterContentHash: "6".repeat(64),
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
            content: "## 第 1 章《潮痕》状态更新",
          },
        ],
      },
    ],
    summary: { addedLines: 1, removedLines: 0, unchangedLines: 0 },
    readyToApply: true,
  };
}

function pendingStatus(resolution: "pending" | "applied"): LedgerPendingStatus {
  return {
    resolution,
    record: {
      schemaVersion: 1,
      pendingId: "ledger_pending_aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
      chapterId: "chapter_11111111-2222-4333-8444-555555555555",
      chapterNumber: 1,
      chapterTitle: "潮痕",
      chapterRelativePath: "chapters/001.md",
      acceptedStatus: "accepted",
      sourceChapterHash: "4".repeat(64),
      proposalHash: "1".repeat(64),
      sourceManifestHash: manifest.aggregateHash,
      requestHash: "e".repeat(64),
      candidates: [{ summary: "石阶出现发光纹路", sourceIds: ["project:project.md"] }],
    },
  };
}

function lifecycleStatus(
  chapterStatus: "draft" | "accepted",
  ledgerPending: LedgerPendingStatus[],
  statusPreview: ChapterStatusPreview | null = null,
  statusApplied: ChapterPatchApplyResult | null = null,
  ledgerPreview: LedgerUpdatePreview | null = null,
  ledgerApplied: ChapterPatchApplyResult | null = null,
): StoryChapterLifecycleStatus {
  return {
    schemaVersion: 1,
    targetRelativePath: "chapters/001.md",
    chapterStatus,
    ledgerPending,
    statusPreview,
    statusApplied,
    ledgerPreview,
    ledgerApplied,
  };
}

describe("NG-501～NG-504 StoryContextBuilder", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    storyContextMocks.catalog.mockResolvedValue(catalog);
    storyContextMocks.build.mockResolvedValue(manifest);
    storyContextMocks.preview.mockResolvedValue(requestSnapshot());
    storyChapterMocks.status.mockResolvedValue({ schemaVersion: 1, run: null });
    storyChapterMocks.generate.mockResolvedValue(completedChapterRun());
    storyContinuityMocks.status.mockResolvedValue({ schemaVersion: 1, report: null });
    storyContinuityMocks.check.mockResolvedValue(continuityReport());
    storyContinuityMocks.confirm.mockResolvedValue(continuityReport(true));
    storyPatchMocks.status.mockResolvedValue({ schemaVersion: 1, preview: null, applied: null });
    storyLifecycleMocks.status.mockResolvedValue({
      schemaVersion: 1,
      targetRelativePath: "chapters/001.md",
      chapterStatus: "draft",
      ledgerPending: [],
      statusPreview: null,
      statusApplied: null,
      ledgerPreview: null,
      ledgerApplied: null,
    });
  });

  it("NG-505 shows an exact local diff and writes only after the explicit apply click", async () => {
    const user = userEvent.setup();
    const report = readyContinuityReport();
    const preview = chapterPatchPreview();
    const applied = chapterPatchApplied();
    storyContinuityMocks.check.mockResolvedValue(report);
    storyPatchMocks.preview.mockResolvedValue(preview);
    storyPatchMocks.apply.mockResolvedValue(applied);
    render(<StoryContextBuilder onClose={vi.fn()} sessionId="session_fixture" />);

    await user.type(await screen.findByLabelText("章节标题"), "潮痕");
    await user.click(screen.getByRole("button", { name: "构建 Context Manifest" }));
    await user.click(await screen.findByRole("button", { name: "生成只读请求快照" }));
    await user.click(await screen.findByRole("button", { name: "生成只读章节提案" }));
    await waitFor(() => expect(screen.getByText("只读章节提案已完成")).toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "运行本地连续性检查" }));
    await user.click(await screen.findByRole("button", { name: "生成写入差异" }));

    expect(await screen.findByText("潮水退去，石阶上留下发光的纹路。")).toBeInTheDocument();
    expect(storyPatchMocks.apply).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认并写入此章节差异" }));
    await waitFor(() =>
      expect(storyPatchMocks.apply).toHaveBeenCalledWith("session_fixture", "run_fixture", preview),
    );
    expect(await screen.findByText(/已写入 chapters\/001\.md；事务 ID/u)).toBeInTheDocument();
  });

  it("NG-506 keeps chapter acceptance and Ledger update as two visible confirmations", async () => {
    const user = userEvent.setup();
    const report = readyContinuityReport();
    const patchPreview = chapterPatchPreview();
    const patchApplied = chapterPatchApplied();
    const statusPreview = chapterStatusPreview();
    const statusApplied = lifecycleReceipt("chapters/001.md", "6".repeat(64));
    const ledgerPreview = ledgerUpdatePreview();
    const ledgerApplied = lifecycleReceipt("materials/000.story-ledger.md", "9".repeat(64));
    storyContinuityMocks.check.mockResolvedValue(report);
    storyPatchMocks.preview.mockResolvedValue(patchPreview);
    storyPatchMocks.apply.mockResolvedValue(patchApplied);
    storyLifecycleMocks.previewState.mockResolvedValue(statusPreview);
    storyLifecycleMocks.applyState.mockResolvedValue(statusApplied);
    storyLifecycleMocks.previewLedger.mockResolvedValue(ledgerPreview);
    storyLifecycleMocks.applyLedger.mockResolvedValue(ledgerApplied);
    storyLifecycleMocks.status
      .mockResolvedValueOnce(lifecycleStatus("draft", []))
      .mockResolvedValueOnce(
        lifecycleStatus("accepted", [pendingStatus("pending")], statusPreview, statusApplied),
      )
      .mockResolvedValueOnce(
        lifecycleStatus(
          "accepted",
          [pendingStatus("applied")],
          statusPreview,
          statusApplied,
          ledgerPreview,
          ledgerApplied,
        ),
      );
    render(<StoryContextBuilder onClose={vi.fn()} sessionId="session_fixture" />);

    await user.type(await screen.findByLabelText("章节标题"), "潮痕");
    await user.click(screen.getByRole("button", { name: "构建 Context Manifest" }));
    await user.click(await screen.findByRole("button", { name: "生成只读请求快照" }));
    await user.click(await screen.findByRole("button", { name: "生成只读章节提案" }));
    await waitFor(() => expect(screen.getByText("只读章节提案已完成")).toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "运行本地连续性检查" }));
    await user.click(await screen.findByRole("button", { name: "生成写入差异" }));
    await user.click(screen.getByRole("button", { name: "确认并写入此章节差异" }));

    await user.click(await screen.findByRole("button", { name: "预览接受差异" }));
    expect(storyLifecycleMocks.applyState).not.toHaveBeenCalled();
    expect(storyLifecycleMocks.applyLedger).not.toHaveBeenCalled();
    await user.click(await screen.findByRole("button", { name: "确认接受章节" }));
    await waitFor(() =>
      expect(storyLifecycleMocks.applyState).toHaveBeenCalledWith("session_fixture", statusPreview),
    );
    expect(storyLifecycleMocks.applyLedger).not.toHaveBeenCalled();

    await user.click(await screen.findByRole("button", { name: "生成 Ledger 差异" }));
    expect(storyLifecycleMocks.applyLedger).not.toHaveBeenCalled();
    await user.click(await screen.findByRole("button", { name: "确认并更新 Story Ledger" }));
    await waitFor(() =>
      expect(storyLifecycleMocks.applyLedger).toHaveBeenCalledWith(
        "session_fixture",
        ledgerPreview,
      ),
    );
    expect(storyLifecycleMocks.applyState.mock.invocationCallOrder[0]).toBeLessThan(
      storyLifecycleMocks.applyLedger.mock.invocationCallOrder[0],
    );
  });

  it("keeps required sources locked, makes optional selection explicit, and renders the manifest", async () => {
    const user = userEvent.setup();
    render(<StoryContextBuilder onClose={vi.fn()} sessionId="session_fixture" />);

    const required = await screen.findByRole("checkbox", { name: /工作流协议/u });
    const plot = screen.getByRole("checkbox", { name: /主剧情/u });
    const world = screen.getByRole("checkbox", { name: /潮汐城/u });
    expect(required).toBeChecked();
    expect(required).toBeDisabled();
    expect(plot).toBeChecked();
    expect(world).not.toBeChecked();

    await user.click(plot);
    await user.click(world);
    await user.type(screen.getByLabelText("章节标题"), "潮痕");
    await user.click(screen.getByRole("button", { name: "构建 Context Manifest" }));

    await waitFor(() =>
      expect(storyContextMocks.build).toHaveBeenCalledWith("session_fixture", {
        action: "create",
        chapterNumber: 2,
        chapterTitle: "潮痕",
        selectedSourcePaths: ["world/潮汐城.md"],
      }),
    );
    expect(await screen.findByText("Context Manifest 已构建")).toBeInTheDocument();
    expect(screen.getByText("chapters/002.md")).toBeInTheDocument();
    expect(screen.getByText("用户显式选择")).toBeInTheDocument();
    expect(screen.getByText("查看排除来源（1）")).toBeInTheDocument();
    expect(screen.getByText(/NG-504 边界/u)).toBeInTheDocument();
    expect(JSON.stringify(storyContextMocks.build.mock.calls)).not.toMatch(
      /provider|apiKey|PRIVATE_CANARY/iu,
    );
  });

  it("edits all six Reader Model fields and produces a local request hash", async () => {
    const user = userEvent.setup();
    render(<StoryContextBuilder onClose={vi.fn()} sessionId="session_fixture" />);
    await screen.findByRole("checkbox", { name: /工作流协议/u });
    await user.type(screen.getByLabelText("章节标题"), "潮痕");
    await user.click(screen.getByRole("button", { name: "构建 Context Manifest" }));
    await screen.findByText("Context Manifest 已构建");

    expect(screen.getAllByRole("textbox")).toHaveLength(7);
    await user.type(screen.getByLabelText(/读者已知/u), readerMentalModel.readerKnows);
    await user.type(screen.getByLabelText(/读者推断/u), "林遥可能隐瞒真相。");
    await user.type(screen.getByLabelText(/允许误读/u), "异响来自城外。");
    await user.type(screen.getByLabelText(/未决问题/u), "潮声来自哪里？");
    await user.type(screen.getByLabelText(/本章承诺/u), "推进异响线索。");
    await user.type(screen.getByLabelText(/禁止提前揭露/u), "守门人的身份。");
    await user.click(screen.getByRole("button", { name: "生成只读请求快照" }));

    await waitFor(() =>
      expect(storyContextMocks.preview).toHaveBeenCalledWith("session_fixture", {
        action: "create",
        chapterNumber: 2,
        chapterTitle: "潮痕",
        selectedSourcePaths: ["plot/main.md"],
        reviewedManifestHash: manifest.aggregateHash,
        readerMentalModel: {
          readerKnows: readerMentalModel.readerKnows,
          readerInfers: "林遥可能隐瞒真相。",
          readerMisreads: "异响来自城外。",
          readerQuestions: "潮声来自哪里？",
          chapterPromises: "推进异响线索。",
          forbiddenReveals: "守门人的身份。",
        },
      }),
    );
    expect(await screen.findByText("来源已复核")).toBeInTheDocument();
    expect(screen.getByTitle("e".repeat(64))).toBeInTheDocument();
    expect(screen.getByText("4096 tokens")).toBeInTheDocument();
  });

  it("keeps edited Reader Model text when a source changes and asks for review", async () => {
    const user = userEvent.setup();
    storyContextMocks.preview.mockResolvedValue(requestSnapshot("needsReview"));
    render(<StoryContextBuilder onClose={vi.fn()} sessionId="session_fixture" />);
    await screen.findByRole("checkbox", { name: /工作流协议/u });
    await user.type(screen.getByLabelText("章节标题"), "潮痕");
    await user.click(screen.getByRole("button", { name: "构建 Context Manifest" }));
    const knows = screen.getByLabelText(/读者已知/u);
    await user.type(knows, readerMentalModel.readerKnows);
    await user.click(screen.getByRole("button", { name: "生成只读请求快照" }));

    expect(await screen.findByText("来源有变化")).toBeInTheDocument();
    expect(screen.getByText(/你的六段编辑已原样保留/u)).toBeInTheDocument();
    expect(knows).toHaveValue(readerMentalModel.readerKnows);
    expect(screen.getByRole("button", { name: "重新确认变化后的来源" })).toBeInTheDocument();
  });

  it("explicitly starts a provider-bound read-only proposal and renders trace without write controls", async () => {
    const user = userEvent.setup();
    render(<StoryContextBuilder onClose={vi.fn()} sessionId="session_fixture" />);
    await screen.findByRole("checkbox", { name: /工作流协议/u });
    await user.type(screen.getByLabelText("章节标题"), "潮痕");
    await user.click(screen.getByRole("button", { name: "构建 Context Manifest" }));
    await screen.findByText("Context Manifest 已构建");
    await user.click(screen.getByRole("button", { name: "生成只读请求快照" }));
    await screen.findByText("来源已复核");
    await user.click(screen.getByRole("button", { name: "生成只读章节提案" }));

    await waitFor(() =>
      expect(storyChapterMocks.generate).toHaveBeenCalledWith("session_fixture", {
        action: "create",
        chapterNumber: 2,
        chapterTitle: "潮痕",
        selectedSourcePaths: ["plot/main.md"],
        reviewedManifestHash: manifest.aggregateHash,
        readerMentalModel,
        expectedRequestHash: "e".repeat(64),
      }),
    );
    expect(await screen.findByText("只读章节提案已完成")).toBeInTheDocument();
    expect(screen.getByText("潮水退去，石阶上留下发光的纹路。")).toBeInTheDocument();
    expect(screen.getByText("deepseek · deepseek-v4-pro")).toBeInTheDocument();
    expect(screen.getByText(/章节提案与连续性报告始终是只读对象/u)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "生成写入差异" })).toBeDisabled();
    expect(screen.queryByRole("button", { name: "确认并写入此章节差异" })).not.toBeInTheDocument();
    expect(JSON.stringify(storyChapterMocks.generate.mock.calls)).not.toMatch(/apiKey/iu);
  });

  it("keeps unknown visible and requires session-only author confirmation before patch preview", async () => {
    const user = userEvent.setup();
    render(<StoryContextBuilder onClose={vi.fn()} sessionId="session_fixture" />);
    await screen.findByRole("checkbox", { name: /工作流协议/u });
    await user.type(screen.getByLabelText("章节标题"), "潮痕");
    await user.click(screen.getByRole("button", { name: "构建 Context Manifest" }));
    await user.click(screen.getByRole("button", { name: "生成只读请求快照" }));
    await user.click(screen.getByRole("button", { name: "生成只读章节提案" }));
    await screen.findByText("只读章节提案已完成");
    await user.click(screen.getByRole("button", { name: "运行本地连续性检查" }));

    expect(await screen.findByText("禁揭示关键词扫描")).toBeInTheDocument();
    expect(screen.getByText("是否过早兑现未来纲要")).toBeInTheDocument();
    expect(screen.getByText(/不会调用第二个 AI/u)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "确认本次写入会话中的全部警告与未知项" }));
    expect(await screen.findByText(/警告与未知项的作者确认已记录/u)).toBeInTheDocument();
    expect(screen.getByText("是否过早兑现未来纲要")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "生成写入差异" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "确认并写入此章节差异" })).not.toBeInTheDocument();
    expect(JSON.stringify(storyContinuityMocks.check.mock.calls)).not.toMatch(
      /apiKey|endpoint|modelId/iu,
    );
  });
});
