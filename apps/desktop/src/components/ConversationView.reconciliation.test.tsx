import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ConversationView } from "./ConversationView";
import { DEFAULT_PROVIDER_STATE } from "./provider-settings-model";
import * as conversationApi from "../lib/tauri/conversation";

const PROJECT_SESSION_ID = "10000000-0000-4000-8000-000000000001";
const CONVERSATION_SESSION_ID = "20000000-0000-4000-8000-000000000002";
const PROJECT_ID = "30000000-0000-4000-8000-000000000003";
const TURN_ID = "40000000-0000-4000-8000-000000000004";
const USER_MESSAGE_ID = "50000000-0000-4000-8000-000000000005";
const ASSISTANT_MESSAGE_ID = "60000000-0000-4000-8000-000000000006";

const configuredProvider = {
  ...DEFAULT_PROVIDER_STATE,
  providers: DEFAULT_PROVIDER_STATE.providers.map((provider) =>
    provider.providerId === "openai"
      ? { ...provider, configured: true, connectionStatus: "untested" as const }
      : provider,
  ),
};

const blankSession: conversationApi.ConversationSession = {
  schemaVersion: 1,
  projectId: PROJECT_ID,
  sessionId: CONVERSATION_SESSION_ID,
  turns: [],
};

const runningSession: conversationApi.ConversationSession = {
  ...blankSession,
  turns: [
    {
      turnId: TURN_ID,
      providerId: "openai",
      model: "gpt-5.6-terra",
      status: "running",
      startedAtMs: 1,
      updatedAtMs: 1,
      messages: [
        {
          messageId: USER_MESSAGE_ID,
          role: "user",
          content: "继续聊下去",
          createdAtMs: 1,
        },
      ],
    },
  ],
};

const startedTurn: conversationApi.ConversationTurnStarted = {
  sessionId: PROJECT_SESSION_ID,
  conversationSessionId: CONVERSATION_SESSION_ID,
  turnId: TURN_ID,
  providerId: "openai",
  model: "gpt-5.6-terra",
};

function terminalSession(
  status: Exclude<conversationApi.ConversationTurnStatus, "running">,
): conversationApi.ConversationSession {
  return {
    ...runningSession,
    turns: [
      {
        ...runningSession.turns[0],
        status,
        updatedAtMs: 2,
        messages: [
          ...runningSession.turns[0].messages,
          {
            messageId: ASSISTANT_MESSAGE_ID,
            role: "assistant",
            content: "Store 中的最终回复。",
            createdAtMs: 2,
          },
        ],
      },
    ],
  };
}

async function renderReadyConversation() {
  render(
    <ConversationView projectSessionId={PROJECT_SESSION_ID} providerState={configuredProvider} />,
  );
  await screen.findByText("从任何话题开始");
}

describe("NR-G1-R01 Conversation reconciliation", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("keeps sending disabled until the Event listener is ready", async () => {
    let resolveListener: ((dispose: () => void) => void) | undefined;
    const listenerReady = new Promise<() => void>((resolve) => {
      resolveListener = resolve;
    });
    vi.spyOn(conversationApi, "listenConversationTurnEvents").mockReturnValue(listenerReady);
    const load = vi
      .spyOn(conversationApi, "requestConversationSession")
      .mockResolvedValue(blankSession);

    render(
      <ConversationView projectSessionId={PROJECT_SESSION_ID} providerState={configuredProvider} />,
    );

    const input = screen.getByRole("textbox", { name: "对话输入" });
    expect(input).toBeDisabled();
    expect(screen.getByText("正在准备对话连接。")).toBeInTheDocument();
    expect(load).not.toHaveBeenCalled();

    await act(async () => {
      resolveListener?.(() => undefined);
      await listenerReady;
    });

    await waitFor(() => expect(input).toBeEnabled());
    expect(load).toHaveBeenCalledOnce();
  });

  it.each([
    ["completed", "已完成"],
    ["failed", "未完成"],
    ["cancelled", "已停止 · 未完成"],
    ["not_generated", "未生成"],
  ] as const)(
    "recovers the %s terminal state from Store when the terminal Event is lost",
    async (status, label) => {
      const user = userEvent.setup();
      vi.spyOn(conversationApi, "listenConversationTurnEvents").mockResolvedValue(() => undefined);
      const load = vi
        .spyOn(conversationApi, "requestConversationSession")
        .mockResolvedValueOnce(blankSession)
        .mockResolvedValueOnce(runningSession)
        .mockResolvedValueOnce(terminalSession(status));
      const start = vi
        .spyOn(conversationApi, "requestConversationTurnStart")
        .mockResolvedValue(startedTurn);

      await renderReadyConversation();
      await user.type(screen.getByRole("textbox", { name: "对话输入" }), "继续聊下去");
      await user.click(screen.getByRole("button", { name: "发送" }));

      expect(await screen.findByText("继续聊下去")).toBeInTheDocument();
      expect(await screen.findByText(label, {}, { timeout: 2_500 })).toBeInTheDocument();
      expect(screen.getByText("Store 中的最终回复。")).toBeInTheDocument();
      expect(start).toHaveBeenCalledOnce();
      expect(load).toHaveBeenCalledTimes(3);
    },
    4_000,
  );

  it("terminalizes a requested stop from Store without a cancelled Event", async () => {
    const user = userEvent.setup();
    vi.spyOn(conversationApi, "listenConversationTurnEvents").mockResolvedValue(() => undefined);
    vi.spyOn(conversationApi, "requestConversationSession")
      .mockResolvedValueOnce(blankSession)
      .mockResolvedValueOnce(runningSession)
      .mockResolvedValueOnce(terminalSession("cancelled"));
    vi.spyOn(conversationApi, "requestConversationTurnStart").mockResolvedValue(startedTurn);
    const cancel = vi
      .spyOn(conversationApi, "requestConversationTurnCancel")
      .mockResolvedValue({ cancelRequested: true });

    await renderReadyConversation();
    await user.type(screen.getByRole("textbox", { name: "对话输入" }), "继续聊下去");
    await user.click(screen.getByRole("button", { name: "发送" }));
    await user.click(await screen.findByRole("button", { name: "停止" }));

    expect(screen.getByRole("button", { name: "正在停止" })).toBeDisabled();
    expect(await screen.findByText("已停止 · 未完成", {}, { timeout: 2_500 })).toBeInTheDocument();
    expect(cancel).toHaveBeenCalledWith(PROJECT_SESSION_ID, TURN_ID);
  });

  it("shows an explicit synchronization failure and retries only the Store read", async () => {
    const user = userEvent.setup();
    vi.spyOn(conversationApi, "listenConversationTurnEvents").mockResolvedValue(() => undefined);
    const load = vi
      .spyOn(conversationApi, "requestConversationSession")
      .mockResolvedValueOnce(blankSession)
      .mockResolvedValueOnce(runningSession)
      .mockRejectedValueOnce({
        code: "runtime.unknown",
        messageKey: "errors.runtimeUnknown",
      })
      .mockResolvedValueOnce(terminalSession("completed"));
    const start = vi
      .spyOn(conversationApi, "requestConversationTurnStart")
      .mockResolvedValue(startedTurn);

    await renderReadyConversation();
    await user.type(screen.getByRole("textbox", { name: "对话输入" }), "继续聊下去");
    await user.click(screen.getByRole("button", { name: "发送" }));

    expect(await screen.findByText("状态同步失败", {}, { timeout: 2_500 })).toBeInTheDocument();
    expect(screen.queryByText("正在回复")).not.toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "对话输入" })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "重试状态同步" }));

    expect(await screen.findByText("已完成")).toBeInTheDocument();
    expect(start).toHaveBeenCalledOnce();
    expect(load).toHaveBeenCalledTimes(4);
  });

  it("recovers from listener registration failure without starting a Turn", async () => {
    const user = userEvent.setup();
    const listen = vi
      .spyOn(conversationApi, "listenConversationTurnEvents")
      .mockRejectedValueOnce({
        code: "runtime.unknown",
        messageKey: "errors.runtimeUnknown",
      })
      .mockResolvedValueOnce(() => undefined);
    vi.spyOn(conversationApi, "requestConversationSession").mockResolvedValue(blankSession);
    const start = vi.spyOn(conversationApi, "requestConversationTurnStart");

    render(
      <ConversationView projectSessionId={PROJECT_SESSION_ID} providerState={configuredProvider} />,
    );
    expect(await screen.findByText("状态同步失败")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "对话输入" })).toBeDisabled();
    expect(start).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "重试状态同步" }));

    await waitFor(() => expect(screen.getByRole("textbox", { name: "对话输入" })).toBeEnabled());
    expect(listen).toHaveBeenCalledTimes(2);
    expect(start).not.toHaveBeenCalled();
  });
});
