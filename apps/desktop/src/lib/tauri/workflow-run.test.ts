import { describe, expect, it, vi } from "vitest";
import workflowRunFixture from "../../../../../contracts/ipc/workflow-run.v3.json";
import {
  parseWorkflowRunEvent,
  parseWorkflowRunStatusSnapshot,
  parseWorkflowSourceCatalog,
  requestCharacterWorkflowRunPrepare,
  requestCharacterWorkflowSourceCatalog,
  requestMaterialWorkflowRunPrepare,
  requestMaterialWorkflowSourceCatalog,
  requestPlotWorkflowRunPrepare,
  requestPlotWorkflowSourceCatalog,
  requestWorkflowProposalApply,
  requestWorkflowRunAbandon,
  requestWorkflowRunCancel,
  requestWorkflowRunGenerate,
  requestWorkflowRunPrepare,
  requestSettingWorkflowRunPrepare,
  requestWorkflowSourceCatalog,
  requestWorkflowRunStatus,
  WORKFLOW_RUN_ABANDON_COMMAND,
  WORKFLOW_RUN_CANCEL_COMMAND,
  WORKFLOW_RUN_GENERATE_COMMAND,
  WORKFLOW_PROPOSAL_APPLY_COMMAND,
  WORKFLOW_SOURCE_CATALOG_COMMAND,
  WORKFLOW_RUN_PREPARE_COMMAND,
  WORKFLOW_RUN_STATUS_LOAD_COMMAND,
} from "./workflow-run";

describe("NG-304 Workflow Run IPC contract", () => {
  it("accepts the exact context-ready Manifest and rejects hidden or forged fields", () => {
    expect(parseWorkflowRunStatusSnapshot(workflowRunFixture)).toEqual(workflowRunFixture);
    expect(() =>
      parseWorkflowRunStatusSnapshot({
        ...workflowRunFixture,
        contextManifest: {
          ...workflowRunFixture.contextManifest,
          prompt: "secret",
        },
      }),
    ).toThrow();
    expect(() =>
      parseWorkflowRunStatusSnapshot({
        ...workflowRunFixture,
        contextManifest: {
          ...workflowRunFixture.contextManifest,
          providerId: "openai",
          model: "gpt-5.6-terra",
        },
      }),
    ).toThrow();
    expect(() =>
      parseWorkflowRunStatusSnapshot({
        ...workflowRunFixture,
        contextManifest: {
          ...workflowRunFixture.contextManifest,
          sources: [
            {
              ...workflowRunFixture.contextManifest.sources[0],
              relativePath: "characters/secret.md",
            },
          ],
        },
      }),
    ).toThrow();
    expect(() =>
      parseWorkflowRunStatusSnapshot({
        schemaVersion: 3,
        available: false,
        hasActiveRun: false,
        run: null,
        contextManifest: workflowRunFixture.contextManifest,
        proposalView: null,
      }),
    ).toThrow();
  });

  it("uses only the fixed prepare, confirmed-generate and session control arguments", async () => {
    const invoke = vi.fn().mockResolvedValue(workflowRunFixture);
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const runId = workflowRunFixture.run.runId;
    const manifestHash = workflowRunFixture.contextManifest.manifestHash;

    await requestWorkflowRunStatus(sessionId, invoke);
    await requestWorkflowRunPrepare(sessionId, "  冷静的档案员\r\n隐藏着一个秘密  ", invoke);
    await requestWorkflowRunGenerate(runId, manifestHash, true, invoke);
    await requestWorkflowProposalApply(runId, "f".repeat(64), true, invoke);
    await requestWorkflowRunCancel(sessionId, invoke);
    await requestWorkflowRunAbandon(sessionId, true, invoke);

    expect(invoke.mock.calls).toEqual([
      [WORKFLOW_RUN_STATUS_LOAD_COMMAND, { sessionId }],
      [
        WORKFLOW_RUN_PREPARE_COMMAND,
        {
          sessionId,
          workflowId: "novelgainker.character-create",
          input: {
            schemaVersion: 2,
            mode: "create",
            brief: "冷静的档案员\n隐藏着一个秘密",
          },
          selectedSourcePaths: [],
          replaceTargetPath: null,
        },
      ],
      [WORKFLOW_RUN_GENERATE_COMMAND, { runId, manifestHash, confirmed: true }],
      [WORKFLOW_PROPOSAL_APPLY_COMMAND, { runId, proposalHash: "f".repeat(64), confirmed: true }],
      [WORKFLOW_RUN_CANCEL_COMMAND, { sessionId }],
      [WORKFLOW_RUN_ABANDON_COMMAND, { sessionId, confirmed: true }],
    ]);
    expect(JSON.stringify(invoke.mock.calls[2])).not.toMatch(
      /session|path|prompt|message|context|proposal|diff|key/iu,
    );
    expect(invoke.mock.calls[3][1]).toEqual({
      runId,
      proposalHash: "f".repeat(64),
      confirmed: true,
    });
    expect(invoke.mock.calls[3][1]).not.toHaveProperty("content");
    expect(invoke.mock.calls[3][1]).not.toHaveProperty("relativePath");
    expect(invoke.mock.calls[3][1]).not.toHaveProperty("tool");
  });

  it("keeps character v1/v2 sources explicit, bounded and sends only CharacterBriefV2", async () => {
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const entry = (
      relativePath: string,
      sourceKind: "character" | "setting" | "material" | "plot",
      entityId: string | null,
      entitySchemaVersion: 1 | 2,
    ) => ({
      relativePath,
      sourceKind,
      displayName: relativePath.split("/").slice(-1)[0]?.replace(/\.md$/u, "") ?? relativePath,
      entityId,
      entitySchemaVersion,
      settingId: null,
      settingType: null,
      materialId: null,
      materialType: null,
      contentHash: "c".repeat(64),
      byteCount: 256,
    });
    const legacy = entry("characters/旧人物.md", "character", null, 1);
    const current = entry(
      "characters/林遥.md",
      "character",
      "character_20000000-0000-4000-8000-000000000002",
      2,
    );
    const material = entry(
      "materials/铜钥匙.md",
      "material",
      "material_30000000-0000-4000-8000-000000000003",
      1,
    );
    const plot = entry("plot/main.md", "plot", "plot_40000000-0000-4000-8000-000000000004", 1);
    const setting = entry(
      "world/雾城.md",
      "setting",
      "setting_50000000-0000-4000-8000-000000000005",
      1,
    );
    const catalog = {
      schemaVersion: 1,
      workflowId: "novelgainker.character-create",
      sources: [legacy, current, material, plot, setting],
      replaceTargets: [legacy, current],
    } as const;
    const invoke = vi.fn().mockResolvedValueOnce(catalog).mockResolvedValueOnce(workflowRunFixture);

    await expect(requestCharacterWorkflowSourceCatalog(sessionId, invoke)).resolves.toEqual(
      catalog,
    );
    await expect(
      requestCharacterWorkflowRunPrepare(
        sessionId,
        { schemaVersion: 2, mode: "enhance", brief: " 完善旧人物的动机与关系 " },
        ["world/雾城.md", "characters/旧人物.md"],
        "characters/旧人物.md",
        invoke,
      ),
    ).resolves.toEqual(workflowRunFixture);
    expect(invoke.mock.calls).toEqual([
      [WORKFLOW_SOURCE_CATALOG_COMMAND, { sessionId, workflowId: "novelgainker.character-create" }],
      [
        WORKFLOW_RUN_PREPARE_COMMAND,
        {
          sessionId,
          workflowId: "novelgainker.character-create",
          input: { schemaVersion: 2, mode: "enhance", brief: "完善旧人物的动机与关系" },
          selectedSourcePaths: ["characters/旧人物.md", "world/雾城.md"],
          replaceTargetPath: "characters/旧人物.md",
        },
      ],
    ]);

    const rejectedInvoke = vi.fn();
    await expect(
      requestCharacterWorkflowRunPrepare(
        sessionId,
        { schemaVersion: 2, mode: "create", brief: "来源过多" },
        Array.from({ length: 7 }, (_, index) => `characters/人物${index}.md`),
        null,
        rejectedInvoke,
      ),
    ).rejects.toMatchObject({ code: "workflow.source_limit_exceeded" });
    await expect(
      requestCharacterWorkflowRunPrepare(
        sessionId,
        { schemaVersion: 2, mode: "enhance", brief: "目标未选中" },
        ["world/雾城.md"],
        "characters/旧人物.md",
        rejectedInvoke,
      ),
    ).rejects.toMatchObject({ code: "workflow.source_not_allowed" });
    expect(rejectedInvoke).not.toHaveBeenCalled();

    expect(() =>
      parseWorkflowSourceCatalog({
        ...catalog,
        sources: [legacy, current, { ...material, entityId: current.entityId }],
      }),
    ).toThrow();
  });

  it("exposes metadata-only setting sources and sends only the fixed SettingBrief contract", async () => {
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const settingSource = {
      relativePath: "world/旧都.md",
      sourceKind: "setting",
      entityId: null,
      entitySchemaVersion: null,
      settingId: "setting_10000000-0000-4000-8000-000000000001",
      displayName: "旧都",
      settingType: "location",
      materialId: null,
      materialType: null,
      contentHash: "c".repeat(64),
      byteCount: 256,
    } as const;
    const catalog = {
      schemaVersion: 1,
      workflowId: "novelgainker.setting-create",
      sources: [settingSource],
      replaceTargets: [settingSource],
    } as const;
    const settingSnapshot = {
      ...workflowRunFixture,
      run: {
        ...workflowRunFixture.run,
        workflowId: "novelgainker.setting-create",
        workflowVersion: 1,
      },
      contextManifest: {
        ...workflowRunFixture.contextManifest,
        workflowId: "novelgainker.setting-create",
        workflowVersion: 1,
        sources: [
          ...workflowRunFixture.contextManifest.sources,
          {
            ...workflowRunFixture.contextManifest.sources[0],
            relativePath: "world/旧都.md",
            contentHash: "f".repeat(64),
            byteCount: 18,
            characterCount: 6,
            includedByteCount: 18,
            includedCharacterCount: 6,
            inclusionReason: "selected_setting_context",
            boundedPreview: "旧都设定摘要",
          },
        ],
      },
    } as const;
    const invoke = vi.fn().mockResolvedValueOnce(catalog).mockResolvedValueOnce(settingSnapshot);

    expect(parseWorkflowSourceCatalog(catalog)).toEqual(catalog);
    expect(() =>
      parseWorkflowSourceCatalog({
        ...catalog,
        sources: [{ ...catalog.sources[0], body: "PRIVATE_SOURCE_CANARY" }],
      }),
    ).toThrow();

    await expect(requestWorkflowSourceCatalog(sessionId, invoke)).resolves.toEqual(catalog);
    await expect(
      requestSettingWorkflowRunPrepare(
        sessionId,
        {
          schemaVersion: 1,
          displayName: " 雾城 ",
          settingType: "location",
          objective: " 建立压迫感 ",
          mustInclude: [" 终年浓雾 ", " 钟声决定宵禁 "],
          constraints: null,
        },
        ["world/旧都.md"],
        "world/旧都.md",
        invoke,
      ),
    ).resolves.toEqual(settingSnapshot);

    expect(invoke.mock.calls).toEqual([
      [WORKFLOW_SOURCE_CATALOG_COMMAND, { sessionId, workflowId: "novelgainker.setting-create" }],
      [
        WORKFLOW_RUN_PREPARE_COMMAND,
        {
          sessionId,
          workflowId: "novelgainker.setting-create",
          input: {
            schemaVersion: 1,
            displayName: "雾城",
            settingType: "location",
            objective: "建立压迫感",
            mustInclude: ["终年浓雾", "钟声决定宵禁"],
            constraints: null,
          },
          selectedSourcePaths: ["world/旧都.md"],
          replaceTargetPath: "world/旧都.md",
        },
      ],
    ]);
    expect(JSON.stringify(invoke.mock.calls)).not.toContain("PRIVATE_SOURCE_CANARY");

    const rejectedInvoke = vi.fn();
    await expect(
      requestSettingWorkflowRunPrepare(
        sessionId,
        {
          schemaVersion: 1,
          displayName: "雾城",
          settingType: "location",
          objective: "建立压迫感",
          mustInclude: ["终年浓雾"],
          constraints: null,
        },
        Array.from({ length: 7 }, (_, index) => "world/来源" + index + ".md"),
        null,
        rejectedInvoke,
      ),
    ).rejects.toMatchObject({ code: "workflow.source_limit_exceeded" });
    await expect(
      requestSettingWorkflowRunPrepare(
        sessionId,
        {
          schemaVersion: 1,
          displayName: "雾城",
          settingType: "location",
          objective: "建立压迫感",
          mustInclude: ["终年浓雾"],
          constraints: null,
        },
        [],
        "world/旧都.md",
        rejectedInvoke,
      ),
    ).rejects.toMatchObject({ code: "workflow.source_not_allowed" });
    expect(rejectedInvoke).not.toHaveBeenCalled();
  });

  it("accepts a full create preview and rejects forged Proposal fields", () => {
    const fullContent = "---\ntitle: A\nwritingLanguage: zh-CN\n---\n\n# A\n";
    const proposalSnapshot = {
      ...workflowRunFixture,
      run: {
        ...workflowRunFixture.run,
        state: "awaiting_confirmation",
        sequence: 6,
      },
      contextManifest: null,
      proposalView: {
        schemaVersion: 1,
        proposalId: "30000000-0000-4000-8000-000000000003",
        proposalHash: "f".repeat(64),
        runId: workflowRunFixture.run.runId,
        workflowId: "novelgainker.character-create",
        workflowVersion: 2,
        packHash: "a".repeat(64),
        providerId: "deepseek",
        presetVersion: 1,
        model: "deepseek-v4-pro",
        manifestHash: "e".repeat(64),
        operation: {
          kind: "create",
          relativePath: "characters/A.md",
          expectedSourceHash: "absent",
          proposedContentHash: "d".repeat(64),
          proposedByteCount: 45,
        },
        validationSummary: {
          strict: true,
          eligibleForAiApply: true,
          diagnosticCodes: [],
        },
        warningCodes: [],
        fullContent,
        lineDiff: [],
      },
    } as const;
    expect(parseWorkflowRunStatusSnapshot(proposalSnapshot)).toEqual(proposalSnapshot);
    const settingReplaceSnapshot = {
      ...proposalSnapshot,
      run: {
        ...proposalSnapshot.run,
        workflowId: "novelgainker.setting-create",
        workflowVersion: 1,
      },
      proposalView: {
        ...proposalSnapshot.proposalView,
        workflowId: "novelgainker.setting-create",
        workflowVersion: 1,
        operation: {
          ...proposalSnapshot.proposalView.operation,
          kind: "replace",
          relativePath: "world/旧都.md",
          expectedSourceHash: "c".repeat(64),
        },
        lineDiff: [
          { kind: "remove", oldLine: 6, newLine: null, content: "# 旧都" },
          { kind: "add", oldLine: null, newLine: 6, content: "# 焕新旧都" },
        ],
      },
    } as const;
    expect(parseWorkflowRunStatusSnapshot(settingReplaceSnapshot)).toEqual(settingReplaceSnapshot);
    expect(() =>
      parseWorkflowRunStatusSnapshot({
        ...settingReplaceSnapshot,
        proposalView: {
          ...settingReplaceSnapshot.proposalView,
          operation: {
            ...settingReplaceSnapshot.proposalView.operation,
            relativePath: "characters/旧都.md",
          },
        },
      }),
    ).toThrow();
    expect(() =>
      parseWorkflowRunStatusSnapshot({
        ...proposalSnapshot,
        proposalView: { ...proposalSnapshot.proposalView, relativePath: "../escape.md" },
      }),
    ).toThrow();
    expect(() =>
      parseWorkflowRunStatusSnapshot({
        ...proposalSnapshot,
        proposalView: {
          ...proposalSnapshot.proposalView,
          operation: { ...proposalSnapshot.proposalView.operation, relativePath: "../escape.md" },
        },
      }),
    ).toThrow();
    expect(() =>
      parseWorkflowRunStatusSnapshot({
        ...proposalSnapshot,
        proposalView: {
          ...proposalSnapshot.proposalView,
          operation: {
            ...proposalSnapshot.proposalView.operation,
            relativePath: "characters/A\u0000.md",
          },
        },
      }),
    ).toThrow();
    expect(() =>
      parseWorkflowRunStatusSnapshot({
        ...proposalSnapshot,
        proposalView: { ...proposalSnapshot.proposalView, fullContent: `${fullContent}forged` },
      }),
    ).toThrow();
  });

  it("rejects invalid briefs before invoking the desktop runtime", async () => {
    const invoke = vi.fn();
    const sessionId = "30000000-0000-4000-8000-000000000003";
    await expect(requestWorkflowRunPrepare(sessionId, "   ", invoke)).rejects.toEqual({
      code: "workflow.input_invalid",
      messageKey: "errors.workflowInputInvalid",
    });
    await expect(
      requestWorkflowRunPrepare(sessionId, "a".repeat(4_001), invoke),
    ).rejects.toMatchObject({ code: "workflow.input_invalid" });
    await expect(
      requestWorkflowRunPrepare(sessionId, "人物\u0000秘密", invoke),
    ).rejects.toMatchObject({
      code: "workflow.input_invalid",
    });
    expect(invoke).not.toHaveBeenCalled();
  });

  it("keeps material sources, replace targets and the fixed MaterialBrief contract separate", async () => {
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const source = {
      relativePath: "characters/林遥.md",
      sourceKind: "characters",
      displayName: "林遥",
      entityId: null,
      entitySchemaVersion: null,
      settingId: null,
      settingType: null,
      materialId: null,
      materialType: null,
      contentHash: "a".repeat(64),
      byteCount: 320,
    } as const;
    const target = {
      relativePath: "materials/旧钥匙.md",
      sourceKind: "material",
      displayName: "旧钥匙",
      entityId: null,
      entitySchemaVersion: null,
      settingId: null,
      settingType: null,
      materialId: "material_80000000-0000-4000-8000-000000000008",
      materialType: "object",
      contentHash: "b".repeat(64),
      byteCount: 480,
    } as const;
    const catalog = {
      schemaVersion: 1,
      workflowId: "novelgainker.material-create",
      sources: [source],
      replaceTargets: [target],
    } as const;
    const materialSnapshot = {
      ...workflowRunFixture,
      run: {
        ...workflowRunFixture.run,
        workflowId: "novelgainker.material-create",
        workflowVersion: 1,
      },
      contextManifest: {
        ...workflowRunFixture.contextManifest,
        workflowId: "novelgainker.material-create",
        workflowVersion: 1,
      },
    } as const;
    const invoke = vi.fn().mockResolvedValueOnce(catalog).mockResolvedValueOnce(materialSnapshot);

    expect(parseWorkflowSourceCatalog(catalog)).toEqual(catalog);
    expect(() =>
      parseWorkflowSourceCatalog({
        ...catalog,
        sources: [
          {
            ...source,
            relativePath: "materials/000.story-ledger.md",
            sourceKind: "material",
          },
        ],
      }),
    ).toThrow();
    expect(() =>
      parseWorkflowSourceCatalog({
        ...catalog,
        replaceTargets: [
          {
            ...target,
            relativePath: "materials/000.story-ledger.md",
          },
        ],
      }),
    ).toThrow();
    expect(() =>
      parseWorkflowSourceCatalog({
        ...catalog,
        sources: [{ ...source, body: "PRIVATE_SOURCE_CANARY" }],
      }),
    ).toThrow();

    await expect(requestMaterialWorkflowSourceCatalog(sessionId, invoke)).resolves.toEqual(catalog);
    await expect(
      requestMaterialWorkflowRunPrepare(
        sessionId,
        {
          schemaVersion: 1,
          displayName: " 退潮铜钥匙 ",
          materialType: "clue",
          seed: " 退潮后才显出字迹 ",
          intendedUse: " 第二幕调查线索 ",
          constraints: " 遵守潮汐规则 ",
        },
        ["characters/林遥.md"],
        "materials/旧钥匙.md",
        invoke,
      ),
    ).resolves.toEqual(materialSnapshot);

    expect(invoke.mock.calls).toEqual([
      [WORKFLOW_SOURCE_CATALOG_COMMAND, { sessionId, workflowId: "novelgainker.material-create" }],
      [
        WORKFLOW_RUN_PREPARE_COMMAND,
        {
          sessionId,
          workflowId: "novelgainker.material-create",
          input: {
            schemaVersion: 1,
            displayName: "退潮铜钥匙",
            materialType: "clue",
            seed: "退潮后才显出字迹",
            intendedUse: "第二幕调查线索",
            constraints: "遵守潮汐规则",
          },
          selectedSourcePaths: ["characters/林遥.md"],
          replaceTargetPath: "materials/旧钥匙.md",
        },
      ],
    ]);
  });
  it("keeps Plot sources bounded, the replace target fixed, and sends only PlotBriefV1", async () => {
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const source = (relativePath: string, sourceKind: "world" | "characters" | "materials") => ({
      relativePath,
      sourceKind,
      displayName: relativePath.split("/").slice(-1)[0]?.replace(/\.md$/u, "") ?? relativePath,
      entityId: null,
      entitySchemaVersion: null,
      settingId: null,
      settingType: null,
      materialId: null,
      materialType: null,
      contentHash: "d".repeat(64),
      byteCount: 256,
    });
    const target = {
      relativePath: "plot/main.md",
      sourceKind: "plot",
      displayName: "旧主剧情",
      entityId: null,
      entitySchemaVersion: null,
      settingId: null,
      settingType: null,
      materialId: null,
      materialType: null,
      contentHash: "e".repeat(64),
      byteCount: 512,
    } as const;
    const catalog = {
      schemaVersion: 1,
      workflowId: "novelgainker.plot-create",
      sources: [
        source("characters/林遥.md", "characters"),
        source("materials/铜钥匙.md", "materials"),
        source("world/潮汐.md", "world"),
      ],
      replaceTargets: [target],
    } as const;
    expect(parseWorkflowSourceCatalog(catalog)).toEqual(catalog);

    const invoke = vi.fn().mockResolvedValueOnce(catalog).mockResolvedValueOnce(workflowRunFixture);
    await requestPlotWorkflowSourceCatalog(sessionId, invoke);
    await requestPlotWorkflowRunPrepare(
      sessionId,
      {
        schemaVersion: 1,
        title: " 退潮之塔 ",
        genre: " 悬疑奇幻 ",
        coreQuestion: " 林遥是否愿意用记忆换取真相？ ",
        desiredEnding: " 林遥公开真相并承担代价。 ",
        mustInclude: [" 铜钥匙必须在第二幕改变调查方向 "],
        forbiddenDirections: [" 不使用巧合解决最终冲突 "],
      },
      ["world/潮汐.md", "characters/林遥.md", "materials/铜钥匙.md"],
      "plot/main.md",
      invoke,
    );
    expect(invoke.mock.calls).toEqual([
      [WORKFLOW_SOURCE_CATALOG_COMMAND, { sessionId, workflowId: "novelgainker.plot-create" }],
      [
        WORKFLOW_RUN_PREPARE_COMMAND,
        {
          sessionId,
          workflowId: "novelgainker.plot-create",
          input: {
            schemaVersion: 1,
            title: "退潮之塔",
            genre: "悬疑奇幻",
            coreQuestion: "林遥是否愿意用记忆换取真相？",
            desiredEnding: "林遥公开真相并承担代价。",
            mustInclude: ["铜钥匙必须在第二幕改变调查方向"],
            forbiddenDirections: ["不使用巧合解决最终冲突"],
          },
          selectedSourcePaths: ["characters/林遥.md", "materials/铜钥匙.md", "world/潮汐.md"],
          replaceTargetPath: "plot/main.md",
        },
      ],
    ]);
    expect(JSON.stringify(catalog)).not.toMatch(/reveal|payoff|正文|prompt|response/iu);

    await expect(
      requestPlotWorkflowRunPrepare(
        sessionId,
        {
          schemaVersion: 1,
          title: "退潮之塔",
          genre: "悬疑",
          coreQuestion: "核心问题",
          desiredEnding: "期望结局",
          mustInclude: [],
          forbiddenDirections: [],
        },
        ["plot/main.md"],
        null,
        vi.fn(),
      ),
    ).rejects.toEqual({
      code: "workflow.source_not_allowed",
      messageKey: "errors.workflowSourceNotAllowed",
    });
    expect(() =>
      parseWorkflowSourceCatalog({
        ...catalog,
        replaceTargets: [target, { ...target }],
      }),
    ).toThrow();
  });
  it("parses only strict repair, proposal-ready and terminal metadata events", () => {
    const event = {
      runId: workflowRunFixture.run.runId,
      sequence: 4,
      kind: "failed",
      workflowId: "novelgainker.character-create",
      providerId: "deepseek",
      model: "deepseek-v4-pro",
      state: "failed",
      error: { code: "workflow.output_pending", messageKey: "errors.workflowOutputPending" },
    };
    expect(parseWorkflowRunEvent(event)).toEqual(event);
    const repairing = {
      ...event,
      sequence: 3,
      kind: "repairing",
      state: "generating",
      error: {
        code: "workflow.output_invalid",
        messageKey: "errors.workflowOutputInvalid",
      },
    };
    expect(parseWorkflowRunEvent(repairing)).toEqual(repairing);
    const proposalReady = {
      ...event,
      sequence: 5,
      kind: "proposalReady",
      state: "awaiting_confirmation",
      error: null,
    };
    expect(parseWorkflowRunEvent(proposalReady)).toEqual(proposalReady);
    expect(() =>
      parseWorkflowRunEvent({
        ...repairing,
        providerId: "openai",
        model: "deepseek-v4-pro",
      }),
    ).toThrow();
    expect(() => parseWorkflowRunEvent({ ...repairing, error: null })).toThrow();
    expect(() =>
      parseWorkflowRunEvent({ ...proposalReady, response: "PRIVATE_RESPONSE_CANARY" }),
    ).toThrow();
    expect(() => parseWorkflowRunEvent({ ...event, response: "hidden" })).toThrow();
    expect(() => parseWorkflowRunEvent({ ...event, state: "completed", error: null })).toThrow();
  });

  it("normalizes raw failures and malformed responses", async () => {
    await expect(
      requestWorkflowRunStatus(
        "30000000-0000-4000-8000-000000000003",
        vi.fn().mockResolvedValue({}),
      ),
    ).rejects.toEqual({
      code: "contract.invalid_response",
      messageKey: "errors.invalidRuntimeResponse",
    });
    await expect(
      requestWorkflowRunCancel(
        "30000000-0000-4000-8000-000000000003",
        vi.fn().mockRejectedValue(new Error("C:\\Private\\prompt.md")),
      ),
    ).rejects.toEqual({ code: "runtime.unknown", messageKey: "errors.unknown" });
  });
});
