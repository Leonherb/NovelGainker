import { describe, expect, it, vi } from "vitest";
import {
  CONVERSATION_SESSION_LOAD_COMMAND,
  CONVERSATION_TURN_CANCEL_COMMAND,
  CONVERSATION_TURN_EVENT_CHANNEL,
  CONVERSATION_TURN_START_COMMAND,
  listenConversationTurnEvents,
  parseConversationRuntimeView,
  parseConversationSession,
  parseConversationTurnEvent,
  parseConversationTurnStarted,
  parseTaskActivity,
  requestConversationSession,
  requestConversationTurnCancel,
  requestConversationTurnStart,
} from "./conversation";

const PROJECT_SESSION_ID = "10000000-0000-4000-8000-000000000001";
const CONVERSATION_SESSION_ID = "20000000-0000-4000-8000-000000000002";
const PROJECT_ID = "30000000-0000-4000-8000-000000000003";
const TURN_ID = "40000000-0000-4000-8000-000000000004";

it("requires bounded Host tool evidence and rejects raw payloads and inconsistent states", () => {
  const task = {
    taskId: "70000000-0000-4000-8000-000000000007",
    sessionId: CONVERSATION_SESSION_ID,
    turnId: TURN_ID,
    status: "failed",
    displayLabel: "维护小说资料",
    taskRevision: 2,
    toolName: "novelist.propose_change",
    toolVersion: 2,
    startedAtMs: 1,
    updatedAtMs: 2,
    errorCode: "agent.tool_arguments_invalid",
  };
  expect(parseTaskActivity(task)).toEqual(task);
  for (const patch of [
    { arguments: "private manuscript" },
    { errorCode: null },
    { updatedAtMs: 0 },
    { toolName: "C:/private" },
    { status: "succeeded" },
    { toolVersion: 0 },
  ])
    expect(() => parseTaskActivity({ ...task, ...patch })).toThrow();
  const missing = { ...task } as Record<string, unknown>;
  delete missing.toolName;
  expect(() => parseTaskActivity(missing)).toThrow();
});
const MESSAGE_ID = "50000000-0000-4000-8000-000000000005";
const IDENTITY = {
  sessionId: PROJECT_SESSION_ID,
  conversationSessionId: CONVERSATION_SESSION_ID,
  attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
  attemptNumber: 1,
  turnId: TURN_ID,
  providerId: "openai" as const,
  model: "gpt-5.6-terra",
};

describe("Conversation IPC contract", () => {
  it("strictly parses a complete persisted session", () => {
    const session = {
      schemaVersion: 1,
      projectId: PROJECT_ID,
      sessionId: CONVERSATION_SESSION_ID,
      turns: [
        {
          turnId: TURN_ID,
          providerId: "openai",
          model: "gpt-5.6-terra",
          status: "completed",
          startedAtMs: 1,
          updatedAtMs: 2,
          messages: [
            { messageId: MESSAGE_ID, role: "user", content: "你好", createdAtMs: 1 },
            {
              messageId: "60000000-0000-4000-8000-000000000006",
              role: "assistant",
              content: "你好。",
              createdAtMs: 2,
            },
          ],
        },
      ],
    };
    expect(parseConversationSession(session)).toEqual(session);
    expect(() => parseConversationSession({ ...session, control: {} })).toThrow();
  });

  it("keeps project and Conversation session identities separate", () => {
    const started = {
      ...IDENTITY,
      conversationSessionId: CONVERSATION_SESSION_ID,
      attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
      attemptNumber: 1,
    };
    expect(parseConversationTurnStarted(started)).toEqual(started);
    expect(() => parseConversationTurnStarted({ ...started, apiKey: "secret" })).toThrow();
  });

  it("strictly parses the independent Runtime View and Task Activity", () => {
    const conversation = {
      schemaVersion: 1 as const,
      projectId: PROJECT_ID,
      sessionId: CONVERSATION_SESSION_ID,
      turns: [],
    };
    expect(
      parseConversationRuntimeView({
        schemaVersion: 1,
        conversation,
        taskActivities: [],
      }),
    ).toEqual({ schemaVersion: 1, conversation, taskActivities: [] });
    expect(() =>
      parseConversationRuntimeView({
        schemaVersion: 1,
        conversation,
        taskActivities: [],
        rawToolResult: {},
      }),
    ).toThrow();
  });

  it("strictly parses natural text, usage and unique terminal events", () => {
    expect(
      parseConversationTurnEvent({ ...IDENTITY, sequence: 1, kind: "textDelta", delta: "你好" }),
    ).toMatchObject({ kind: "textDelta", delta: "你好" });
    expect(
      parseConversationTurnEvent({
        ...IDENTITY,
        sequence: 2,
        kind: "usage",
        usage: { inputTokens: 2, outputTokens: 1, totalTokens: 3 },
      }),
    ).toMatchObject({ kind: "usage" });
    expect(
      parseConversationTurnEvent({
        ...IDENTITY,
        sequence: 3,
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
          displayLabel: "处理相关内容",
          taskRevision: 2,
        },
      }),
    ).toMatchObject({ kind: "taskActivity" });
    expect(
      parseConversationTurnEvent({
        ...IDENTITY,
        sequence: 3,
        kind: "completed",
        finishReason: "stop",
      }),
    ).toMatchObject({ kind: "completed" });
    expect(
      parseConversationTurnEvent({
        ...IDENTITY,
        sequence: 4,
        kind: "failed",
        error: { code: "provider.timeout", messageKey: "errors.providerTimeout" },
      }),
    ).toMatchObject({ kind: "failed" });
    expect(
      parseConversationTurnEvent({ ...IDENTITY, sequence: 5, kind: "cancelled" }),
    ).toMatchObject({ kind: "cancelled" });
    expect(() =>
      parseConversationTurnEvent({
        ...IDENTITY,
        sequence: 6,
        kind: "cancelled",
        rawResponse: "secret",
      }),
    ).toThrow();
  });

  it("invokes only the three approved commands with visible text and opaque IDs", async () => {
    const session = {
      schemaVersion: 1,
      projectId: PROJECT_ID,
      sessionId: CONVERSATION_SESSION_ID,
      turns: [],
    };
    const started = { ...IDENTITY, conversationSessionId: CONVERSATION_SESSION_ID };
    const invokeCommand = vi
      .fn()
      .mockResolvedValueOnce({
        schemaVersion: 1,
        conversation: session,
        taskActivities: [],
      })
      .mockResolvedValueOnce(started)
      .mockResolvedValueOnce({ cancelRequested: true });
    await requestConversationSession(PROJECT_SESSION_ID, CONVERSATION_SESSION_ID, invokeCommand);
    await requestConversationTurnStart(
      PROJECT_SESSION_ID,
      "随便聊聊",
      CONVERSATION_SESSION_ID,
      invokeCommand,
    );
    await requestConversationTurnCancel(PROJECT_SESSION_ID, TURN_ID, invokeCommand);
    expect(invokeCommand.mock.calls).toEqual([
      [
        CONVERSATION_SESSION_LOAD_COMMAND,
        { sessionId: PROJECT_SESSION_ID, conversationSessionId: CONVERSATION_SESSION_ID },
      ],
      [
        CONVERSATION_TURN_START_COMMAND,
        {
          sessionId: PROJECT_SESSION_ID,
          message: "随便聊聊",
          conversationSessionId: CONVERSATION_SESSION_ID,
        },
      ],
      [CONVERSATION_TURN_CANCEL_COMMAND, { sessionId: PROJECT_SESSION_ID, turnId: TURN_ID }],
    ]);
    expect(JSON.stringify(invokeCommand.mock.calls)).not.toMatch(
      /apiKey|skill|control|anchor|canon|tool/iu,
    );
  });

  it("drops malformed native events before UI state", async () => {
    const received = vi.fn();
    let nativeHandler: ((event: { payload: unknown }) => void) | undefined;
    const unlisten = vi.fn();
    const listenEvent = vi.fn(async (_channel, handler) => {
      nativeHandler = handler;
      return unlisten;
    });
    await expect(listenConversationTurnEvents(received, listenEvent)).resolves.toBe(unlisten);
    expect(listenEvent).toHaveBeenCalledWith(CONVERSATION_TURN_EVENT_CHANNEL, expect.any(Function));
    nativeHandler?.({ payload: { ...IDENTITY, sequence: 1, kind: "cancelled" } });
    nativeHandler?.({
      payload: { ...IDENTITY, sequence: 2, kind: "cancelled", hiddenPrompt: "secret" },
    });
    expect(received).toHaveBeenCalledOnce();
  });
});
