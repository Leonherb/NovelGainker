import { describe, expect, it, vi } from "vitest";
import {
  AGENT_SESSION_LOAD_COMMAND,
  AGENT_TURN_EVENT_CHANNEL,
  listenAgentTurnEvents,
  parseAgentSession,
  parseAgentTurnEvent,
  requestAgentSession,
} from "./agent";

const run = {
  runId: "10000000-0000-4000-8000-000000000001",
  turnId: "trn_10000000000040008000000000000001",
  agentSessionId: "ses_10000000000040008000000000000001",
  providerId: "openai",
  model: "gpt-5.6-terra",
  sequence: 1,
} as const;

describe("AM2 Agent Tauri boundary", () => {
  it("accepts bounded metadata sessions with policy, pending previews and no full chat", () => {
    const session = parseAgentSession({
      schemaVersion: 1,
      projectSessionId: "desktop-session",
      projectId: "10000000-0000-4000-8000-000000000001",
      agentSessionId: run.agentSessionId,
      providerId: "openai",
      model: "gpt-5.6-terra",
      packVersion: "1.0.0",
      packHash: "a".repeat(64),
      summaryText: "上次结论",
      summaryVersion: 1,
      policy: "review_every_write",
      recentTurns: [],
      undoRecords: [],
      pendingWrites: [],
      fullChatPersisted: false,
      projectWriteCount: 0,
    });
    expect(session.fullChatPersisted).toBe(false);
    expect(() => parseAgentSession({ ...session, fullChatPersisted: true })).toThrow();
    expect(() => parseAgentSession({ ...session, transcript: [] })).toThrow();
  });

  it("accepts visible read traces and bounded committed write counts", () => {
    const event = parseAgentTurnEvent({
      ...run,
      kind: "toolCompleted",
      callId: "call_10000000000040008000000000000001",
      toolId: "project.read_markdown",
      status: "succeeded",
      sourcePaths: ["characters/hero.md"],
      truncated: false,
      projectWriteCount: 0,
    });
    expect(event.kind).toBe("toolCompleted");
    expect(() => parseAgentTurnEvent({ ...event, projectWriteCount: 1 })).toThrow();
    expect(() => parseAgentTurnEvent({ ...event, toolId: "project.patch_markdown" })).toThrow();
    const completed = parseAgentTurnEvent({
      ...run,
      kind: "completed",
      summary: "已写入",
      usage: null,
      projectWriteCount: 2,
    });
    expect(completed.kind === "completed" && completed.projectWriteCount).toBe(2);
  });

  it("invokes the project-bound session command and drops malformed events", async () => {
    const invoke = vi.fn().mockResolvedValue({
      schemaVersion: 1,
      projectSessionId: "desktop-session",
      projectId: "10000000-0000-4000-8000-000000000001",
      agentSessionId: run.agentSessionId,
      providerId: "openai",
      model: "gpt-5.6-terra",
      packVersion: "1.0.0",
      packHash: "a".repeat(64),
      summaryText: null,
      summaryVersion: 0,
      policy: "auto_apply_safe",
      recentTurns: [],
      undoRecords: [],
      pendingWrites: [],
      fullChatPersisted: false,
      projectWriteCount: 0,
    });
    await requestAgentSession("desktop-session", invoke);
    expect(invoke).toHaveBeenCalledWith(AGENT_SESSION_LOAD_COMMAND, {
      projectSessionId: "desktop-session",
    });

    let listener: ((event: { payload: unknown }) => void) | undefined;
    const listen = vi.fn(async (_channel, handler) => {
      listener = handler;
      return () => undefined;
    });
    const received = vi.fn();
    await listenAgentTurnEvents(received, listen);
    expect(listen).toHaveBeenCalledWith(AGENT_TURN_EVENT_CHANNEL, expect.any(Function));
    listener?.({ payload: { ...run, kind: "cancelled", projectWriteCount: 9 } });
    expect(received).not.toHaveBeenCalled();
  });
});
