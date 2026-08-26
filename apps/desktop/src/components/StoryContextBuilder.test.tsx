import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  StoryContextCatalogEntry,
  StoryContextCatalogSnapshot,
  StoryContextManifest,
  StoryContextManifestSource,
} from "../lib/tauri/story-context";

const storyContextMocks = vi.hoisted(() => ({
  build: vi.fn(),
  catalog: vi.fn(),
}));

vi.mock("../lib/tauri/story-context", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/tauri/story-context")>()),
  requestStoryContextBuild: storyContextMocks.build,
  requestStoryContextCatalog: storyContextMocks.catalog,
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

describe("NG-501 StoryContextBuilder", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    storyContextMocks.catalog.mockResolvedValue(catalog);
    storyContextMocks.build.mockResolvedValue(manifest);
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
    expect(screen.getByText(/NG-501 边界/u)).toBeInTheDocument();
    expect(JSON.stringify(storyContextMocks.build.mock.calls)).not.toMatch(
      /provider|apiKey|PRIVATE_CANARY/iu,
    );
  });
});
