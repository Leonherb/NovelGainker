import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AgentConversation } from "./AgentConversation";
import { DEFAULT_PROVIDER_STATE } from "./provider-settings-model";
import * as agentApi from "../lib/tauri/agent";

const readyProvider = {
  ...DEFAULT_PROVIDER_STATE,
  providers: DEFAULT_PROVIDER_STATE.providers.map((item) => ({
    ...item,
    configured: item.providerId === "openai",
    connectionStatus: item.providerId === "openai" ? ("connected" as const) : item.connectionStatus,
  })),
};

const session: agentApi.AgentSessionSnapshotV2 = {
  schemaVersion: 2,
  projectId: "10000000-0000-4000-8000-000000000001",
  historyConsentRequired: false,
  branchSelectionRequired: false,
  activeBranchId: "brn_10000000000040008000000000000001",
  sessionId: "ses_10000000000040008000000000000001",
  branches: [],
  messages: [],
  memory: null,
  draftQuota: 0,
  writePolicy: "auto_apply_safe",
  writePolicyHash: "a".repeat(64),
  contextMarks: {
    schemaVersion: 1,
    projectId: "10000000-0000-4000-8000-000000000001",
    marks: [],
    storeHash: "b".repeat(64),
  },
};

const registry = {
  schemaVersion: 2 as const,
  packId: "novelgainker.novelist-compatible",
  packVersion: "1.0.0",
  packHash: "a".repeat(64),
  sourceCommit: "b".repeat(40),
  runtimeSource: "bundled_release_only" as const,
  runtimeEnabled: true as const,
  eligibleSkillIds: [],
  skills: Array.from({ length: 9 }, (_, index) => ({
    id: `skill-${index}`,
    displayName: `Skill ${index}`,
    description: "描述",
    triggerSummary: "触发条件",
    manifestSha256: "c".repeat(64),
    resources: [],
  })),
};

describe("Agent Protocol V2 Conversation workspace", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(agentApi, "requestAgentSkillRegistry").mockResolvedValue(registry);
    vi.spyOn(agentApi, "requestAgentSession").mockResolvedValue(session);
    vi.spyOn(agentApi, "listenAgentTurnEvents").mockResolvedValue(() => undefined);
    vi.spyOn(agentApi, "requestAgentTurnCancel").mockResolvedValue({ cancelRequested: true });
  });

  it("requires explicit project-history consent before creating Agent storage", async () => {
    vi.spyOn(agentApi, "requestAgentSession")
      .mockResolvedValueOnce({
        ...session,
        historyConsentRequired: true,
        activeBranchId: null,
        sessionId: null,
      })
      .mockResolvedValueOnce(session);
    render(
      <AgentConversation
        activityHost={document.createElement("div")}
        desktopRuntime
        projectSessionId="desktop-session"
        providerState={readyProvider}
      />,
    );
    const user = userEvent.setup();
    expect(
      await screen.findByRole("dialog", { name: "是否在工程内保存完整可见对话？" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "继续并保存" }));
    await waitFor(() =>
      expect(agentApi.requestAgentSession).toHaveBeenLastCalledWith("desktop-session", true),
    );
  });

  it("keeps messages in the center and Skill/source activity in the right host", async () => {
    const host = document.createElement("div");
    document.body.append(host);
    let onEvent: ((event: agentApi.AgentTurnEventV2) => void) | undefined;
    vi.spyOn(agentApi, "listenAgentTurnEvents").mockImplementation(async (handler) => {
      onEvent = handler;
      return () => undefined;
    });
    vi.spyOn(agentApi, "requestAgentTurnStart").mockResolvedValue({
      schemaVersion: 2,
      runId: "run_10000000000040008000000000000001",
      turnId: "trn_10000000000040008000000000000001",
      providerId: "openai",
      model: "gpt-5.6-terra",
    });
    render(
      <AgentConversation
        activityHost={host}
        desktopRuntime
        projectSessionId="desktop-session"
        providerState={readyProvider}
      />,
    );
    const user = userEvent.setup();
    await screen.findByText("从一句话开始");
    await user.type(screen.getByRole("textbox", { name: "输入给 AI 的消息" }), "林遥是谁？");
    await user.click(screen.getByRole("button", { name: /发送/u }));
    await waitFor(() => expect(agentApi.requestAgentTurnStart).toHaveBeenCalled());
    const base = {
      schemaVersion: 2 as const,
      runId: "run_10000000000040008000000000000001",
      turnId: "trn_10000000000040008000000000000001",
      projectSessionId: "desktop-session",
      providerId: "openai" as const,
      model: "gpt-5.6-terra",
    };
    await act(async () => {
      onEvent?.({
        ...base,
        sequence: 1,
        kind: "skillSelection",
        selectedSkillIds: ["create-character"],
        intentClass: "project_discussion",
        visibleReason: "读取人物设定",
      });
      onEvent?.({
        ...base,
        sequence: 2,
        kind: "contextSources",
        indexHash: "d".repeat(64),
        sources: [
          {
            sourceId: "src_10000000000040008000000000000001",
            relativePath: "characters/林遥.md",
            entity: "林遥",
            fieldPath: "name",
            headingPath: ["人物"],
            contentHash: "e".repeat(64),
            excerptHash: "f".repeat(64),
            excerpt: "林遥是一名潮汐记录员。",
            truncated: false,
            untrusted: true,
          },
        ],
      });
      onEvent?.({ ...base, sequence: 3, kind: "conversationDelta", delta: "林遥是潮汐记录员。" });
    });
    const center = screen.getByRole("region", { name: "小说创作对话消息" });
    expect(within(center).getByText("林遥是潮汐记录员。")).toBeInTheDocument();
    expect(within(center).queryByText("characters/林遥.md")).toBeNull();
    expect(within(host).getByText("create-character")).toBeInTheDocument();
    expect(within(host).getByText("characters/林遥.md")).toBeInTheDocument();
    host.remove();
  });

  it("shows visible progress before the first delta and leaves cancelling on the terminal event", async () => {
    let onEvent: ((event: agentApi.AgentTurnEventV2) => void) | undefined;
    vi.spyOn(agentApi, "listenAgentTurnEvents").mockImplementation(async (handler) => {
      onEvent = handler;
      return () => undefined;
    });
    vi.spyOn(agentApi, "requestAgentTurnStart").mockResolvedValue({
      schemaVersion: 2,
      runId: "run_10000000000040008000000000000001",
      turnId: "trn_10000000000040008000000000000001",
      providerId: "openai",
      model: "gpt-5.6-terra",
    });
    render(
      <AgentConversation
        activityHost={document.createElement("div")}
        desktopRuntime
        projectSessionId="desktop-session"
        providerState={readyProvider}
      />,
    );
    const user = userEvent.setup();
    const center = await screen.findByRole("region", { name: "小说创作对话消息" });
    await user.type(screen.getByRole("textbox", { name: "输入给 AI 的消息" }), "继续讨论林遥");
    await waitFor(() => expect(screen.getByRole("button", { name: /发送/u })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: /发送/u }));

    expect(await within(center).findByText("正在处理你的请求，请稍候……")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "停止" }));
    expect(agentApi.requestAgentTurnCancel).toHaveBeenCalledWith(
      "run_10000000000040008000000000000001",
    );
    expect(await within(center).findByText("正在停止")).toBeInTheDocument();

    await act(async () => {
      onEvent?.({
        schemaVersion: 2,
        runId: "run_10000000000040008000000000000001",
        turnId: "trn_10000000000040008000000000000001",
        projectSessionId: "desktop-session",
        providerId: "openai",
        model: "gpt-5.6-terra",
        sequence: 1,
        kind: "cancelled",
        message: null,
        usage: {
          providerCalls: 1,
          totalProviderInputBytes: 10,
          totalProviderOutputBytes: 0,
          accumulatedContextResultBytes: 0,
          structuredRepairCalls: 0,
          totalProviderInputTokens: 3,
          totalProviderOutputTokens: 0,
        },
        agentCanonWriteCount: 0,
      });
    });

    await waitFor(() =>
      expect(screen.getByRole("textbox", { name: "输入给 AI 的消息" })).toBeEnabled(),
    );
    expect(screen.queryByRole("button", { name: "停止" })).toBeNull();
    expect(within(center).queryByText("正在停止")).toBeNull();
  });

  it("renders the hash-bound Write Preview only in the right panel and confirms by IDs", async () => {
    const host = document.createElement("div");
    document.body.append(host);
    let onEvent: ((event: agentApi.AgentTurnEventV2) => void) | undefined;
    vi.spyOn(agentApi, "listenAgentTurnEvents").mockImplementation(async (handler) => {
      onEvent = handler;
      return () => undefined;
    });
    vi.spyOn(agentApi, "requestAgentTurnStart").mockResolvedValue({
      schemaVersion: 2,
      runId: "run_10000000000040008000000000000001",
      turnId: "trn_10000000000040008000000000000001",
      providerId: "openai",
      model: "gpt-5.6-terra",
    });
    const receipt: agentApi.AgentCommitReceiptV2 = {
      schemaVersion: 2,
      receiptId: `rcpt_${"a".repeat(64)}`,
      transactionHash: "a".repeat(64),
      resultHash: "b".repeat(64),
      relativePaths: ["chapters/001.md"],
      committedAtMs: 10,
      undoExpiresAtMs: 20,
      projectWriteCount: 1,
    };
    vi.spyOn(agentApi, "requestAgentWriteConfirmation").mockResolvedValue(receipt);
    render(
      <AgentConversation
        activityHost={host}
        desktopRuntime
        projectSessionId="desktop-session"
        providerState={readyProvider}
      />,
    );
    const user = userEvent.setup();
    await screen.findByText("从一句话开始");
    await user.type(screen.getByRole("textbox", { name: "输入给 AI 的消息" }), "锚定为正文");
    await user.click(screen.getByRole("button", { name: /发送/u }));
    await waitFor(() => expect(agentApi.requestAgentTurnStart).toHaveBeenCalled());
    const plan: agentApi.AgentWritePlanV2 = {
      schemaVersion: 2,
      pendingId: "pnd_10000000000040008000000000000001",
      projectId: session.projectId,
      sessionId: session.sessionId!,
      branchId: session.activeBranchId!,
      turnId: "trn_10000000000040008000000000000001",
      proposalId: "prop_10000000000040008000000000000001",
      artifactId: "art_10000000000040008000000000000001",
      proposalHash: "c".repeat(64),
      planHash: "d".repeat(64),
      sourceBindingHash: "e".repeat(64),
      conflictSetHash: null,
      policy: "review_every_write",
      policyHash: "f".repeat(64),
      preview: {
        schemaVersion: 1,
        pendingId: "pnd_10000000000040008000000000000001",
        policy: "review_every_write",
        requiresConfirmation: true,
        canConfirm: true,
        authorizationEffect: "await_confirmation",
        transactionHash: "1".repeat(64),
        actionPlanHash: "2".repeat(64),
        sourceSetHash: "3".repeat(64),
        policySnapshotHash: "f".repeat(64),
        highestRisk: "always-confirm",
        projectWriteCount: 1,
        files: [
          {
            callId: "call_10000000000040008000000000000001",
            toolId: "project.create_markdown",
            relativePath: "chapters/001.md",
            operation: "create",
            mutationShape: "create_file",
            action: "create chapter draft",
            risk: "always-confirm",
            expectedSourceHash: null,
            fullContent: "# 第一章\n正文",
            resultHash: "4".repeat(64),
            resultBytes: 20,
            lineDiff: [{ kind: "add", oldLine: null, newLine: 1, content: "# 第一章" }],
          },
        ],
      },
    };
    await act(async () => {
      onEvent?.({
        schemaVersion: 2,
        runId: "run_10000000000040008000000000000001",
        turnId: plan.turnId,
        projectSessionId: "desktop-session",
        providerId: "openai",
        model: "gpt-5.6-terra",
        sequence: 1,
        kind: "writePlan",
        plan,
      });
    });
    const center = screen.getByRole("region", { name: "小说创作对话消息" });
    expect(within(host).getByText("Write Preview")).toBeInTheDocument();
    expect(within(host).getByText("chapters/001.md")).toBeInTheDocument();
    expect(within(center).queryByText("Write Preview")).toBeNull();
    await user.click(within(host).getByRole("button", { name: "确认写入" }));
    await waitFor(() =>
      expect(agentApi.requestAgentWriteConfirmation).toHaveBeenCalledWith("desktop-session", plan),
    );
    expect(await within(host).findByText("Commit Receipt")).toBeInTheDocument();
    host.remove();
  });
});
