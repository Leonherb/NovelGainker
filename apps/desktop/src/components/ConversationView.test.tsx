import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ConversationView } from "./ConversationView";
import { DEFAULT_PROVIDER_STATE } from "./provider-settings-model";
import * as conversationApi from "../lib/tauri/conversation";
import * as publicationApi from "../lib/tauri/publication";

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
    vi.spyOn(publicationApi, "requestPublications").mockResolvedValue({
      schemaVersion: 1,
      items: [],
      nextCursor: null,
    });
    emit = undefined;
    vi.spyOn(conversationApi, "listenConversationTurnEvents").mockImplementation(
      async (handler) => {
        emit = handler;
        return () => undefined;
      },
    );
  });

  it("restores every Host tool record after reopening without generating an API reply", async () => {
    const tasks: conversationApi.TaskActivity[] = [
      {
        taskId: "70000000-0000-4000-8000-000000000007",
        sessionId: CONVERSATION_SESSION_ID,
        turnId: TURN_ID,
        status: "succeeded",
        displayLabel: "读取工程资料",
        taskRevision: 7,
        toolName: "novelist.read_context",
        toolVersion: 2,
        startedAtMs: 1,
        updatedAtMs: 2,
        errorCode: null,
      },
      {
        taskId: "80000000-0000-4000-8000-000000000008",
        sessionId: CONVERSATION_SESSION_ID,
        turnId: TURN_ID,
        status: "failed",
        displayLabel: "保存章节或维护资料",
        taskRevision: 2,
        toolName: "novelist.propose_change",
        toolVersion: 2,
        startedAtMs: 3,
        updatedAtMs: 4,
        errorCode: "canon.change_method_not_read",
      },
    ];
    vi.spyOn(conversationApi, "requestConversationSession").mockResolvedValue({
      ...runtimeView({
        ...runningSession,
        turns: [{ ...runningSession.turns[0], status: "not_generated" }],
      }),
      taskActivities: tasks,
    });
    const start = vi.spyOn(conversationApi, "requestConversationTurnStart");
    const props = {
      projectSessionId: PROJECT_SESSION_ID,
      providerState: configuredProvider,
      readOnly: true,
    };
    const first = render(<ConversationView {...props} />);
    await screen.findByText("Gainker 工具记录");
    expect(
      screen.getAllByText(/读取工程资料|保存章节或维护资料/).map((element) => element.textContent),
    ).toEqual(["读取工程资料", "保存章节或维护资料"]);
    await userEvent.click(screen.getByText("保存章节或维护资料"));
    expect(screen.getByText("canon.change_method_not_read")).toBeVisible();
    expect(screen.getByText("novelist.propose_change")).toBeVisible();
    expect(screen.getByText(/缺少本轮有效的方法读取回执/)).toBeVisible();
    first.unmount();
    render(<ConversationView {...props} />);
    await screen.findByText("Gainker 工具记录");
    expect(screen.getByText("执行完成")).toBeInTheDocument();
    expect(screen.getByText("执行失败")).toBeInTheDocument();
    expect(start).not.toHaveBeenCalled();
  });

  it.each([false, true, undefined])(
    "only asserts no tools when Host has a definite false dispatch fact (%s)",
    async (toolDispatched) => {
      vi.spyOn(conversationApi, "requestConversationSession").mockResolvedValue(
        runtimeView({
          ...runningSession,
          turns: [
            {
              ...runningSession.turns[0],
              status: "completed",
              retryState:
                toolDispatched === undefined
                  ? undefined
                  : {
                      attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
                      attemptNumber: 1,
                      agentRequests: 1,
                      compactorRequests: 0,
                      requestedOutputTokens: 4096,
                      toolDispatched,
                      outcome: "blocked",
                    },
              messages: [
                ...runningSession.turns[0].messages,
                {
                  messageId: ASSISTANT_MESSAGE_ID,
                  role: "assistant",
                  content: "已提交，等待 Host 回执。",
                  createdAtMs: 2,
                },
              ],
            },
          ],
        }),
      );
      const start = vi.spyOn(conversationApi, "requestConversationTurnStart");
      render(
        <ConversationView
          projectSessionId={PROJECT_SESSION_ID}
          providerState={configuredProvider}
        />,
      );
      await screen.findByText("已提交，等待 Host 回执。");
      expect(Boolean(screen.queryByText(/Gainker 核对：本轮未执行工具/))).toBe(
        toolDispatched === false,
      );
      expect(start).not.toHaveBeenCalled();
    },
  );

  it("NR-505 keeps partial reply and independently committed EPUB fact while publication queries never start a turn", async () => {
    const text = "已经说明的非空正文。\n\n本次内容未生成完成。";
    vi.spyOn(conversationApi, "requestConversationSession").mockResolvedValue(
      runtimeView({
        ...runningSession,
        turns: [
          {
            ...runningSession.turns[0],
            status: "failed",
            updatedAtMs: 2,
            messages: [
              ...runningSession.turns[0].messages,
              {
                messageId: ASSISTANT_MESSAGE_ID,
                role: "assistant",
                content: text,
                createdAtMs: 2,
              },
            ],
          },
        ],
      }),
    );
    const start = vi.spyOn(conversationApi, "requestConversationTurnStart");
    vi.mocked(publicationApi.requestPublications).mockResolvedValue({
      schemaVersion: 1,
      nextCursor: null,
      items: [
        {
          schemaVersion: 1,
          artifactId: PROJECT_ID,
          sessionId: CONVERSATION_SESSION_ID,
          turnId: TURN_ID,
          status: "generated",
          chapterCount: 1,
          skippedDrafts: 0,
          withoutCover: true,
          relativePath: "published/" + PROJECT_ID + ".epub",
          outputHash: "a".repeat(64),
          sizeBytes: 2048,
          issue: null,
        },
      ],
    });
    vi.spyOn(publicationApi, "requestPublicationDetail").mockRejectedValue(
      new Error("synthetic unavailable"),
    );
    render(
      <ConversationView projectSessionId={PROJECT_SESSION_ID} providerState={configuredProvider} />,
    );
    await screen.findByText("EPUB 已生成（本地产物，未上架）");
    expect(screen.getByText(/已经说明的非空正文/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "查看出版依据" }));
    await screen.findByText(/以上是先前回执/);
    expect(screen.getByText(/已经说明的非空正文/)).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "对话输入" })).toBeEnabled();
    expect(start).not.toHaveBeenCalled();
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
      attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
      attemptNumber: 1,
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
        conversationSessionId: CONVERSATION_SESSION_ID,
        attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
        attemptNumber: 1,
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
        conversationSessionId: CONVERSATION_SESSION_ID,
        attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
        attemptNumber: 1,
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
          toolName: "novelist.read_context",
          toolVersion: 2,
          startedAtMs: 1,
          updatedAtMs: 2,
          errorCode: null,
          displayLabel: "读取工程资料",
          taskRevision: 2,
        },
      });
    });
    expect(screen.getByText("正在执行")).toBeInTheDocument();
    expect(screen.getByText("读取工程资料")).toBeInTheDocument();
    act(() => {
      emit?.({
        sessionId: PROJECT_SESSION_ID,
        conversationSessionId: CONVERSATION_SESSION_ID,
        attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
        attemptNumber: 1,
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
          toolName: "novelist.read_context",
          toolVersion: 2,
          startedAtMs: 1,
          updatedAtMs: 2,
          errorCode: null,
          displayLabel: "读取工程资料",
          taskRevision: 3,
        },
      });
    });
    expect(screen.getByText("执行完成")).toBeInTheDocument();
    act(() => {
      emit?.({
        sessionId: PROJECT_SESSION_ID,
        conversationSessionId: CONVERSATION_SESSION_ID,
        attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
        attemptNumber: 1,
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

  it("preserves reading position during streaming and completion, and follows again at the bottom", async () => {
    const streamingSession: conversationApi.ConversationSession = {
      ...runningSession,
      turns: [
        {
          ...runningSession.turns[0],
          retryState: {
            attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
            attemptNumber: 1,
            agentRequests: 1,
            compactorRequests: 0,
            requestedOutputTokens: 4096,
            toolDispatched: false,
            outcome: "running",
          },
        },
      ],
    };
    const load = vi
      .spyOn(conversationApi, "requestConversationSession")
      .mockResolvedValue(runtimeView(streamingSession));
    const { container } = render(
      <ConversationView projectSessionId={PROJECT_SESSION_ID} providerState={configuredProvider} />,
    );
    await screen.findByText("今天天气怎么样？");
    const canvas = container.querySelector<HTMLDivElement>(".message-canvas")!;
    let height = 1000;
    let top = 600;
    // jsdom has no layout; emulate the browser's bounded scroll position.
    Object.defineProperties(canvas, {
      clientHeight: { get: () => 400 },
      scrollHeight: { get: () => height },
      scrollTop: {
        get: () => top,
        set: (value: number) => {
          top = Math.max(0, Math.min(value, height - 400));
        },
      },
    });
    const delta = (sequence: number, text: string) =>
      act(() => {
        height += 200;
        emit?.({
          sessionId: PROJECT_SESSION_ID,
          conversationSessionId: CONVERSATION_SESSION_ID,
          attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
          attemptNumber: 1,
          turnId: TURN_ID,
          providerId: "openai",
          model: "gpt-5.6-terra",
          sequence,
          kind: "textDelta",
          delta: text,
        });
      });
    delta(1, "第一段。");
    expect(canvas.scrollTop).toBe(800);
    canvas.scrollTop = 300;
    fireEvent.scroll(canvas);
    delta(2, "第二段。");
    expect(canvas.scrollTop).toBe(300);
    canvas.scrollTop = height - 400;
    fireEvent.scroll(canvas);
    delta(3, "第三段。");
    expect(canvas.scrollTop).toBe(1200);
    canvas.scrollTop = 300;
    fireEvent.scroll(canvas);
    load.mockResolvedValue(
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
                content: "第一段。第二段。第三段。",
                createdAtMs: 2,
              },
            ],
          },
        ],
      }),
    );
    act(() =>
      emit?.({
        sessionId: PROJECT_SESSION_ID,
        conversationSessionId: CONVERSATION_SESSION_ID,
        attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
        attemptNumber: 1,
        turnId: TURN_ID,
        providerId: "openai",
        model: "gpt-5.6-terra",
        sequence: 4,
        kind: "completed",
        finishReason: "stop",
      }),
    );
    await screen.findByText("已完成");
    expect(canvas.scrollTop).toBe(300);
  });

  it.each([
    ["failed", "本次回复未生成。"],
    ["failed", "保留非空内容。\n\n本次内容未生成完成。"],
    ["completed", "工具参数无效，但仍可继续讨论。"],
  ] as const)(
    "NR-506 keeps %s text and allows composing after a failed tool: %s",
    async (status, text) => {
      const snapshot = runtimeView({
        ...runningSession,
        turns: [
          {
            ...runningSession.turns[0],
            status,
            updatedAtMs: 2,
            messages: [
              ...runningSession.turns[0].messages,
              { messageId: ASSISTANT_MESSAGE_ID, role: "assistant", content: text, createdAtMs: 2 },
            ],
          },
        ],
      });
      snapshot.taskActivities = [
        {
          taskId: PROJECT_ID,
          sessionId: CONVERSATION_SESSION_ID,
          turnId: TURN_ID,
          status: "failed",
          toolName: "novelist.read_context",
          toolVersion: 2,
          startedAtMs: 1,
          updatedAtMs: 2,
          errorCode: "agent.tool_arguments_invalid",
          displayLabel: "读取小说上下文",
          taskRevision: 3,
        },
      ];
      vi.spyOn(conversationApi, "requestConversationSession").mockResolvedValue(snapshot);
      const start = vi.spyOn(conversationApi, "requestConversationTurnStart");
      render(
        <ConversationView
          projectSessionId={PROJECT_SESSION_ID}
          providerState={configuredProvider}
        />,
      );
      await screen.findByText(text, { exact: false, normalizer: (value) => value });
      const input = screen.getByRole("textbox", { name: "对话输入" });
      await userEvent.type(input, "继续讨论");
      expect(input).toHaveValue("继续讨论");
      expect(screen.getByRole("button", { name: "发送" })).toBeEnabled();
      expect(start).not.toHaveBeenCalled();
      expect(screen.getByText("读取小说上下文")).toBeInTheDocument();
      expect(screen.getByText("执行失败")).toBeInTheDocument();
    },
  );

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
      attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
      attemptNumber: 1,
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
    expect(cancel).toHaveBeenCalledWith(
      PROJECT_SESSION_ID,
      TURN_ID,
      undefined,
      "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
      CONVERSATION_SESSION_ID,
    );
    expect(screen.getByRole("button", { name: "正在停止" })).toBeDisabled();

    act(() => {
      emit?.({
        sessionId: PROJECT_SESSION_ID,
        conversationSessionId: CONVERSATION_SESSION_ID,
        attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
        attemptNumber: 1,
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
