import { describe, expect, it, vi } from "vitest";
import {
  AGENT_SESSION_LOAD_COMMAND,
  AGENT_TURN_EVENT_CHANNEL,
  AGENT_V2_COMMANDS,
  AGENT_WRITE_CONFIRMATION_SUBMIT_COMMAND,
  listenAgentTurnEvents,
  parseAgentSession,
  parseAgentTurnEvent,
  requestAgentSession,
  requestAgentWriteConfirmation,
  type AgentWritePlanV2,
} from "./agent";

const session = {
  schemaVersion: 2,
  projectId: "10000000-0000-4000-8000-000000000001",
  historyConsentRequired: false,
  branchSelectionRequired: false,
  activeBranchId: "brn_10000000000040008000000000000001",
  sessionId: "ses_10000000000040008000000000000001",
  branches: [
    {
      branchId: "brn_10000000000040008000000000000001",
      sessionId: "ses_10000000000040008000000000000001",
      lastTurnAtMs: 10,
      messageCount: 0,
      hashStatus: "empty",
    },
  ],
  messages: [],
  memory: null,
  draftQuota: 0,
  writePolicy: "auto_apply_safe",
  writePolicyHash: "a".repeat(64),
  contextMarks: {
    schemaVersion: 1,
    projectId: "10000000-0000-4000-8000-000000000001",
    marks: [],
    storeHash: "b".repeat(64),
  },
} as const;

const baseEvent = {
  schemaVersion: 2,
  runId: "run_10000000000040008000000000000001",
  turnId: "trn_10000000000040008000000000000001",
  projectSessionId: "desktop-session",
  providerId: "openai",
  model: "gpt-5.6-terra",
  sequence: 1,
} as const;

const usage = {
  providerCalls: 3,
  totalProviderInputBytes: 100,
  totalProviderOutputBytes: 40,
  accumulatedContextResultBytes: 20,
  structuredRepairCalls: 0,
  totalProviderInputTokens: 25,
  totalProviderOutputTokens: 10,
};

describe("GAR2 Agent Protocol V2 Tauri boundary", () => {
  it("accepts project-local visible history and rejects legacy metadata fields", () => {
    expect(parseAgentSession(session).draftQuota).toBe(0);
    expect(() => parseAgentSession({ ...session, fullChatPersisted: false })).toThrow();
    expect(() => parseAgentSession({ ...session, draftQuota: 1 })).toThrow();
    expect(() => parseAgentSession({ ...session, writePolicyHash: "stale" })).toThrow();
  });

  it("keeps Conversation deltas separate and accepts bounded Canon-write terminal counts", () => {
    const delta = parseAgentTurnEvent({ ...baseEvent, kind: "conversationDelta", delta: "潮声" });
    expect(delta.kind).toBe("conversationDelta");
    expect(
      parseAgentTurnEvent({
        ...baseEvent,
        kind: "cancelled",
        message: null,
        usage,
        agentCanonWriteCount: 1,
      }),
    ).toMatchObject({ kind: "cancelled", agentCanonWriteCount: 1 });
    expect(() =>
      parseAgentTurnEvent({
        ...baseEvent,
        kind: "cancelled",
        message: null,
        usage,
        agentCanonWriteCount: -1,
      }),
    ).toThrow();
  });

  it("invokes consent explicitly and drops malformed event payloads", async () => {
    const invoke = vi.fn().mockResolvedValue(session);
    await requestAgentSession("desktop-session", true, invoke);
    expect(invoke).toHaveBeenCalledWith(AGENT_SESSION_LOAD_COMMAND, {
      projectSessionId: "desktop-session",
      historyConsent: true,
    });

    let listener: ((event: { payload: unknown }) => void) | undefined;
    const listen = vi.fn(async (_channel, handler) => {
      listener = handler;
      return () => undefined;
    });
    const received = vi.fn();
    await listenAgentTurnEvents(received, listen);
    expect(listen).toHaveBeenCalledWith(AGENT_TURN_EVENT_CHANNEL, expect.any(Function));
    listener?.({ payload: { ...baseEvent, kind: "conversationDelta", delta: "" } });
    expect(received).not.toHaveBeenCalled();
  });

  it("exports exactly the fourteen approved V2 commands and keeps retired names absent", () => {
    expect(AGENT_V2_COMMANDS).toHaveLength(14);
    expect(AGENT_V2_COMMANDS).not.toContain("agent_policy_set");
    expect(AGENT_V2_COMMANDS).not.toContain("agent_write_confirm");
    expect(AGENT_V2_COMMANDS).not.toContain("agent_write_reject");
    expect(AGENT_V2_COMMANDS).not.toContain("agent_undo_apply");
  });

  it("submits write confirmation using only Host IDs and hashes", async () => {
    const invoke = vi.fn().mockResolvedValue({
      schemaVersion: 2,
      receiptId: `rcpt_${"a".repeat(64)}`,
      transactionHash: "a".repeat(64),
      resultHash: "b".repeat(64),
      relativePaths: ["characters/林舟.md"],
      committedAtMs: 10,
      undoExpiresAtMs: 20,
      projectWriteCount: 1,
    });
    const plan = {
      pendingId: "pnd_10000000000040008000000000000001",
      planHash: "c".repeat(64),
      sourceBindingHash: "d".repeat(64),
      policyHash: "e".repeat(64),
    } as unknown as AgentWritePlanV2;
    await requestAgentWriteConfirmation("desktop-session", plan, invoke);
    expect(invoke).toHaveBeenCalledWith(AGENT_WRITE_CONFIRMATION_SUBMIT_COMMAND, {
      input: {
        schemaVersion: 2,
        projectSessionId: "desktop-session",
        pendingId: plan.pendingId,
        planHash: plan.planHash,
        sourceBindingHash: plan.sourceBindingHash,
        policyHash: plan.policyHash,
        confirmed: true,
      },
    });
    expect(JSON.stringify(invoke.mock.calls[0])).not.toMatch(/body|content|path/u);
  });
});
