import type {
  ConversationSession,
  ConversationTurnEvent,
  ConversationTurnStarted,
} from "./tauri/conversation";

type Cursor = {
  attemptId: string | null;
  attemptNumber: number;
  sequence: number;
  terminal: boolean;
};
// Local event reconciliation only. This never calls a Provider or stores a server cursor.
export class ConversationEventCursor {
  private turns = new Map<string, Cursor>();
  isOlder(session: ConversationSession) {
    return session.turns.some((turn) => {
      const previous = this.turns.get(turn.turnId);
      const state = turn.retryState;
      return (
        previous &&
        state &&
        (previous.attemptNumber > state.attemptNumber ||
          (previous.attemptId === state.attemptId &&
            previous.terminal &&
            turn.status === "running"))
      );
    });
  }
  observe(session: ConversationSession) {
    for (const turn of session.turns) {
      const previous = this.turns.get(turn.turnId);
      const state = turn.retryState;
      if (previous && state && previous.attemptNumber > state.attemptNumber) continue;
      if (previous && (!state || previous.attemptId === state.attemptId)) {
        previous.terminal ||= turn.status !== "running";
      } else {
        this.turns.set(turn.turnId, {
          attemptId: state?.attemptId ?? null,
          attemptNumber: state?.attemptNumber ?? 0,
          sequence: 0,
          terminal: turn.status !== "running",
        });
      }
    }
  }
  expect(started: ConversationTurnStarted) {
    const previous = this.turns.get(started.turnId);
    if (previous && previous.attemptNumber >= started.attemptNumber) return;
    this.turns.set(started.turnId, {
      attemptId: started.attemptId,
      attemptNumber: started.attemptNumber,
      sequence: 0,
      terminal: false,
    });
  }
  accept(event: ConversationTurnEvent) {
    let cursor = this.turns.get(event.turnId);
    if (!cursor) {
      if (event.attemptNumber !== 1 || this.turns.size >= 10000) return false;
      cursor = { attemptId: event.attemptId, attemptNumber: 1, sequence: 0, terminal: false };
      this.turns.set(event.turnId, cursor);
    }
    // New-attempt events arriving before its acknowledgement wait for the Store.
    if (
      cursor.terminal ||
      cursor.attemptId !== event.attemptId ||
      cursor.attemptNumber !== event.attemptNumber ||
      event.sequence <= cursor.sequence
    )
      return false;
    cursor.sequence = event.sequence;
    cursor.terminal = ["completed", "failed", "cancelled"].includes(event.kind);
    return true;
  }
}
