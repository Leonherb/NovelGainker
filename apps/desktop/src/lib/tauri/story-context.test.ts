import { describe, expect, it, vi } from "vitest";
import {
  parseStoryContextCatalog,
  parseStoryContextManifest,
  requestStoryContextBuild,
} from "./story-context";

const hash = "a".repeat(64);
const includedHash = "b".repeat(64);

function catalogSource(overrides: Record<string, unknown> = {}) {
  return {
    sourceId: "project:plot/main.md",
    kind: "plot",
    relativePath: "plot/main.md",
    builtinResourceId: null,
    displayName: "main",
    selectionReason: "defaultPlot",
    required: false,
    userDeselectable: true,
    defaultSelected: true,
    priority: 30,
    byteLength: 120,
    contentHash: hash,
    modifiedTimeMs: 42,
    ...overrides,
  };
}

function manifestSource(included: boolean, overrides: Record<string, unknown> = {}) {
  return {
    sourceId: "project:plot/main.md",
    kind: "plot",
    relativePath: "plot/main.md",
    builtinResourceId: null,
    displayName: "main",
    selectionReason: "defaultPlot",
    required: false,
    userDeselectable: true,
    priority: 30,
    sourceByteCount: 120,
    originalByteLength: 120,
    includedByteLength: included ? 120 : 0,
    contentHash: hash,
    includedContentHash: includedHash,
    truncation: included
      ? null
      : { reason: "notSelected", originalByteLength: 120, retainedByteLength: 0 },
    included,
    modifiedTimeMs: 42,
    ...overrides,
  };
}

function manifestValue() {
  return {
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
    selectedSources: [manifestSource(true)],
    excludedSources: [
      manifestSource(false, {
        sourceId: "project:characters/林遥.md",
        relativePath: "characters/林遥.md",
        kind: "character",
        displayName: "林遥",
        selectionReason: "available",
      }),
    ],
    sourceOrder: ["project:plot/main.md"],
    budget: {
      requestLimitBytes: 96 * 1024,
      protocolBudgetBytes: 24 * 1024,
      projectContextBudgetBytes: 48 * 1024,
      safetyReserveBytes: 24 * 1024,
      singleSourceLimitBytes: 8 * 1024,
      protocolBytesUsed: 0,
      projectContextBytesUsed: 120,
      totalIncludedBytes: 120,
    },
    aggregateHash: "c".repeat(64),
    createdAt: 42,
  };
}

describe("Story Context Tauri contract", () => {
  it("accepts a metadata-only catalog and rejects duplicate or body-bearing entries", () => {
    const required = [
      catalogSource({
        sourceId: "builtin:storytelling/workflow",
        kind: "builtinProtocol",
        relativePath: null,
        builtinResourceId: "storytelling/workflow.md",
        displayName: "工作流协议",
        selectionReason: "required",
        required: true,
        userDeselectable: false,
        priority: 1,
      }),
      catalogSource({
        sourceId: "builtin:storytelling/input-schema",
        kind: "builtinSchema",
        relativePath: null,
        builtinResourceId: "storytelling/input.schema.json",
        displayName: "输入 Schema",
        selectionReason: "required",
        required: true,
        userDeselectable: false,
        priority: 2,
      }),
      catalogSource({
        sourceId: "builtin:storytelling/output-schema",
        kind: "builtinSchema",
        relativePath: null,
        builtinResourceId: "storytelling/output.schema.json",
        displayName: "输出 Schema",
        selectionReason: "required",
        required: true,
        userDeselectable: false,
        priority: 3,
      }),
      catalogSource({
        sourceId: "builtin:storytelling/chapter-template",
        kind: "builtinTemplate",
        relativePath: null,
        builtinResourceId: "storytelling/chapter.template.md",
        displayName: "章节模板",
        selectionReason: "required",
        required: true,
        userDeselectable: false,
        priority: 4,
      }),
      catalogSource({
        sourceId: "project:project.md",
        kind: "projectRules",
        relativePath: "project.md",
        displayName: "项目规则",
        selectionReason: "required",
        required: true,
        userDeselectable: false,
        priority: 10,
      }),
      catalogSource({
        sourceId: "project:AGENTS.md",
        kind: "projectRules",
        relativePath: "AGENTS.md",
        displayName: "项目协作规则",
        selectionReason: "required",
        required: true,
        userDeselectable: false,
        priority: 11,
      }),
    ];
    const value = {
      schemaVersion: 2,
      workflowId: "novelgainker.storytelling",
      suggestedChapterNumber: 2,
      sources: [...required, catalogSource()],
    };
    expect(parseStoryContextCatalog(value).sources).toHaveLength(7);
    expect(() =>
      parseStoryContextCatalog({
        ...value,
        sources: [...value.sources, { ...catalogSource(), content: "PRIVATE_CANARY" }],
      }),
    ).toThrow();
  });

  it("accepts only ordered, hash-bound fixed-budget manifests", () => {
    expect(parseStoryContextManifest(manifestValue()).targetChapter.relativePath).toBe(
      "chapters/002.md",
    );
    expect(() =>
      parseStoryContextManifest({ ...manifestValue(), sourceOrder: ["forged"] }),
    ).toThrow();
    expect(() =>
      parseStoryContextManifest({
        ...manifestValue(),
        budget: { ...manifestValue().budget, requestLimitBytes: 1 },
      }),
    ).toThrow();
  });

  it("sorts explicit paths and sends only bounded build metadata", async () => {
    const invokeCommand = vi.fn().mockResolvedValue(manifestValue());
    await requestStoryContextBuild(
      "session_fixture",
      {
        action: "create",
        chapterNumber: 2,
        chapterTitle: " 潮痕 ",
        selectedSourcePaths: ["world/潮汐.md", "plot/main.md"],
      },
      invokeCommand,
    );
    expect(invokeCommand).toHaveBeenCalledWith("story_context_build", {
      sessionId: "session_fixture",
      input: {
        schemaVersion: 2,
        action: "create",
        chapterNumber: 2,
        chapterTitle: "潮痕",
        selectedSourcePaths: ["plot/main.md", "world/潮汐.md"],
      },
    });
  });
});
