import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as agentApi from "../lib/tauri/agent";
import type { ProviderStateSnapshot } from "../lib/tauri/provider";
import { AgentTimelinePanel } from "./AgentTimelinePanel";

const CONNECTED: ProviderStateSnapshot = {
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

const SESSION: agentApi.AgentSessionSnapshot = {
  schemaVersion: 1,
  projectSessionId: "desktop-session",
  projectId: "10000000-0000-4000-8000-000000000001",
  agentSessionId: "ses_10000000000040008000000000000001",
  providerId: "openai",
  model: "gpt-5.6-terra",
  packVersion: "1.0.0",
  packHash: "a".repeat(64),
  summaryText: null,
  summaryVersion: 0,
  policy: "review_every_write",
  recentTurns: [],
  undoRecords: [],
  pendingWrites: [],
  fullChatPersisted: false,
  projectWriteCount: 0,
};

const STARTED: agentApi.AgentTurnStarted = {
  runId: "10000000-0000-4000-8000-000000000001",
  turnId: "trn_10000000000040008000000000000001",
  agentSessionId: SESSION.agentSessionId,
  providerId: "openai",
  model: "gpt-5.6-terra",
  selectedSkillIds: ["create-character"],
};

const WRITE_PREVIEW: agentApi.AgentWritePreview = {
  schemaVersion: 1,
  pendingId: "pnd_10000000000040008000000000000001",
  policy: "review_every_write",
  authorizationEffect: "await_confirmation",
  requiresConfirmation: true,
  canConfirm: true,
  transactionHash: "b".repeat(64),
  actionPlanHash: "c".repeat(64),
  sourceSetHash: "d".repeat(64),
  policySnapshotHash: "e".repeat(64),
  highestRisk: "write_safe",
  projectWriteCount: 0,
  files: [
    {
      callId: "call_10000000000040008000000000000002",
      toolId: "project.create_markdown",
      relativePath: "world/harbor.md",
      operation: "create",
      mutationShape: "create_entity_markdown",
      action: "create_entity_markdown",
      risk: "write_safe",
      expectedSourceHash: null,
      resultHash: "f".repeat(64),
      resultBytes: 10,
      fullContent: "港口。\n",
      lineDiff: [{ kind: "add", oldLine: null, newLine: 1, content: "港口。" }],
    },
  ],
};

describe("AM1 unified Agent timeline", () => {
  let emit: ((event: agentApi.AgentTurnEvent) => void) | undefined;

  beforeEach(() => {
    vi.restoreAllMocks();
    emit = undefined;
    vi.spyOn(agentApi, "requestAgentSkillRegistry").mockResolvedValue({
      schemaVersion: 1,
      packId: "novelgainker.novelist-compatible",
      packVersion: "1.0.0",
      packHash: "a".repeat(64),
      runtimeEnabled: true,
      skills: Array.from({ length: 9 }, (_, index) => ({
        id: `skill-${index}`,
        displayName: `Skill ${index}`,
        description: "只读",
        readScopes: ["**/*.md"],
        writeScopes: index < 6 ? ["world/*.md"] : [],
      })),
      readToolIds: [
        "project.list_markdown",
        "project.read_markdown",
        "project.search_markdown",
        "project.validate_markdown",
      ],
      writeToolIds: ["project.create_markdown", "project.patch_markdown"],
    });
    vi.spyOn(agentApi, "requestAgentSession").mockResolvedValue(SESSION);
    vi.spyOn(agentApi, "listenAgentTurnEvents").mockImplementation(async (listener) => {
      emit = listener;
      return () => undefined;
    });
  });

  it("requires an open project and explains the AM2 governed-write boundary", () => {
    render(<AgentTimelinePanel desktopRuntime projectSessionId={null} providerState={CONNECTED} />);
    expect(screen.getByText("先打开一个小说项目")).toBeInTheDocument();
    expect(screen.getByText(/写入只允许进入 Skill 声明的 Markdown 范围/u)).toBeInTheDocument();
    expect(screen.queryByText("检测到迁移前的工作流状态")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "发送" })).toBeDisabled();
  });

  it("routes one input into a visible tool timeline with zero writes", async () => {
    const user = userEvent.setup();
    vi.spyOn(agentApi, "requestAgentTurnStart").mockResolvedValue(STARTED);
    const onRunActiveChange = vi.fn();
    render(
      <AgentTimelinePanel
        desktopRuntime
        onRunActiveChange={onRunActiveChange}
        projectSessionId="desktop-session"
        providerState={CONNECTED}
      />,
    );
    await screen.findByText("项目会话已绑定");
    await user.type(screen.getByLabelText("Agent 会话输入"), "检查人物动机与世界规则");
    await user.click(screen.getByRole("button", { name: "发送" }));
    await waitFor(() => expect(agentApi.requestAgentTurnStart).toHaveBeenCalled());
    expect(agentApi.requestAgentTurnStart).toHaveBeenCalledWith(
      "desktop-session",
      "openai",
      "检查人物动机与世界规则",
    );
    await waitFor(() => expect(onRunActiveChange).toHaveBeenLastCalledWith(true));

    await act(async () => {
      emit?.({
        ...STARTED,
        sequence: 1,
        kind: "toolStarted",
        callId: "call_10000000000040008000000000000001",
        toolId: "project.read_markdown",
        purpose: "读取人物事实",
        path: "characters/hero.md",
      });
      emit?.({
        ...STARTED,
        sequence: 2,
        kind: "toolCompleted",
        callId: "call_10000000000040008000000000000001",
        toolId: "project.read_markdown",
        status: "succeeded",
        sourcePaths: ["characters/hero.md"],
        truncated: false,
        projectWriteCount: 0,
      });
      emit?.({
        ...STARTED,
        sequence: 3,
        kind: "assistantMessage",
        text: "人物动机与规则存在可解释的张力。",
        finalSummary: true,
      });
      emit?.({
        ...STARTED,
        sequence: 4,
        kind: "completed",
        summary: "人物动机与规则存在可解释的张力。",
        usage: null,
        projectWriteCount: 0,
      });
    });
    expect(screen.getByText("project.read_markdown")).toBeInTheDocument();
    expect(screen.getByText("已完成 · 0 写入")).toBeInTheDocument();
    expect(screen.getByText("人物动机与规则存在可解释的张力。")).toBeInTheDocument();
    await waitFor(() => expect(onRunActiveChange).toHaveBeenLastCalledWith(false));
  });

  it("acknowledges cancellation immediately and releases the Turn on the terminal event", async () => {
    const user = userEvent.setup();
    vi.spyOn(agentApi, "requestAgentTurnStart").mockResolvedValue(STARTED);
    vi.spyOn(agentApi, "requestAgentTurnCancel").mockResolvedValue({ cancelRequested: true });
    render(
      <AgentTimelinePanel
        desktopRuntime
        projectSessionId="desktop-session"
        providerState={CONNECTED}
      />,
    );
    await screen.findByText("项目会话已绑定");
    await user.type(screen.getByLabelText("Agent 会话输入"), "分析一段很长的故事背景");
    await user.click(screen.getByRole("button", { name: "发送" }));
    await screen.findByRole("button", { name: "取消 Turn" });
    await user.click(screen.getByRole("button", { name: "取消 Turn" }));

    await waitFor(() =>
      expect(agentApi.requestAgentTurnCancel).toHaveBeenCalledWith(STARTED.runId),
    );
    expect(screen.getByRole("button", { name: "正在取消…" })).toBeDisabled();
    expect(
      screen.getByText("正在中断 Provider 请求；不会再启动新的工具或写入"),
    ).toBeInTheDocument();

    await act(async () => {
      emit?.({
        ...STARTED,
        sequence: 2,
        kind: "cancelled",
        projectWriteCount: 0,
      });
    });
    expect(await screen.findByRole("button", { name: "发送" })).toBeDisabled();
    expect(screen.queryByRole("button", { name: "正在取消…" })).not.toBeInTheDocument();
    expect(screen.getByText("已取消")).toBeInTheDocument();
  });

  it("keeps unsent input when a real workflow is still running", async () => {
    const user = userEvent.setup();
    vi.spyOn(agentApi, "requestAgentTurnStart").mockRejectedValue({
      code: "workflow.run_active",
      messageKey: "errors.workflowRunActive",
    });
    const { container } = render(
      <AgentTimelinePanel
        desktopRuntime
        projectSessionId="desktop-session"
        providerState={CONNECTED}
      />,
    );
    await screen.findByText("项目会话已绑定");
    const composer = screen.getByLabelText("Agent 会话输入");
    await user.type(composer, "把港口规则同步到世界观");
    await user.click(screen.getByRole("button", { name: "发送" }));

    expect(await screen.findByText("已有工作流正在执行")).toBeInTheDocument();
    expect(
      screen.getByText("请等待当前工作流结束，或在对应运行界面取消后重试。"),
    ).toBeInTheDocument();
    expect(screen.getByText("错误代码：workflow.run_active")).toBeInTheDocument();
    expect(composer).toHaveValue("把港口规则同步到世界观");
    expect(composer).toHaveAttribute("aria-describedby", "agent-composer-error");
    expect(container.querySelector('.agent-message[data-role="user"]')).toBeNull();
    expect(screen.queryByRole("button", { name: "确认结束旧工作流" })).not.toBeInTheDocument();
  });

  it("keeps an asynchronous Turn failure visible and restores the submitted input", async () => {
    const user = userEvent.setup();
    vi.spyOn(agentApi, "requestAgentTurnStart").mockResolvedValue({
      ...STARTED,
      selectedSkillIds: ["novel-story-telling"],
    });
    render(
      <AgentTimelinePanel
        desktopRuntime
        projectSessionId="desktop-session"
        providerState={CONNECTED}
      />,
    );
    await screen.findByText("项目会话已绑定");
    const composer = screen.getByLabelText("Agent 会话输入");
    const request = "被冤枉的流放犯人抵达岭南，开始他的传奇故事。";
    await user.type(composer, request);
    await user.click(screen.getByRole("button", { name: "发送" }));
    await waitFor(() => expect(agentApi.requestAgentTurnStart).toHaveBeenCalled());

    await act(async () => {
      emit?.({
        ...STARTED,
        selectedSkillIds: ["novel-story-telling"],
        sequence: Number.MAX_SAFE_INTEGER,
        kind: "failed",
        error: {
          code: "agent-action.repair-exhausted",
          messageKey: "errors.agentContract",
        },
      });
    });

    expect(await screen.findByText("Agent 操作格式未通过校验")).toBeInTheDocument();
    expect(screen.getByText("错误代码：agent-action.repair-exhausted")).toBeInTheDocument();
    expect(screen.getByText("Turn 失败 · 输入已恢复")).toBeInTheDocument();
    expect(composer).toHaveValue(request);
    await waitFor(() => expect(agentApi.requestAgentSession).toHaveBeenCalledTimes(2));
    expect(screen.getByText("Agent 操作格式未通过校验")).toBeInTheDocument();
  });

  it("clears only bounded Agent state after an explicit confirmation", async () => {
    const user = userEvent.setup();
    vi.spyOn(agentApi, "requestAgentSessionClear").mockResolvedValue({
      counts: {
        sessionsRemoved: 1,
        turnsRemoved: 1,
        messageIndexesRemoved: 2,
        undoRecordsRemoved: 0,
        canonWriteCount: 0,
      },
      session: { ...SESSION, agentSessionId: "ses_20000000000040008000000000000002" },
    });
    render(
      <AgentTimelinePanel
        desktopRuntime
        projectSessionId="desktop-session"
        providerState={CONNECTED}
      />,
    );
    await user.click(await screen.findByRole("button", { name: "清除会话" }));
    expect(screen.getByText(/不会修改任何项目 Markdown/u)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "确认清除" }));
    await waitFor(() =>
      expect(agentApi.requestAgentSessionClear).toHaveBeenCalledWith("desktop-session"),
    );
  });

  it("restores a pending diff and sends a hash-bound confirmation", async () => {
    const user = userEvent.setup();
    vi.mocked(agentApi.requestAgentSession).mockResolvedValue({
      ...SESSION,
      pendingWrites: [WRITE_PREVIEW],
    });
    vi.spyOn(agentApi, "requestAgentWriteConfirm").mockResolvedValue({
      receipt: {
        transactionHash: WRITE_PREVIEW.transactionHash,
        committedResults: [{ path: "world/harbor.md", resultHash: "f".repeat(64) }],
      },
      undoRecord: { transactionHash: WRITE_PREVIEW.transactionHash, status: "ready" },
      projectWriteCount: 1,
    });
    render(
      <AgentTimelinePanel
        desktopRuntime
        projectSessionId="desktop-session"
        providerState={CONNECTED}
      />,
    );
    expect(await screen.findByText("world/harbor.md")).toBeInTheDocument();
    expect(screen.getByText(WRITE_PREVIEW.transactionHash)).toBeInTheDocument();
    expect(screen.getByText(WRITE_PREVIEW.sourceSetHash)).toBeInTheDocument();
    expect(screen.getByText(WRITE_PREVIEW.files[0].resultHash)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "确认并写入全部差异" }));
    await waitFor(() =>
      expect(agentApi.requestAgentWriteConfirm).toHaveBeenCalledWith(
        "desktop-session",
        WRITE_PREVIEW,
      ),
    );
  });
});
