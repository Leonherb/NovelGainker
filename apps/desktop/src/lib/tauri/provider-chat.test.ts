import { describe, expect, it, vi } from "vitest";
import {
  PROVIDER_CHAT_CANCEL_COMMAND,
  PROVIDER_CHAT_START_COMMAND,
  PROVIDER_RUN_EVENT_CHANNEL,
  listenProviderRunEvents,
  parseProviderRunCancelResult,
  parseProviderRunEvent,
  parseProviderRunStarted,
  requestProviderChatCancel,
  requestProviderChatStart,
} from "./provider-chat";

const RUN_ID = "10000000-0000-4000-8000-000000000001";
const IDENTITY = {
  runId: RUN_ID,
  providerId: "openai" as const,
  model: "gpt-5.6-terra",
};

describe("Provider chat IPC contract", () => {
  it("accepts only the fixed Provider/model run identity", () => {
    expect(parseProviderRunStarted(IDENTITY)).toEqual(IDENTITY);
    expect(() => parseProviderRunStarted({ ...IDENTITY, model: "gpt-4o" })).toThrow(
      "Invalid Provider run start contract",
    );
    expect(() => parseProviderRunStarted({ ...IDENTITY, projectPath: "D:/novel" })).toThrow(
      "Invalid Provider run start contract",
    );
  });

  it("strictly parses every safe run event and rejects extras", () => {
    expect(
      parseProviderRunEvent({
        ...IDENTITY,
        sequence: 1,
        kind: "textDelta",
        delta: "你好",
      }),
    ).toMatchObject({ kind: "textDelta", delta: "你好" });
    expect(
      parseProviderRunEvent({
        ...IDENTITY,
        sequence: 2,
        kind: "usage",
        usage: { inputTokens: 4, outputTokens: 2, totalTokens: 6 },
      }),
    ).toMatchObject({ kind: "usage", usage: { totalTokens: 6 } });
    expect(
      parseProviderRunEvent({
        ...IDENTITY,
        sequence: 3,
        kind: "completed",
        finishReason: "stop",
      }),
    ).toMatchObject({ kind: "completed", finishReason: "stop" });
    expect(
      parseProviderRunEvent({
        ...IDENTITY,
        sequence: 4,
        kind: "failed",
        error: { code: "provider.timeout", messageKey: "errors.providerTimeout" },
      }),
    ).toMatchObject({ kind: "failed", error: { code: "provider.timeout" } });
    expect(parseProviderRunEvent({ ...IDENTITY, sequence: 5, kind: "cancelled" })).toMatchObject({
      kind: "cancelled",
    });

    expect(() =>
      parseProviderRunEvent({
        ...IDENTITY,
        sequence: 0,
        kind: "cancelled",
      }),
    ).toThrow();
    expect(() =>
      parseProviderRunEvent({
        ...IDENTITY,
        sequence: 6,
        kind: "cancelled",
        apiKey: "secret",
      }),
    ).toThrow();
  });

  it("sends only bounded in-memory role/content messages", async () => {
    const invokeCommand = vi.fn().mockResolvedValue(IDENTITY);
    const messages = [{ role: "user" as const, content: "写一个开场" }];

    await expect(requestProviderChatStart("openai", messages, invokeCommand)).resolves.toEqual(
      IDENTITY,
    );
    expect(invokeCommand).toHaveBeenCalledWith(PROVIDER_CHAT_START_COMMAND, {
      expectedProviderId: "openai",
      messages,
    });
    const serialized = JSON.stringify(invokeCommand.mock.calls[0]);
    expect(serialized).not.toContain("project");
    expect(serialized).not.toContain("filePath");
    expect(serialized).not.toContain("selection");
    expect(serialized).not.toContain("apiKey");
  });

  it("uses a strict cancellation response", async () => {
    const invokeCommand = vi.fn().mockResolvedValue({ cancelRequested: true });
    await expect(requestProviderChatCancel(RUN_ID, invokeCommand)).resolves.toEqual({
      cancelRequested: true,
    });
    expect(invokeCommand).toHaveBeenCalledWith(PROVIDER_CHAT_CANCEL_COMMAND, { runId: RUN_ID });
    expect(parseProviderRunCancelResult({ cancelRequested: true })).toEqual({
      cancelRequested: true,
    });
    expect(() => parseProviderRunCancelResult({ cancelRequested: true, extra: 1 })).toThrow();
  });

  it("drops malformed native events before they reach conversation state", async () => {
    const received = vi.fn();
    let nativeHandler: ((event: { payload: unknown }) => void) | undefined;
    const unlisten = vi.fn();
    const listenEvent = vi.fn(async (_channel, handler) => {
      nativeHandler = handler;
      return unlisten;
    });

    await expect(listenProviderRunEvents(received, listenEvent)).resolves.toBe(unlisten);
    expect(listenEvent).toHaveBeenCalledWith(PROVIDER_RUN_EVENT_CHANNEL, expect.any(Function));
    nativeHandler?.({ payload: { ...IDENTITY, sequence: 1, kind: "cancelled" } });
    nativeHandler?.({
      payload: { ...IDENTITY, sequence: 2, kind: "cancelled", rawResponse: "secret" },
    });
    expect(received).toHaveBeenCalledOnce();
  });
});
