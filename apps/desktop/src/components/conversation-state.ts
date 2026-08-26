import type { AppError } from "../lib/tauri/health";
import type {
  ProviderChatMessage,
  ProviderFinishReason,
  ProviderRunEvent,
  ProviderRunStarted,
  ProviderUsage,
} from "../lib/tauri/provider-chat";
import type { ProviderId } from "../lib/tauri/provider";

export type ConversationRunStatus =
  "connecting" | "streaming" | "completed" | "failed" | "cancelled";

export type ConversationMessage = {
  id: string;
  role: "user" | "assistant";
  content: string;
  runId: string;
  providerId: ProviderId;
  model: string;
  status?: ConversationRunStatus;
  usage?: ProviderUsage;
  finishReason?: ProviderFinishReason;
  error?: AppError;
};

type ActiveConversationRun = ProviderRunStarted & {
  assistantMessageId: string;
  lastSequence: number;
};

export type ConversationState = {
  messages: ConversationMessage[];
  activeRun: ActiveConversationRun | null;
};

export const EMPTY_CONVERSATION_STATE: ConversationState = {
  messages: [],
  activeRun: null,
};

export function beginConversationRun(
  state: ConversationState,
  started: ProviderRunStarted,
  userContent: string,
): ConversationState {
  if (state.activeRun) {
    return state;
  }
  const userMessageId = `${started.runId}-user`;
  const assistantMessageId = `${started.runId}-assistant`;
  return {
    messages: [
      ...state.messages,
      {
        id: userMessageId,
        role: "user",
        content: userContent,
        runId: started.runId,
        providerId: started.providerId,
        model: started.model,
      },
      {
        id: assistantMessageId,
        role: "assistant",
        content: "",
        runId: started.runId,
        providerId: started.providerId,
        model: started.model,
        status: "connecting",
      },
    ],
    activeRun: {
      ...started,
      assistantMessageId,
      lastSequence: 0,
    },
  };
}

function updateAssistant(
  state: ConversationState,
  updater: (message: ConversationMessage) => ConversationMessage,
): ConversationMessage[] {
  const messageId = state.activeRun?.assistantMessageId;
  if (!messageId) {
    return state.messages;
  }
  return state.messages.map((message) => (message.id === messageId ? updater(message) : message));
}

export function applyProviderRunEvent(
  state: ConversationState,
  event: ProviderRunEvent,
): ConversationState {
  const active = state.activeRun;
  if (
    !active ||
    event.runId !== active.runId ||
    event.providerId !== active.providerId ||
    event.model !== active.model ||
    event.sequence <= active.lastSequence
  ) {
    return state;
  }

  const nextActive = { ...active, lastSequence: event.sequence };
  if (event.kind === "textDelta") {
    return {
      messages: updateAssistant(state, (message) => ({
        ...message,
        content: message.content + event.delta,
        status: "streaming",
      })),
      activeRun: nextActive,
    };
  }
  if (event.kind === "usage") {
    return {
      messages: updateAssistant(state, (message) => ({
        ...message,
        usage: event.usage,
      })),
      activeRun: nextActive,
    };
  }
  if (event.kind === "completed") {
    return {
      messages: updateAssistant(state, (message) => ({
        ...message,
        status: "completed",
        finishReason: event.finishReason,
      })),
      activeRun: null,
    };
  }
  if (event.kind === "failed") {
    return {
      messages: updateAssistant(state, (message) => ({
        ...message,
        status: "failed",
        error: event.error,
      })),
      activeRun: null,
    };
  }
  return {
    messages: updateAssistant(state, (message) => ({
      ...message,
      status: "cancelled",
    })),
    activeRun: null,
  };
}

export function toProviderChatMessages(messages: ConversationMessage[]): ProviderChatMessage[] {
  return messages.flatMap((message): ProviderChatMessage[] => {
    if (message.role === "assistant" && message.content.length === 0) {
      return [];
    }
    return [{ role: message.role, content: message.content }];
  });
}
