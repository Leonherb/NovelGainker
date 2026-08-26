import { describe, expect, it } from "vitest";
import {
  applyProviderRunEvent,
  beginConversationRun,
  EMPTY_CONVERSATION_STATE,
  toProviderChatMessages,
} from "./conversation-state";
import type { ProviderRunEvent, ProviderRunStarted } from "../lib/tauri/provider-chat";

const STARTED: ProviderRunStarted = {
  runId: "10000000-0000-4000-8000-000000000001",
  providerId: "openai",
  model: "gpt-5.6-terra",
};

function event(value: {
  kind: string;
  sequence: number;
  [key: string]: unknown;
}): ProviderRunEvent {
  return { ...STARTED, ...value } as ProviderRunEvent;
}

describe("conversation run state", () => {
  it("merges monotonic deltas and ignores duplicate, late, or wrong-identity events", () => {
    let state = beginConversationRun(EMPTY_CONVERSATION_STATE, STARTED, "写一个开场");
    state = applyProviderRunEvent(
      state,
      event({ kind: "textDelta", sequence: 1, delta: "第一段" }),
    );
    const afterFirst = state;
    state = applyProviderRunEvent(state, event({ kind: "textDelta", sequence: 1, delta: "重复" }));
    expect(state).toBe(afterFirst);
    state = applyProviderRunEvent(state, {
      ...STARTED,
      providerId: "deepseek",
      model: "deepseek-v4-pro",
      kind: "textDelta",
      sequence: 2,
      delta: "错误 Provider",
    });
    expect(state).toBe(afterFirst);
    state = applyProviderRunEvent(state, event({ kind: "textDelta", sequence: 3, delta: "继续" }));
    state = applyProviderRunEvent(state, event({ kind: "textDelta", sequence: 2, delta: "迟到" }));
    expect(state.messages[1].content).toBe("第一段继续");
    expect(state.activeRun?.lastSequence).toBe(3);
  });

  it("preserves partial text and typed error when a run fails", () => {
    let state = beginConversationRun(EMPTY_CONVERSATION_STATE, STARTED, "继续写");
    state = applyProviderRunEvent(
      state,
      event({ kind: "textDelta", sequence: 1, delta: "保留的半段" }),
    );
    state = applyProviderRunEvent(
      state,
      event({
        kind: "failed",
        sequence: 2,
        error: { code: "provider.timeout", messageKey: "errors.providerTimeout" },
      }),
    );
    expect(state.activeRun).toBeNull();
    expect(state.messages[1]).toMatchObject({
      content: "保留的半段",
      status: "failed",
      error: { code: "provider.timeout" },
    });
  });

  it("records usage when supplied and represents missing usage by absence", () => {
    let withUsage = beginConversationRun(EMPTY_CONVERSATION_STATE, STARTED, "问题");
    withUsage = applyProviderRunEvent(
      withUsage,
      event({
        kind: "usage",
        sequence: 1,
        usage: { inputTokens: 8, outputTokens: 5, totalTokens: 13 },
      }),
    );
    withUsage = applyProviderRunEvent(
      withUsage,
      event({ kind: "completed", sequence: 2, finishReason: "stop" }),
    );
    expect(withUsage.messages[1].usage?.totalTokens).toBe(13);

    let withoutUsage = beginConversationRun(EMPTY_CONVERSATION_STATE, STARTED, "另一个问题");
    withoutUsage = applyProviderRunEvent(
      withoutUsage,
      event({ kind: "completed", sequence: 1, finishReason: "length" }),
    );
    expect(withoutUsage.messages[1].usage).toBeUndefined();
    expect(withoutUsage.messages[1].finishReason).toBe("length");
  });

  it("keeps conversation history in memory as role/content only", () => {
    let state = beginConversationRun(EMPTY_CONVERSATION_STATE, STARTED, "用户内容");
    state = applyProviderRunEvent(
      state,
      event({ kind: "textDelta", sequence: 1, delta: "助手内容" }),
    );
    state = applyProviderRunEvent(state, event({ kind: "cancelled", sequence: 2 }));
    expect(toProviderChatMessages(state.messages)).toEqual([
      { role: "user", content: "用户内容" },
      { role: "assistant", content: "助手内容" },
    ]);
  });
});
