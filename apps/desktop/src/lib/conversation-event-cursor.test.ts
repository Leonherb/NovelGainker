import { describe, expect, it } from "vitest";
import { ConversationEventCursor } from "./conversation-event-cursor";
import {
  parseConversationSession,
  parseConversationTurnEvent,
  type ConversationTurnStarted,
  type ConversationTurnEvent,
} from "./tauri/conversation";
const id = "10000000-0000-4000-8000-000000000001";
const second = "20000000-0000-4000-8000-000000000002";
const first: ConversationTurnStarted = {
  sessionId: id,
  conversationSessionId: id,
  turnId: id,
  providerId: "deepseek",
  model: "deepseek-v4-pro",
  attemptId: id,
  attemptNumber: 1,
};
const retry: ConversationTurnStarted = { ...first, attemptId: second, attemptNumber: 2 };
const delta: ConversationTurnEvent = { ...first, kind: "textDelta", sequence: 1, delta: "正文" };
function snapshot(attemptNumber: 1 | 2, running: boolean) {
  return parseConversationSession({
    schemaVersion: 1,
    projectId: id,
    sessionId: id,
    turns: [
      {
        turnId: id,
        providerId: "deepseek",
        model: "deepseek-v4-pro",
        startedAtMs: 1,
        updatedAtMs: 2,
        status: running ? "running" : "not_generated",
        errorCode: "provider.unreachable",
        messages: [
          { messageId: id, role: "user", content: "检查", createdAtMs: 1 },
          ...(!running
            ? [
                {
                  messageId: second,
                  role: "assistant",
                  content: "本次回复未生成。",
                  createdAtMs: 2,
                },
              ]
            : []),
        ],
        retryState: {
          attemptId: attemptNumber === 1 ? id : second,
          attemptNumber,
          agentRequests: attemptNumber,
          compactorRequests: 0,
          requestedOutputTokens: 4096 * attemptNumber,
          toolDispatched: false,
          outcome: running ? "running" : "blocked",
        },
      },
    ],
  });
}
describe("NR-603 local event cursor", () => {
  it("deduplicates sequences and rejects wrong or retired attempt identities", () => {
    const cursor = new ConversationEventCursor();
    cursor.expect(first);
    expect(cursor.accept(delta)).toBe(true);
    expect(cursor.accept(delta)).toBe(false);
    expect(cursor.accept({ ...delta, attemptId: second, sequence: 2 })).toBe(false);
    cursor.expect(retry);
    expect(cursor.accept({ ...delta, sequence: 99 })).toBe(false);
    expect(cursor.accept({ ...delta, ...retry })).toBe(true);
    expect(cursor.isOlder(snapshot(1, false))).toBe(true);
    cursor.observe(snapshot(1, false));
    expect(cursor.accept({ ...delta, ...retry, sequence: 2 })).toBe(true);
  });
  it("reopen terminal truth rejects late text and never implicitly authorizes retry", () => {
    const cursor = new ConversationEventCursor();
    cursor.observe(snapshot(1, false));
    expect(cursor.accept(delta)).toBe(false);
    expect(cursor.accept({ ...delta, ...retry })).toBe(false);
    cursor.observe(snapshot(2, true));
    expect(cursor.accept({ ...delta, ...retry })).toBe(true);
    expect(cursor.accept({ ...retry, kind: "completed", sequence: 2, finishReason: "stop" })).toBe(
      true,
    );
    expect(cursor.isOlder(snapshot(2, true))).toBe(true);
    expect(cursor.accept({ ...delta, ...retry, sequence: 3 })).toBe(false);
  });
  it("requires closed and bounded attempt/event metadata", () => {
    const valid = snapshot(1, true);
    for (const change of [
      { agentRequests: 9 },
      { compactorRequests: 2 },
      { attemptNumber: 3 },
      { continuation: "hidden" },
      { requestedOutputTokens: 33793 },
    ]) {
      expect(() =>
        parseConversationSession({
          ...valid,
          turns: [{ ...valid.turns[0], retryState: { ...valid.turns[0].retryState, ...change } }],
        }),
      ).toThrow();
    }
    for (const change of [
      { attemptId: "invalid" },
      { attemptNumber: 3 },
      { sequence: 0 },
      { providerCursor: "opaque" },
    ]) {
      expect(() => parseConversationTurnEvent({ ...delta, ...change })).toThrow();
    }
  });
});
