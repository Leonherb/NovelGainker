import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
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
const runtimeView = (
  conversation: conversationApi.ConversationSession,
): conversationApi.ConversationRuntimeView => ({
  schemaVersion: 1,
  conversation,
  taskActivities: [],
});
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
          content: "今天天气怎么样？",
          createdAtMs: 1,
        },
      ],
    },
  ],
};

describe("NR-005 ConversationView", () => {
  let emit: ((event: conversationApi.ConversationTurnEvent) => void) | undefined;

  beforeEach(() => {
    vi.restoreAllMocks();
    emit = undefined;
    vi.spyOn(conversationApi, "listenConversationTurnEvents").mockImplementation(
      async (handler) => {
        emit = handler;
        return () => undefined;
      },
    );
  });

  it("sends ordinary chat, shows the saved user message, and streams natural text", async () => {
    const user = userEvent.setup();
    const load = vi
      .spyOn(conversationApi, "requestConversationSession")
      .mockResolvedValueOnce(runtimeView(blankSession))
      .mockResolvedValueOnce(runtimeView(runningSession))
      .mockResolvedValueOnce(
        runtimeView({
          ...runningSession,
          turns: [
            {
              ...runningSession.turns[0],
              status: "completed",
              updatedAtMs: 2,
              messages: [
                ...runningSession.turns[0].messages,
                {
                  messageId: ASSISTANT_MESSAGE_ID,
                  role: "assistant",
                  content: "今天适合聊故事。",
                  createdAtMs: 2,
                },
              ],
            },
          ],
        }),
      );
    vi.spyOn(conversationApi, "requestConversationTurnStart").mockResolvedValue({
      sessionId: PROJECT_SESSION_ID,
      conversationSessionId: CONVERSATION_SESSION_ID,
      turnId: TURN_ID,
      providerId: "openai",
      model: "gpt-5.6-terra",
    });

    render(
      <ConversationView projectSessionId={PROJECT_SESSION_ID} providerState={configuredProvider} />,
    );
    expect(await screen.findByText("从任何话题开始")).toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: "对话输入" }), "今天天气怎么样？");
    await user.click(screen.getByRole("button", { name: "发送" }));

    expect(await screen.findByText("今天天气怎么样？")).toBeInTheDocument();
    expect(screen.getByText("已保存")).toBeInTheDocument();
    act(() => {
      emit?.({
        sessionId: PROJECT_SESSION_ID,
        turnId: TURN_ID,
        providerId: "openai",
        model: "gpt-5.6-terra",
        sequence: 1,
        kind: "textDelta",
        delta: "今天适合聊故事。",
      });
    });
    expect(screen.getByText("今天适合聊故事。")).toBeInTheDocument();
    act(() => {
      emit?.({
        sessionId: PROJECT_SESSION_ID,
        turnId: TURN_ID,
        providerId: "openai",
        model: "gpt-5.6-terra",
        sequence: 2,
        kind: "taskActivity",
        taskActivity: {
          taskId: "70000000-0000-4000-8000-000000000007",
          sessionId: CONVERSATION_SESSION_ID,
          turnId: TURN_ID,
          status: "running",
          displayLabel: "不会直接显示的技术标签",
          taskRevision: 2,
        },
      });
    });
    expect(screen.getByText("正在处理……")).toBeInTheDocument();
    expect(screen.queryByText("不会直接显示的技术标签")).not.toBeInTheDocument();
    act(() => {
      emit?.({
        sessionId: PROJECT_SESSION_ID,
        turnId: TURN_ID,
        providerId: "openai",
        model: "gpt-5.6-terra",
        sequence: 3,
        kind: "taskActivity",
        taskActivity: {
          taskId: "70000000-0000-4000-8000-000000000007",
          sessionId: CONVERSATION_SESSION_ID,
          turnId: TURN_ID,
          status: "succeeded",
          displayLabel: "不会直接显示的技术标签",
          taskRevision: 3,
        },
      });
    });
    expect(screen.getByText("已完成相关操作。")).toBeInTheDocument();
    act(() => {
      emit?.({
        sessionId: PROJECT_SESSION_ID,
        turnId: TURN_ID,
        providerId: "openai",
        model: "gpt-5.6-terra",
        sequence: 4,
        kind: "completed",
        finishReason: "stop",
      });
    });
    await waitFor(() => expect(load).toHaveBeenCalledTimes(3));
    expect(await screen.findByText("已完成")).toBeInTheDocument();
  });

  it("requests idempotent stop and keeps the visible incomplete text", async () => {
    const user = userEvent.setup();
    vi.spyOn(conversationApi, "requestConversationSession")
      .mockResolvedValueOnce(runtimeView(blankSession))
      .mockResolvedValueOnce(runtimeView(runningSession))
      .mockResolvedValueOnce(
        runtimeView({
          ...runningSession,
          turns: [
            {
              ...runningSession.turns[0],
              status: "cancelled",
              updatedAtMs: 2,
              messages: [
                ...runningSession.turns[0].messages,
                {
                  messageId: ASSISTANT_MESSAGE_ID,
                  role: "assistant",
                  content: "已经写下半句。\n\n本次内容未生成完成。",
                  createdAtMs: 2,
                },
              ],
            },
          ],
        }),
      );
    vi.spyOn(conversationApi, "requestConversationTurnStart").mockResolvedValue({
      sessionId: PROJECT_SESSION_ID,
      conversationSessionId: CONVERSATION_SESSION_ID,
      turnId: TURN_ID,
      providerId: "openai",
      model: "gpt-5.6-terra",
    });
    const cancel = vi
      .spyOn(conversationApi, "requestConversationTurnCancel")
      .mockResolvedValue({ cancelRequested: true });

    render(
      <ConversationView projectSessionId={PROJECT_SESSION_ID} providerState={configuredProvider} />,
    );
    await screen.findByText("从任何话题开始");
    await user.type(screen.getByRole("textbox", { name: "对话输入" }), "今天天气怎么样？");
    await user.click(screen.getByRole("button", { name: "发送" }));
    await user.click(await screen.findByRole("button", { name: "停止" }));
    expect(cancel).toHaveBeenCalledWith(PROJECT_SESSION_ID, TURN_ID);
    expect(screen.getByRole("button", { name: "正在停止" })).toBeDisabled();

    act(() => {
      emit?.({
        sessionId: PROJECT_SESSION_ID,
        turnId: TURN_ID,
        providerId: "openai",
        model: "gpt-5.6-terra",
        sequence: 2,
        kind: "cancelled",
      });
    });
    expect(await screen.findByText(/本次内容未生成完成/u)).toBeInTheDocument();
    expect(screen.getByText("已停止 · 未完成")).toBeInTheDocument();
  });
});
