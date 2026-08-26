import { describe, expect, it, vi } from "vitest";
import {
  listenRouterRunEvents,
  parseLocalRouterDecision,
  parseRouterRunEvent,
  requestLocalRouterDecision,
  requestRouterRunCancel,
  requestRouterRunStart,
} from "./router";

const identity = {
  runId: "10000000-0000-4000-8000-000000000001",
  providerId: "deepseek" as const,
  model: "deepseek-v4-pro",
};

describe("NG-406 Router IPC contract", () => {
  it("accepts exact local outcomes and rejects forged workflow fields", () => {
    expect(
      parseLocalRouterDecision({
        schemaVersion: 1,
        outcome: "direct",
        workflowId: "novelgainker.character-create",
        entryMode: "character-enhance",
      }),
    ).toMatchObject({ outcome: "direct", entryMode: "character-enhance" });
    expect(() =>
      parseLocalRouterDecision({
        schemaVersion: 1,
        outcome: "needsProvider",
        workflowId: "novelgainker.character-create",
        entryMode: null,
      }),
    ).toThrow();
    expect(() =>
      parseLocalRouterDecision({
        schemaVersion: 1,
        outcome: "direct",
        workflowId: "novelgainker.chapter-create",
        entryMode: "default",
      }),
    ).toThrow();
  });

  it("strictly parses terminal events without original text", () => {
    const event = parseRouterRunEvent({
      ...identity,
      kind: "completed",
      decision: {
        schemaVersion: 1,
        workflowId: "novelgainker.plot-create",
        confidence: "medium",
      },
      usage: { inputTokens: 12, outputTokens: 3, totalTokens: 15 },
    });
    expect(event.kind).toBe("completed");
    expect(JSON.stringify(event)).not.toContain("ROUTER_PRIVATE_CANARY");
    expect(() => parseRouterRunEvent({ ...event, originalText: "forbidden" })).toThrow();
    expect(() =>
      parseRouterRunEvent({
        ...identity,
        kind: "completed",
        decision: {
          schemaVersion: 1,
          workflowId: "novelgainker.plot-create",
          confidence: "certain",
        },
        usage: null,
      }),
    ).toThrow();
  });

  it("invokes only bounded Router arguments and never a project session", async () => {
    const localInvoke = vi.fn().mockResolvedValue({
      schemaVersion: 1,
      outcome: "needsProvider",
      workflowId: null,
      entryMode: null,
    });
    await requestLocalRouterDecision("想处理一个人物", localInvoke);
    expect(localInvoke).toHaveBeenCalledWith("router_resolve_local", { text: "想处理一个人物" });

    const startInvoke = vi.fn().mockResolvedValue(identity);
    await requestRouterRunStart("deepseek", "想处理一个人物", startInvoke);
    expect(startInvoke).toHaveBeenCalledWith("router_run_start", {
      expectedProviderId: "deepseek",
      text: "想处理一个人物",
    });
    expect(JSON.stringify(startInvoke.mock.calls)).not.toMatch(
      /session|project|path|history|key/iu,
    );

    const cancelInvoke = vi.fn().mockResolvedValue({ cancelRequested: true });
    await requestRouterRunCancel(identity.runId, cancelInvoke);
    expect(cancelInvoke).toHaveBeenCalledWith("router_run_cancel", { runId: identity.runId });
  });

  it("drops malformed events at the listener boundary", async () => {
    const onEvent = vi.fn();
    const unlisten = vi.fn();
    const listen = vi.fn(async (_channel, handler) => {
      handler({ payload: { ...identity, kind: "completed", decision: {}, usage: null } });
      handler({
        payload: {
          ...identity,
          kind: "cancelled",
        },
      });
      return unlisten;
    });
    await listenRouterRunEvents(onEvent, listen);
    expect(listen.mock.calls[0]?.[0]).toBe("router-run-event");
    expect(onEvent).toHaveBeenCalledOnce();
    expect(onEvent.mock.calls[0]?.[0]).toMatchObject({ kind: "cancelled" });
  });
});
