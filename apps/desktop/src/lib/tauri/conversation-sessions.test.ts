import { describe, expect, it, vi } from "vitest";
import {
  parseSessionPage,
  requestSessionCreate,
  requestSessions,
  type SessionPage,
} from "./conversation-sessions";
import { parseConversationTurnEvent } from "./conversation";
const id = "10000000-0000-4000-8000-000000000001";
const other = "20000000-0000-4000-8000-000000000002";
const page: SessionPage = {
  schemaVersion: 1,
  revision: 1,
  activeSessionId: id,
  defaultSessionId: id,
  items: [
    {
      sessionId: id,
      name: "默认会话",
      archived: false,
      createdAtMs: 1,
      updatedAtMs: 1,
      creationRequestId: null,
    },
  ],
  nonArchivedCount: 1,
  canMutate: true,
  totalCount: 1,
  nextCursor: null,
  activeRun: null,
};
describe("NR-601 session IPC", () => {
  it("requires every nullable field and forbids hidden payloads", () => {
    expect(parseSessionPage(page)).toEqual(page);
    for (const key of ["activeRun", "nextCursor", "activeSessionId"]) {
      const invalid = { ...page } as Record<string, unknown>;
      delete invalid[key];
      expect(() => parseSessionPage(invalid)).toThrow();
    }
    expect(() => parseSessionPage({ ...page, hiddenPrompt: "secret" })).toThrow();
    expect(() =>
      parseSessionPage({ ...page, activeRun: { sessionId: id, reservationId: other } }),
    ).toThrow();
  });
  it("enforces catalog, name, revision and unique-ID bounds", () => {
    for (const name of ["", "  名称", "甲".repeat(61), String.fromCharCode(0), "a".repeat(181)]) {
      expect(() => parseSessionPage({ ...page, items: [{ ...page.items[0], name }] })).toThrow();
    }
    for (const update of [
      { items: [page.items[0], page.items[0]] },
      { nonArchivedCount: 33 },
      { totalCount: 257 },
      { revision: 0 },
      { revision: Number.MAX_SAFE_INTEGER + 1 },
      { nextCursor: "x".repeat(1025) },
    ])
      expect(() => parseSessionPage({ ...page, ...update })).toThrow();
  });
  it("sends only scoped local requests and keeps a stable creation request ID", async () => {
    const invoke = vi.fn().mockResolvedValue(page);
    await requestSessions(id, false, null, invoke);
    await requestSessionCreate(id, other, "新会话", 1, invoke);
    expect(invoke.mock.calls).toEqual([
      [
        "conversation_session_list",
        { request: { projectSessionId: id, archived: false, cursor: null } },
      ],
      [
        "conversation_session_create",
        {
          request: { projectSessionId: id, requestId: other, name: "新会话", expectedRevision: 1 },
        },
      ],
    ]);
    await expect(
      requestSessions(
        id,
        false,
        null,
        vi.fn().mockResolvedValue({ ...page, rawProtocol: "hidden" }),
      ),
    ).rejects.toMatchObject({ code: "contract.invalid_response" });
  });
  it("requires explicit event ownership and rejects a task belonging to another conversation", () => {
    const event = {
      attemptId: id,
      attemptNumber: 1,
      sessionId: id,
      conversationSessionId: other,
      turnId: id,
      providerId: "deepseek",
      model: "deepseek-v4-pro",
      sequence: 1,
      kind: "cancelled",
    };
    expect(parseConversationTurnEvent(event).conversationSessionId).toBe(other);
    const missing = { ...event } as Record<string, unknown>;
    delete missing.conversationSessionId;
    expect(() => parseConversationTurnEvent(missing)).toThrow();
    expect(() =>
      parseConversationTurnEvent({
        ...event,
        kind: "taskActivity",
        taskActivity: {
          taskId: other,
          sessionId: id,
          turnId: id,
          status: "running",
          displayLabel: "正在处理",
          taskRevision: 1,
        },
      }),
    ).toThrow();
  });
});
