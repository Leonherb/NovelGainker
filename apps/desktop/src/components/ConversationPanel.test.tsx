import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProviderRunEvent } from "../lib/tauri/provider-chat";
import type { RouterRunEvent } from "../lib/tauri/router";
import type { ProviderStateSnapshot } from "../lib/tauri/provider";
import workflowRunFixture from "../../../../contracts/ipc/workflow-run.v3.json";

const chatMocks = vi.hoisted(() => ({
  cancel: vi.fn(),
  listen: vi.fn(),
  start: vi.fn(),
}));

const routerMocks = vi.hoisted(() => ({
  cancel: vi.fn(),
  listen: vi.fn(),
  local: vi.fn(),
  start: vi.fn(),
}));
const workflowMocks = vi.hoisted(() => ({
  load: vi.fn(),
}));
const workflowRunMocks = vi.hoisted(() => ({
  abandon: vi.fn(),
  catalog: vi.fn(),
  characterCatalog: vi.fn(),
  materialCatalog: vi.fn(),
  plotCatalog: vi.fn(),
  apply: vi.fn(),
  cancel: vi.fn(),
  generate: vi.fn(),
  listen: vi.fn(),
  prepare: vi.fn(),
  prepareCharacter: vi.fn(),
  prepareSetting: vi.fn(),
  prepareMaterial: vi.fn(),
  preparePlot: vi.fn(),
  status: vi.fn(),
}));

vi.mock("../lib/tauri/router", () => ({
  listenRouterRunEvents: routerMocks.listen,
  requestLocalRouterDecision: routerMocks.local,
  requestRouterRunCancel: routerMocks.cancel,
  requestRouterRunStart: routerMocks.start,
}));
vi.mock("../lib/tauri/provider-chat", () => ({
  listenProviderRunEvents: chatMocks.listen,
  requestProviderChatCancel: chatMocks.cancel,
  requestProviderChatStart: chatMocks.start,
}));

vi.mock("../lib/tauri/workflow", () => ({
  CHARACTER_CREATE_WORKFLOW_ID: "novelgainker.character-create",
  PROJECT_CREATE_WORKFLOW_ID: "novelgainker.project-create",
  MATERIAL_CREATE_WORKFLOW_ID: "novelgainker.material-create",
  PLOT_CREATE_WORKFLOW_ID: "novelgainker.plot-create",
  SETTING_CREATE_WORKFLOW_ID: "novelgainker.setting-create",
  STORYTELLING_WORKFLOW_ID: "novelgainker.storytelling",
  requestWorkflowRegistry: workflowMocks.load,
}));

vi.mock("../lib/tauri/workflow-run", () => ({
  listenWorkflowRunEvents: workflowRunMocks.listen,
  requestCharacterWorkflowRunPrepare: workflowRunMocks.prepareCharacter,
  requestCharacterWorkflowSourceCatalog: workflowRunMocks.characterCatalog,
  requestMaterialWorkflowRunPrepare: workflowRunMocks.prepareMaterial,
  requestMaterialWorkflowSourceCatalog: workflowRunMocks.materialCatalog,
  requestPlotWorkflowRunPrepare: workflowRunMocks.preparePlot,
  requestPlotWorkflowSourceCatalog: workflowRunMocks.plotCatalog,
  requestWorkflowProposalApply: workflowRunMocks.apply,
  requestWorkflowRunAbandon: workflowRunMocks.abandon,
  requestWorkflowRunCancel: workflowRunMocks.cancel,
  requestWorkflowRunGenerate: workflowRunMocks.generate,
  requestSettingWorkflowRunPrepare: workflowRunMocks.prepareSetting,
  requestWorkflowSourceCatalog: workflowRunMocks.catalog,
  requestWorkflowRunPrepare: workflowRunMocks.prepare,
  requestWorkflowRunStatus: workflowRunMocks.status,
}));

import { ConversationPanel } from "./ConversationPanel";

const STARTED = {
  runId: "10000000-0000-4000-8000-000000000001",
  providerId: "openai" as const,
  model: "gpt-5.6-terra",
};
const CONNECTED_STATE: ProviderStateSnapshot = {
  schemaVersion: 2,
  selectedProviderId: "openai",
  providers: [
    {
      providerId: "deepseek",
      configured: false,
      connectionStatus: "unconfigured",
      presetVersion: 1,
      selected: false,
    },
    {
      providerId: "openai",
      configured: true,
      connectionStatus: "connected",
      presetVersion: 1,
      selected: true,
    },
  ],
};

describe("ConversationPanel", () => {
  const WORKFLOW_REGISTRY = {
    schemaVersion: 1 as const,
    workflows: [
      {
        id: "novelgainker.character-create" as const,
        version: 2 as const,
        displayName: "创建/完善人物",
        description: "审查显式选择的 Canon 来源，创建或完善单个人物 Markdown，并保留稳定实体引用。",
        mode: "interactive-write" as const,
        executionAvailable: true as const,
        routable: true as const,
      },
      {
        id: "novelgainker.project-create" as const,
        version: 1 as const,
        displayName: "创建项目",
        description: "离线填写项目元数据，安全审查完整脚手架后创建本地 Markdown 小说项目。",
        mode: "deterministic-write" as const,
        executionAvailable: true as const,
        routable: true as const,
      },
      {
        id: "novelgainker.setting-create" as const,
        version: 1 as const,
        displayName: "创建设定",
        description:
          "根据设定简报和用户明确选择的设定来源生成结构化设定提案，经确认后创建设定 Markdown。",
        mode: "interactive-write" as const,
        executionAvailable: true as const,
        routable: true as const,
      },
      {
        id: "novelgainker.material-create" as const,
        version: 1 as const,
        displayName: "创建素材",
        description:
          "审查显式选择的 Canon 来源，生成结构化素材提案，经确认后写入一个普通 materials Markdown。",
        mode: "interactive-write" as const,
        executionAvailable: true as const,
        routable: true as const,
      },
      {
        id: "novelgainker.plot-create" as const,
        version: 1 as const,
        displayName: "创建主剧情",
        description:
          "审查显式选择的人物、设定和普通素材，生成结构化主剧情提案，经确认后写入固定 plot/main.md。",
        mode: "interactive-write" as const,
        executionAvailable: true as const,
        routable: true as const,
      },
      {
        id: "novelgainker.storytelling" as const,
        version: 1 as const,
        displayName: "小说续写与章节创作",
        description:
          "在本地审查章节上下文来源、预算、截断与哈希；当前只开放 Context Manifest 构建。",
        mode: "interactive-write" as const,
        executionAvailable: true as const,
        routable: false as const,
      },
    ] as const,
  };

  let onEvent: ((event: ProviderRunEvent) => void) | undefined;
  let onRouterEvent: ((event: RouterRunEvent) => void) | undefined;
  let onWorkflowEvent: ((event: unknown) => void) | undefined;

  beforeEach(() => {
    vi.clearAllMocks();
    onEvent = undefined;
    onRouterEvent = undefined;
    onWorkflowEvent = undefined;
    workflowRunMocks.listen.mockImplementation(async (handler) => {
      onWorkflowEvent = handler;
      return vi.fn();
    });
    chatMocks.listen.mockImplementation(async (handler) => {
      onEvent = handler;
      return vi.fn();
    });
    routerMocks.listen.mockImplementation(async (handler) => {
      onRouterEvent = handler;
      return vi.fn();
    });
    workflowRunMocks.status.mockResolvedValue({
      schemaVersion: 3,
      available: true,
      hasActiveRun: false,
      run: null,
      contextManifest: null,
      proposalView: null,
    });
    workflowRunMocks.catalog.mockResolvedValue({
      schemaVersion: 1,
      workflowId: "novelgainker.setting-create",
      sources: [],
      replaceTargets: [],
    });
    workflowRunMocks.characterCatalog.mockResolvedValue({
      schemaVersion: 1,
      workflowId: "novelgainker.character-create",
      sources: [],
      replaceTargets: [],
    });
    workflowRunMocks.materialCatalog.mockResolvedValue({
      schemaVersion: 1,
      workflowId: "novelgainker.material-create",
      sources: [],
      replaceTargets: [],
    });
    workflowRunMocks.plotCatalog.mockResolvedValue({
      schemaVersion: 1,
      workflowId: "novelgainker.plot-create",
      sources: [],
      replaceTargets: [],
    });
    workflowRunMocks.cancel.mockResolvedValue({
      schemaVersion: 3,
      available: true,
      hasActiveRun: false,
      run: null,
      contextManifest: null,
      proposalView: null,
    });
    chatMocks.start.mockResolvedValue(STARTED);
    chatMocks.cancel.mockResolvedValue({ cancelRequested: true });
    routerMocks.start.mockResolvedValue(STARTED);
    routerMocks.cancel.mockResolvedValue({ cancelRequested: true });
    routerMocks.local.mockResolvedValue({
      schemaVersion: 1,
      outcome: "needsProvider",
      workflowId: null,
      entryMode: null,
    });
  });

  workflowMocks.load.mockResolvedValue(WORKFLOW_REGISTRY);
  it("shows five routable workflows plus non-routable Storytelling and routes offline project creation", async () => {
    const user = userEvent.setup();
    const onProjectCreateRequested = vi.fn();
    render(
      <ConversationPanel
        desktopRuntime
        onProjectCreateRequested={onProjectCreateRequested}
        providerState={CONNECTED_STATE}
      />,
    );

    const characterEntry = await screen.findByRole("button", { name: "打开创建/完善人物" });
    const projectEntry = screen.getByRole("button", { name: "打开创建项目" });
    const settingEntry = screen.getByRole("button", { name: "打开创建设定" });
    const materialEntry = screen.getByRole("button", { name: "打开创建素材" });
    const plotEntry = screen.getByRole("button", { name: "打开创建主剧情" });
    const storyEntry = screen.getByRole("button", { name: "打开小说续写与章节创作" });
    expect(characterEntry).toBeDisabled();
    expect(settingEntry).toBeDisabled();
    expect(materialEntry).toBeDisabled();
    expect(plotEntry).toBeDisabled();
    expect(storyEntry).toBeDisabled();
    expect(projectEntry).toBeEnabled();
    expect(screen.getByText("NOVELGAINKER WORKFLOW PACK")).toBeInTheDocument();
    expect(screen.getByText("创建主剧情")).toBeInTheDocument();
    expect(screen.getByText("小说续写与章节创作")).toBeInTheDocument();
    await user.click(projectEntry);
    expect(onProjectCreateRequested).toHaveBeenCalledOnce();
    expect(workflowMocks.load).toHaveBeenCalledOnce();
    expect(chatMocks.start).not.toHaveBeenCalled();
  });

  it("collects a strict setting brief and an explicit same-path replace target", async () => {
    const user = userEvent.setup();
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const catalog = {
      schemaVersion: 1,
      workflowId: "novelgainker.setting-create",
      sources: [
        {
          relativePath: "world/旧都.md",
          settingId: "setting_10000000-0000-4000-8000-000000000001",
          displayName: "旧都",
          settingType: "location",
          contentHash: "c".repeat(64),
          byteCount: 256,
        },
      ],
    };
    const prepared = {
      ...workflowRunFixture,
      run: {
        ...workflowRunFixture.run,
        workflowId: "novelgainker.setting-create",
      },
      contextManifest: {
        ...workflowRunFixture.contextManifest,
        workflowId: "novelgainker.setting-create",
      },
    };
    workflowRunMocks.catalog.mockResolvedValueOnce(catalog);
    workflowRunMocks.prepareSetting.mockResolvedValueOnce(prepared);

    render(
      <ConversationPanel
        desktopRuntime
        projectSessionId={sessionId}
        providerState={CONNECTED_STATE}
      />,
    );

    const open = await screen.findByRole("button", { name: "打开创建设定" });
    await waitFor(() => expect(open).toBeEnabled());
    await user.click(open);
    await waitFor(() => expect(workflowRunMocks.catalog).toHaveBeenCalledWith(sessionId));

    await user.selectOptions(screen.getByLabelText("写入方式"), "replace");
    await user.selectOptions(screen.getByLabelText("要完善的已有设定"), "world/旧都.md");
    const targetSource = screen.getByRole("checkbox", { name: /旧都/u });
    expect(targetSource).toBeChecked();
    expect(targetSource).toBeDisabled();
    await user.clear(screen.getByLabelText("设定名称"));
    await user.type(screen.getByLabelText("设定名称"), "焕新旧都");
    await user.selectOptions(screen.getByLabelText("设定类型"), "location");
    await user.type(screen.getByLabelText("创作目标"), "建立压迫感");
    await user.type(
      screen.getByLabelText("必须包含（每行一项，1～12 项）"),
      "终年浓雾{enter}钟声决定宵禁",
    );
    expect(screen.getByText("已选择 1 / 6")).toBeInTheDocument();
    expect(workflowRunMocks.prepareSetting).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "准备发送清单" }));
    await waitFor(() =>
      expect(workflowRunMocks.prepareSetting).toHaveBeenCalledWith(
        sessionId,
        {
          schemaVersion: 1,
          displayName: "焕新旧都",
          settingType: "location",
          objective: "建立压迫感",
          mustInclude: ["终年浓雾", "钟声决定宵禁"],
          constraints: null,
        },
        ["world/旧都.md"],
        "world/旧都.md",
      ),
    );
    expect(chatMocks.start).not.toHaveBeenCalled();
    expect(await screen.findByText("发送上下文清单")).toBeInTheDocument();
  });

  it("collects a MaterialBrief with separate Canon sources and replace target", async () => {
    const user = userEvent.setup();
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const source = {
      relativePath: "characters/林遥.md",
      sourceKind: "characters",
      displayName: "林遥",
      settingId: null,
      settingType: null,
      materialId: null,
      materialType: null,
      contentHash: "a".repeat(64),
      byteCount: 320,
    };
    const target = {
      relativePath: "materials/旧钥匙.md",
      sourceKind: "material",
      displayName: "旧钥匙",
      settingId: null,
      settingType: null,
      materialId: "material_80000000-0000-4000-8000-000000000008",
      materialType: "object",
      contentHash: "b".repeat(64),
      byteCount: 480,
    };
    workflowRunMocks.materialCatalog.mockResolvedValueOnce({
      schemaVersion: 1,
      workflowId: "novelgainker.material-create",
      sources: [source],
      replaceTargets: [target],
    });
    const prepared = {
      ...workflowRunFixture,
      run: {
        ...workflowRunFixture.run,
        workflowId: "novelgainker.material-create",
      },
      contextManifest: {
        ...workflowRunFixture.contextManifest,
        workflowId: "novelgainker.material-create",
      },
    };
    workflowRunMocks.prepareMaterial.mockResolvedValueOnce(prepared);

    render(
      <ConversationPanel
        desktopRuntime
        projectSessionId={sessionId}
        providerState={CONNECTED_STATE}
      />,
    );

    const open = await screen.findByRole("button", { name: "打开创建素材" });
    await waitFor(() => expect(open).toBeEnabled());
    await user.click(open);
    await waitFor(() => expect(workflowRunMocks.materialCatalog).toHaveBeenCalledWith(sessionId));
    expect(
      screen.getByText(/materials\/（含 000\.story-ledger\.md）、章节、聊天、编辑器和搜索结果/u),
    ).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("成为 Canon 的方式"), "replace");
    await user.selectOptions(screen.getByLabelText("要替换的已有素材"), "materials/旧钥匙.md");
    expect(screen.getByRole("checkbox", { name: /林遥/u })).not.toBeChecked();
    await user.clear(screen.getByLabelText("素材名称"));
    await user.type(screen.getByLabelText("素材名称"), "退潮铜钥匙");
    await user.selectOptions(screen.getByLabelText("素材类型"), "clue");
    await user.type(screen.getByLabelText("素材种子"), "退潮后才显出字迹");
    await user.type(screen.getByLabelText("预期用途"), "作为第二幕调查线索");
    await user.click(screen.getByRole("checkbox", { name: /林遥/u }));
    expect(screen.getByText("已选择 1 / 6")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "准备发送清单" }));
    await waitFor(() =>
      expect(workflowRunMocks.prepareMaterial).toHaveBeenCalledWith(
        sessionId,
        {
          schemaVersion: 1,
          displayName: "退潮铜钥匙",
          materialType: "clue",
          seed: "退潮后才显出字迹",
          intendedUse: "作为第二幕调查线索",
          constraints: null,
        },
        ["characters/林遥.md"],
        "materials/旧钥匙.md",
      ),
    );
    expect(chatMocks.start).not.toHaveBeenCalled();
  });

  it("presents material review as continue, confirm Canon, or abandon", async () => {
    const user = userEvent.setup();
    const onCanonApplied = vi.fn();
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const fullContent = "---\ntitle: 退潮铜钥匙\nwritingLanguage: zh-CN\n---\n\n# 退潮铜钥匙\n";
    const proposalSnapshot = {
      schemaVersion: 3,
      available: true,
      hasActiveRun: true,
      run: {
        ...workflowRunFixture.run,
        workflowId: "novelgainker.material-create",
        state: "awaiting_confirmation",
        sequence: 6,
        canCancel: false,
        canAbandon: true,
      },
      contextManifest: null,
      proposalView: {
        schemaVersion: 1,
        proposalId: "30000000-0000-4000-8000-000000000003",
        proposalHash: "f".repeat(64),
        runId: workflowRunFixture.run.runId,
        workflowId: "novelgainker.material-create",
        workflowVersion: 1,
        packHash: "a".repeat(64),
        providerId: "deepseek",
        presetVersion: 1,
        model: "deepseek-v4-pro",
        manifestHash: "e".repeat(64),
        operation: {
          kind: "create",
          relativePath: "materials/退潮铜钥匙.md",
          expectedSourceHash: "absent",
          proposedContentHash: "d".repeat(64),
          proposedByteCount: new TextEncoder().encode(fullContent).byteLength,
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
    };
    workflowRunMocks.status.mockResolvedValue(proposalSnapshot);
    workflowRunMocks.apply.mockResolvedValue({
      ...proposalSnapshot,
      hasActiveRun: false,
      run: {
        ...proposalSnapshot.run,
        state: "completed",
        sequence: 9,
        canAbandon: false,
      },
      proposalView: null,
    });

    render(
      <ConversationPanel
        desktopRuntime
        projectSessionId={sessionId}
        providerState={CONNECTED_STATE}
        onCanonApplied={onCanonApplied}
      />,
    );

    expect(await screen.findByText("素材提案 · 尚未成为 Canon")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "确认成为 Canon" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "放弃素材" }));
    expect(screen.getByText(/清除内存中的素材简报/u)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "保留运行" }));
    expect(workflowRunMocks.abandon).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "确认成为 Canon" }));
    expect(screen.getByText("最后确认：让此素材成为 Canon")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "继续审查" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "继续审查" }));
    expect(workflowRunMocks.apply).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认成为 Canon" }));
    await user.click(screen.getByRole("button", { name: "确认成为 Canon" }));
    await waitFor(() =>
      expect(workflowRunMocks.apply).toHaveBeenCalledWith(
        workflowRunFixture.run.runId,
        "f".repeat(64),
        true,
      ),
    );
    expect(onCanonApplied).toHaveBeenCalledWith("materials/退潮铜钥匙.md");
  });
  it("collects a bounded PlotBrief, explicit sources, and the fixed main plot replace target", async () => {
    const user = userEvent.setup();
    const sessionId = "30000000-0000-4000-8000-000000000003";
    workflowRunMocks.plotCatalog.mockResolvedValueOnce({
      schemaVersion: 1,
      workflowId: "novelgainker.plot-create",
      sources: [
        {
          relativePath: "characters/林遥.md",
          sourceKind: "characters",
          displayName: "林遥",
          settingId: null,
          settingType: null,
          materialId: null,
          materialType: null,
          contentHash: "c".repeat(64),
          byteCount: 256,
        },
        {
          relativePath: "materials/铜钥匙.md",
          sourceKind: "materials",
          displayName: "铜钥匙",
          settingId: null,
          settingType: null,
          materialId: null,
          materialType: null,
          contentHash: "d".repeat(64),
          byteCount: 256,
        },
      ],
      replaceTargets: [
        {
          relativePath: "plot/main.md",
          sourceKind: "plot",
          displayName: "旧主剧情",
          settingId: null,
          settingType: null,
          materialId: null,
          materialType: null,
          contentHash: "e".repeat(64),
          byteCount: 512,
        },
      ],
    });
    workflowRunMocks.preparePlot.mockResolvedValueOnce({
      ...workflowRunFixture,
      run: { ...workflowRunFixture.run, workflowId: "novelgainker.plot-create" },
      contextManifest: {
        ...workflowRunFixture.contextManifest,
        workflowId: "novelgainker.plot-create",
      },
    });

    render(
      <ConversationPanel
        desktopRuntime
        projectSessionId={sessionId}
        providerState={CONNECTED_STATE}
      />,
    );
    const open = await screen.findByRole("button", { name: "打开创建主剧情" });
    await waitFor(() => expect(open).toBeEnabled());
    await user.click(open);
    await waitFor(() => expect(workflowRunMocks.plotCatalog).toHaveBeenCalledWith(sessionId));
    expect(screen.getByLabelText("主剧情操作")).toHaveValue("replace");
    expect(screen.getByText(/旧主剧情 ·\s*plot\/main\.md/u)).toBeInTheDocument();

    await user.clear(screen.getByLabelText("主剧情标题"));
    await user.type(screen.getByLabelText("主剧情标题"), "退潮之塔");
    await user.type(screen.getByLabelText("类型"), "悬疑奇幻");
    await user.type(screen.getByLabelText("核心故事问题"), "林遥是否愿意用记忆换取真相？");
    await user.type(
      screen.getByLabelText("期望结局"),
      "林遥公开潮汐塔的代价并承担失去记忆的结果。",
    );
    await user.type(screen.getByLabelText("必须包含（每行一项，最多 8 项）"), "铜钥匙必须被回收");
    await user.type(screen.getByLabelText("禁止方向（每行一项，最多 8 项）"), "不由巧合解决冲突");
    await user.click(screen.getByRole("checkbox", { name: /林遥/u }));
    await user.click(screen.getByRole("checkbox", { name: /铜钥匙/u }));
    expect(screen.getByText("已选择 2 / 8")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "准备发送清单" }));

    await waitFor(() =>
      expect(workflowRunMocks.preparePlot).toHaveBeenCalledWith(
        sessionId,
        {
          schemaVersion: 1,
          title: "退潮之塔",
          genre: "悬疑奇幻",
          coreQuestion: "林遥是否愿意用记忆换取真相？",
          desiredEnding: "林遥公开潮汐塔的代价并承担失去记忆的结果。",
          mustInclude: ["铜钥匙必须被回收"],
          forbiddenDirections: ["不由巧合解决冲突"],
        },
        ["characters/林遥.md", "materials/铜钥匙.md"],
        "plot/main.md",
      ),
    );
    expect(chatMocks.start).not.toHaveBeenCalled();
  });
  it("prepares offline, renders a plain-text Manifest and sends only after explicit confirmation", async () => {
    const user = userEvent.setup();
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const hostile = '<script>alert("manifest")</script><a href="https://example.com">点我</a>';
    const prepared = {
      ...workflowRunFixture,
      run: {
        ...workflowRunFixture.run,
        providerId: "openai",
        model: "gpt-5.6-terra",
      },
      contextManifest: {
        ...workflowRunFixture.contextManifest,
        providerId: "openai",
        model: "gpt-5.6-terra",
        sources: [
          {
            ...workflowRunFixture.contextManifest.sources[0],
            boundedPreview: hostile,
          },
          workflowRunFixture.contextManifest.sources[1],
        ],
      },
    };
    workflowRunMocks.prepareCharacter.mockResolvedValue(prepared);
    workflowRunMocks.generate.mockResolvedValue({
      ...prepared,
      run: {
        ...prepared.run,
        state: "generating",
        sequence: 4,
        canCancel: true,
      },
    });

    render(
      <ConversationPanel
        desktopRuntime
        projectSessionId={sessionId}
        providerState={CONNECTED_STATE}
      />,
    );

    const open = await screen.findByRole("button", { name: "打开创建/完善人物" });
    await waitFor(() => expect(open).toBeEnabled());
    await user.click(open);
    await user.type(screen.getByLabelText("人物简报"), "冷静的档案员，隐藏着一个秘密");
    expect(workflowRunMocks.prepareCharacter).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "准备发送清单" }));
    await waitFor(() =>
      expect(workflowRunMocks.prepareCharacter).toHaveBeenCalledWith(
        sessionId,
        {
          schemaVersion: 2,
          mode: "create",
          brief: "冷静的档案员，隐藏着一个秘密",
        },
        [],
        null,
      ),
    );

    expect(await screen.findByText("发送上下文清单")).toBeInTheDocument();
    expect(screen.getByText(hostile)).toBeInTheDocument();
    expect(document.querySelector("script")).toBeNull();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(screen.getByText(/可能产生少量 API 用量/u)).toBeInTheDocument();
    expect(workflowRunMocks.generate).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "确认发送并生成" }));
    await waitFor(() =>
      expect(workflowRunMocks.generate).toHaveBeenCalledWith(
        workflowRunFixture.run.runId,
        workflowRunFixture.contextManifest.manifestHash,
        true,
      ),
    );

    workflowRunMocks.status.mockResolvedValue({
      schemaVersion: 3,
      available: true,
      hasActiveRun: true,
      run: {
        ...prepared.run,
        state: "generating",
        sequence: 4,
        canCancel: true,
        canAbandon: true,
        recoveryRequired: false,
      },
      contextManifest: null,
      proposalView: null,
    });
    act(() => {
      onWorkflowEvent?.({
        runId: workflowRunFixture.run.runId,
        sequence: 4,
        kind: "repairing",
        workflowId: "novelgainker.character-create",
        providerId: "openai",
        model: "gpt-5.6-terra",
        state: "generating",
        error: {
          code: "workflow.output_invalid",
          messageKey: "errors.workflowOutputInvalid",
        },
      });
    });
    expect(await screen.findByText("正在修复结构化创作提案")).toBeInTheDocument();
    expect(screen.getByText(/额外少量 API 用量/u)).toBeInTheDocument();

    workflowRunMocks.status.mockResolvedValue({
      schemaVersion: 3,
      available: true,
      hasActiveRun: false,
      run: {
        ...prepared.run,
        state: "failed",
        sequence: 5,
        canCancel: false,
        canAbandon: false,
        recoveryRequired: false,
      },
      contextManifest: null,
      proposalView: null,
    });
    act(() => {
      onWorkflowEvent?.({
        runId: workflowRunFixture.run.runId,
        sequence: 5,
        kind: "failed",
        workflowId: "novelgainker.character-create",
        providerId: "openai",
        model: "gpt-5.6-terra",
        state: "failed",
        error: {
          code: "workflow.repair_failed",
          messageKey: "errors.workflowRepairFailed",
        },
      });
    });
    expect(await screen.findByText("创作提案修复失败")).toBeInTheDocument();
  });

  it("makes legacy enhancement explicit, auto-selects the target and submits CharacterBriefV2", async () => {
    const user = userEvent.setup();
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const legacy = {
      relativePath: "characters/旧人物.md",
      sourceKind: "character",
      displayName: "旧人物",
      entityId: null,
      entitySchemaVersion: 1,
      settingId: null,
      settingType: null,
      materialId: null,
      materialType: null,
      contentHash: "c".repeat(64),
      byteCount: 256,
    };
    workflowRunMocks.characterCatalog.mockResolvedValueOnce({
      schemaVersion: 1,
      workflowId: "novelgainker.character-create",
      sources: [legacy],
      replaceTargets: [legacy],
    });
    workflowRunMocks.prepareCharacter.mockResolvedValueOnce(workflowRunFixture);

    render(
      <ConversationPanel
        desktopRuntime
        projectSessionId={sessionId}
        providerState={CONNECTED_STATE}
      />,
    );

    const open = await screen.findByRole("button", { name: "打开创建/完善人物" });
    await waitFor(() => expect(open).toBeEnabled());
    await user.click(open);
    await user.selectOptions(screen.getByLabelText("人物操作"), "enhance");
    await user.selectOptions(screen.getByLabelText("要完善的人物"), "characters/旧人物.md");
    expect(
      screen.getByText("该人物是 v1；只有确认写入后才会显式升级为 v2，并由本地生成新的稳定 ID。"),
    ).toBeInTheDocument();
    const target = screen.getByRole("checkbox", { name: /旧人物/u });
    expect(target).toBeChecked();
    expect(target).toBeDisabled();

    await user.type(screen.getByLabelText("人物简报"), "补全动机、秘密和剧情关系");
    await user.click(screen.getByRole("button", { name: "准备发送清单" }));
    await waitFor(() =>
      expect(workflowRunMocks.prepareCharacter).toHaveBeenCalledWith(
        sessionId,
        {
          schemaVersion: 2,
          mode: "enhance",
          brief: "补全动机、秘密和剧情关系",
        },
        ["characters/旧人物.md"],
        "characters/旧人物.md",
      ),
    );
  });

  it("renders the full Proposal and line Diff, then applies only after independent confirmation", async () => {
    const user = userEvent.setup();
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const onCanonApplied = vi.fn();
    const fullContent = "---\ntitle: A\nwritingLanguage: zh-CN\n---\n\n# A\n";
    const proposalSnapshot = {
      schemaVersion: 3,
      available: true,
      hasActiveRun: true,
      run: {
        ...workflowRunFixture.run,
        state: "awaiting_confirmation",
        sequence: 6,
        canCancel: false,
        canAbandon: true,
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
          kind: "replace",
          relativePath: "characters/A.md",
          expectedSourceHash: "c".repeat(64),
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
        lineDiff: [
          { kind: "remove", oldLine: 6, newLine: null, content: "# 旧 A" },
          { kind: "add", oldLine: null, newLine: 6, content: "# A" },
        ],
      },
    };
    workflowRunMocks.status.mockResolvedValue(proposalSnapshot);
    workflowRunMocks.apply.mockResolvedValue({
      schemaVersion: 3,
      available: true,
      hasActiveRun: false,
      run: {
        ...proposalSnapshot.run,
        state: "completed",
        sequence: 9,
        canAbandon: false,
      },
      contextManifest: null,
      proposalView: null,
    });

    render(
      <ConversationPanel
        desktopRuntime
        onCanonApplied={onCanonApplied}
        projectSessionId={sessionId}
        providerState={CONNECTED_STATE}
      />,
    );

    expect(await screen.findByText("Canon 写入提案")).toBeInTheDocument();
    expect(screen.getByText("characters/A.md")).toBeInTheDocument();
    expect(screen.getByText("逐行 Diff")).toBeInTheDocument();
    expect(document.querySelector(".proposal-preview pre")).toHaveTextContent("# A");
    expect(document.querySelector('[data-kind="remove"] code')).toHaveTextContent("−# 旧 A");
    expect(document.querySelector('[data-kind="add"] code')).toHaveTextContent("+# A");
    expect(workflowRunMocks.apply).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "我已审查，进入写入确认" }));
    expect(screen.getByText("最后确认写入 Canon")).toBeInTheDocument();
    expect(workflowRunMocks.apply).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认写入此版本" }));
    await waitFor(() =>
      expect(workflowRunMocks.apply).toHaveBeenCalledWith(
        workflowRunFixture.run.runId,
        "f".repeat(64),
        true,
      ),
    );
    expect(onCanonApplied).toHaveBeenCalledWith("characters/A.md");
  });

  it("routes exact Chinese commands locally with zero Provider requests", async () => {
    const user = userEvent.setup();
    const sessionId = "30000000-0000-4000-8000-000000000003";
    routerMocks.local.mockResolvedValue({
      schemaVersion: 1,
      outcome: "direct",
      workflowId: "novelgainker.character-create",
      entryMode: "character-enhance",
    });
    render(
      <ConversationPanel
        desktopRuntime
        projectSessionId={sessionId}
        providerState={CONNECTED_STATE}
      />,
    );
    await screen.findByText("可用工作流");
    await user.type(screen.getByLabelText("对话输入"), "完善人物");
    await user.click(screen.getByRole("button", { name: "识别工作流" }));
    await waitFor(() => expect(routerMocks.local).toHaveBeenCalledWith("完善人物"));
    expect(routerMocks.start).not.toHaveBeenCalled();
    expect(chatMocks.start).not.toHaveBeenCalled();
    expect(await screen.findByLabelText("人物操作")).toHaveValue("enhance");
    expect(screen.getByLabelText("对话输入")).toHaveValue("");
  });

  it("asks before ambiguous Provider classification and stops medium results at the selector", async () => {
    const user = userEvent.setup();
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const canary = "ROUTER_PRIVATE_CANARY_7D8F";
    render(
      <ConversationPanel
        desktopRuntime
        projectSessionId={sessionId}
        providerState={CONNECTED_STATE}
      />,
    );
    await screen.findByText("可用工作流");
    await user.type(screen.getByLabelText("对话输入"), `我想整理一个故事素材 ${canary}`);
    await user.click(screen.getByRole("button", { name: "识别工作流" }));
    expect(await screen.findByText("确认使用 AI 识别")).toBeInTheDocument();
    expect(routerMocks.start).not.toHaveBeenCalled();
    expect(document.body.textContent).not.toContain(canary);

    await user.click(screen.getByRole("button", { name: "确认并识别" }));
    await waitFor(() =>
      expect(routerMocks.start).toHaveBeenCalledWith("openai", `我想整理一个故事素材 ${canary}`),
    );
    act(() => {
      onRouterEvent?.({
        ...STARTED,
        kind: "completed",
        decision: {
          schemaVersion: 1,
          workflowId: "novelgainker.material-create",
          confidence: "medium",
        },
        usage: { inputTokens: 15, outputTokens: 3, totalTokens: 18 },
      });
    });
    expect(await screen.findByText(/需要确认/u)).toBeInTheDocument();
    expect(screen.queryByLabelText("创建或替换素材简报")).not.toBeInTheDocument();
    expect(document.body.textContent).not.toContain(canary);
    await user.click(screen.getByRole("button", { name: "创建素材" }));
    expect(await screen.findByLabelText("创建或替换素材简报")).toBeInTheDocument();
    expect(workflowRunMocks.prepareMaterial).not.toHaveBeenCalled();
  });

  it("opens only the suggested form for a high result and remains cancellable while running", async () => {
    const user = userEvent.setup();
    const sessionId = "30000000-0000-4000-8000-000000000003";
    render(
      <ConversationPanel
        desktopRuntime
        projectSessionId={sessionId}
        providerState={CONNECTED_STATE}
      />,
    );
    await screen.findByText("可用工作流");
    await user.type(screen.getByLabelText("对话输入"), "帮我梳理故事主线");
    await user.click(screen.getByRole("button", { name: "识别工作流" }));
    await user.click(await screen.findByRole("button", { name: "确认并识别" }));
    expect(await screen.findByText("正在识别工作流")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "取消识别" }));
    expect(routerMocks.cancel).toHaveBeenCalledWith(STARTED.runId);
    act(() => {
      onRouterEvent?.({
        ...STARTED,
        kind: "completed",
        decision: {
          schemaVersion: 1,
          workflowId: "novelgainker.plot-create",
          confidence: "high",
        },
        usage: null,
      });
    });
    expect(await screen.findByLabelText("创建或修订主剧情简报")).toBeInTheDocument();
    expect(workflowRunMocks.preparePlot).not.toHaveBeenCalled();
  });

  it("falls back to the fixed selector offline without a key or Router request", async () => {
    const user = userEvent.setup();
    const unconfigured: ProviderStateSnapshot = {
      ...CONNECTED_STATE,
      providers: CONNECTED_STATE.providers.map((provider) =>
        provider.providerId === "openai"
          ? { ...provider, configured: false, connectionStatus: "unconfigured" }
          : provider,
      ),
    };
    render(<ConversationPanel desktopRuntime providerState={unconfigured} />);
    await screen.findByText("可用工作流");
    await user.type(screen.getByLabelText("对话输入"), "也许我要做点人物资料");
    await user.click(screen.getByRole("button", { name: "识别工作流" }));
    expect(await screen.findByText("请选择下一步")).toBeInTheDocument();
    expect(routerMocks.start).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "普通 AI 对话" })).toBeDisabled();
  });
  it("sends memory messages only and renders hostile markup as plain text", async () => {
    const user = userEvent.setup();
    render(<ConversationPanel desktopRuntime providerState={CONNECTED_STATE} />);
    await waitFor(() => expect(chatMocks.listen).toHaveBeenCalledOnce());

    await user.type(screen.getByLabelText("对话输入"), "写一个安全开场");
    await user.click(screen.getByRole("button", { name: "发送" }));
    await waitFor(() =>
      expect(chatMocks.start).toHaveBeenCalledWith("openai", [
        { role: "user", content: "写一个安全开场" },
      ]),
    );
    expect(JSON.stringify(chatMocks.start.mock.calls[0])).not.toMatch(
      /project|filePath|selection|search|apiKey/iu,
    );

    const hostile = '<script>alert("x")</script><a href="https://example.com">点我</a>';
    act(() => {
      onEvent?.({
        ...STARTED,
        kind: "textDelta",
        sequence: 1,
        delta: hostile,
      });
    });
    expect(screen.getByText(hostile)).toBeInTheDocument();
    expect(document.querySelector("script")).toBeNull();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();

    act(() => {
      onEvent?.({
        ...STARTED,
        kind: "completed",
        sequence: 2,
        finishReason: "stop",
      });
    });
    expect(screen.getByText("用量：未提供")).toBeInTheDocument();
    expect(screen.getByLabelText("对话输入")).toBeEnabled();
  });

  it("shows recovery without replay and abandons only after inline confirmation", async () => {
    const user = userEvent.setup();
    const sessionId = "30000000-0000-4000-8000-000000000003";
    const recovered = {
      schemaVersion: 3,
      available: true,
      hasActiveRun: true,
      run: {
        runId: "10000000-0000-4000-8000-000000000001",
        projectId: "20000000-0000-4000-8000-000000000002",
        workflowId: "novelgainker.character-create",
        workflowVersion: 1,
        state: "recovery_required",
        sequence: 5,
        providerId: "deepseek",
        model: "deepseek-v4-pro",
        updatedAt: 5,
        canCancel: false,
        canAbandon: true,
        recoveryRequired: true,
      },
      contextManifest: null,
    };
    workflowRunMocks.status.mockResolvedValue(recovered);
    workflowRunMocks.abandon.mockResolvedValue({
      proposalView: null,
      ...recovered,
      hasActiveRun: false,
      run: {
        ...recovered.run,
        state: "cancelled",
        sequence: 6,
        canAbandon: false,
        recoveryRequired: false,
      },
    });

    render(
      <ConversationPanel
        desktopRuntime
        projectSessionId={sessionId}
        providerState={CONNECTED_STATE}
      />,
    );

    expect(await screen.findByText("上次运行需要你处理")).toBeInTheDocument();
    expect(screen.getByText(/没有重发 AI 请求/u)).toBeInTheDocument();
    expect(workflowRunMocks.cancel).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "放弃本次运行" }));
    expect(screen.getByText(/不会删除或修改任何 Markdown/u)).toBeInTheDocument();
    expect(workflowRunMocks.abandon).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认放弃" }));
    await waitFor(() => expect(workflowRunMocks.abandon).toHaveBeenCalledWith(sessionId, true));
    expect((await screen.findAllByText("已取消")).length).toBeGreaterThan(0);
  });

  it("exposes an explicit stop action for the frozen run", async () => {
    const user = userEvent.setup();
    const onRunActiveChange = vi.fn();
    render(
      <ConversationPanel
        desktopRuntime
        onRunActiveChange={onRunActiveChange}
        providerState={CONNECTED_STATE}
      />,
    );

    await user.type(screen.getByLabelText("对话输入"), "开始生成");
    await user.click(screen.getByRole("button", { name: "发送" }));
    await screen.findByRole("button", { name: "停止" });
    await user.click(screen.getByRole("button", { name: "停止" }));
    expect(chatMocks.cancel).toHaveBeenCalledWith(STARTED.runId);
    expect(onRunActiveChange).toHaveBeenLastCalledWith(true);

    act(() => {
      onEvent?.({ ...STARTED, kind: "cancelled", sequence: 1 });
    });
    await waitFor(() => expect(onRunActiveChange).toHaveBeenLastCalledWith(false));
    expect(screen.getByText(/gpt-5\.6-terra · 已取消/u)).toBeInTheDocument();
  });

  it("retries only through the failed message Provider and requires a new click", async () => {
    const user = userEvent.setup();
    const secondRun = {
      ...STARTED,
      runId: "20000000-0000-4000-8000-000000000002",
    };
    chatMocks.start.mockResolvedValueOnce(STARTED).mockResolvedValueOnce(secondRun);
    const { rerender } = render(
      <ConversationPanel desktopRuntime providerState={CONNECTED_STATE} />,
    );

    await user.type(screen.getByLabelText("对话输入"), "只发给 OpenAI");
    await user.click(screen.getByRole("button", { name: "发送" }));
    await screen.findByRole("button", { name: "停止" });
    act(() => {
      onEvent?.({
        ...STARTED,
        kind: "failed",
        sequence: 1,
        error: { code: "provider.timeout", messageKey: "errors.providerTimeout" },
      });
    });

    const retry = screen.getByRole("button", { name: "使用 OpenAI 重新发送" });
    const switchedState: ProviderStateSnapshot = {
      schemaVersion: 2,
      selectedProviderId: "deepseek",
      providers: [
        {
          providerId: "deepseek",
          configured: true,
          connectionStatus: "connected",
          presetVersion: 1,
          selected: true,
        },
        { ...CONNECTED_STATE.providers[1], selected: false },
      ],
    };
    rerender(<ConversationPanel desktopRuntime providerState={switchedState} />);
    expect(retry).toBeDisabled();
    await user.click(retry);
    expect(chatMocks.start).toHaveBeenCalledOnce();

    rerender(<ConversationPanel desktopRuntime providerState={CONNECTED_STATE} />);
    await user.click(screen.getByRole("button", { name: "使用 OpenAI 重新发送" }));
    await waitFor(() => expect(chatMocks.start).toHaveBeenCalledTimes(2));
    expect(chatMocks.start).toHaveBeenLastCalledWith("openai", [
      { role: "user", content: "只发给 OpenAI" },
    ]);
  });
  it("rejects empty, overlong, and unconfigured sends in the UI", async () => {
    const unconfigured: ProviderStateSnapshot = {
      ...CONNECTED_STATE,
      providers: CONNECTED_STATE.providers.map((provider) =>
        provider.providerId === "openai"
          ? { ...provider, configured: false, connectionStatus: "unconfigured" }
          : provider,
      ),
    };
    const { rerender } = render(<ConversationPanel desktopRuntime providerState={unconfigured} />);
    expect(screen.getByRole("button", { name: "发送" })).toBeDisabled();

    rerender(<ConversationPanel desktopRuntime providerState={CONNECTED_STATE} />);
    fireEvent.change(screen.getByLabelText("对话输入"), {
      target: { value: "a".repeat(32 * 1024 + 1) },
    });
    expect(screen.getByText("输入超过 32 KiB 上限")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "发送" })).toBeDisabled();
    expect(chatMocks.start).not.toHaveBeenCalled();
  });
});
