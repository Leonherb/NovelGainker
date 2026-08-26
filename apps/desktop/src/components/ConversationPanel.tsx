import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { presentAppError } from "../errors/present-app-error";
import { ZH_CN } from "../i18n/zh-CN";
import type { AppError } from "../lib/tauri/health";
import {
  listenProviderRunEvents,
  requestProviderChatCancel,
  requestProviderChatStart,
  type ProviderRunEvent,
} from "../lib/tauri/provider-chat";
import {
  PROJECT_CREATE_WORKFLOW_ID,
  CHARACTER_CREATE_WORKFLOW_ID,
  MATERIAL_CREATE_WORKFLOW_ID,
  PLOT_CREATE_WORKFLOW_ID,
  SETTING_CREATE_WORKFLOW_ID,
  STORYTELLING_WORKFLOW_ID,
  requestWorkflowRegistry,
  type WorkflowRegistrySnapshot,
} from "../lib/tauri/workflow";
import {
  listenWorkflowRunEvents,
  requestCharacterWorkflowRunPrepare,
  requestCharacterWorkflowSourceCatalog,
  requestMaterialWorkflowRunPrepare,
  requestMaterialWorkflowSourceCatalog,
  requestPlotWorkflowRunPrepare,
  requestPlotWorkflowSourceCatalog,
  requestWorkflowProposalApply,
  requestWorkflowRunAbandon,
  requestWorkflowRunCancel,
  requestWorkflowRunGenerate,
  requestSettingWorkflowRunPrepare,
  requestWorkflowSourceCatalog,
  requestWorkflowRunStatus,
  type CharacterBriefInput,
  type MaterialBriefInput,
  type MaterialType,
  type PlotBriefInput,
  type SettingBriefInput,
  type SettingType,
  type WorkflowId,
  type WorkflowSourceCatalogSnapshot,
  type WorkflowRunStatusSnapshot,
  type WorkflowRunState,
} from "../lib/tauri/workflow-run";
import type { ProviderStateSnapshot } from "../lib/tauri/provider";
import {
  listenRouterRunEvents,
  requestLocalRouterDecision,
  requestRouterRunCancel,
  requestRouterRunStart,
  type RouterConfidence,
  type RouterEntryMode,
  type RouterRunEvent,
  type RouterRunStarted,
  type RoutableWorkflowId,
} from "../lib/tauri/router";
import { StoryContextBuilder } from "./StoryContextBuilder";
import { getProviderConnectionLabel, PROVIDER_PRESENTATION } from "./provider-settings-model";
import {
  applyProviderRunEvent,
  beginConversationRun,
  EMPTY_CONVERSATION_STATE,
  toProviderChatMessages,
  type ConversationMessage,
} from "./conversation-state";

const MAX_MESSAGE_BYTES = 32 * 1024;

export type ConversationPanelProps = {
  desktopRuntime: boolean;
  providerState: ProviderStateSnapshot;
  projectSessionId?: string | null;
  onRunActiveChange?: (active: boolean) => void;
  onProjectCreateRequested?: () => void;
  onCanonApplied?: (relativePath: string) => void;
};

type RouterSurface =
  | { kind: "idle" }
  | { kind: "consent"; text: string }
  | { kind: "running"; text: string; run: RouterRunStarted }
  | {
      kind: "selector";
      text: string;
      reason: "local" | "confirmation" | "unavailable" | "cancelled";
      confidence?: Exclude<RouterConfidence, "high">;
      usage?: { inputTokens: number; outputTokens: number; totalTokens: number } | null;
    };
function byteLength(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function statusLabel(message: ConversationMessage): string {
  if (message.status === "connecting") {
    return ZH_CN.conversation.connecting;
  }
  if (message.status === "streaming") {
    return ZH_CN.conversation.streaming;
  }
  if (message.status === "completed") {
    return ZH_CN.conversation.completed;
  }
  if (message.status === "failed") {
    return ZH_CN.conversation.failed;
  }
  if (message.status === "cancelled") {
    return ZH_CN.conversation.cancelled;
  }
  return ZH_CN.conversation.sent;
}

function MessageCard({
  message,
  onRetry,
  retryDisabled,
}: {
  message: ConversationMessage;
  onRetry?: () => void;
  retryDisabled: boolean;
}) {
  const presentation = PROVIDER_PRESENTATION[message.providerId];
  const presentedError = message.error ? presentAppError(message.error) : null;
  return (
    <article className="conversation-message" data-role={message.role}>
      <header>
        <strong>{message.role === "user" ? ZH_CN.conversation.you : presentation.name}</strong>
        <span>
          {message.role === "assistant"
            ? `${message.model} · ${statusLabel(message)}`
            : statusLabel(message)}
        </span>
      </header>
      <p>{message.content || ZH_CN.conversation.waitingForResponse}</p>
      {presentedError ? (
        <div className="conversation-error" role="alert">
          <strong>{presentedError.title}</strong>
          <span>{presentedError.message}</span>
          <small>{presentedError.recovery}</small>
        </div>
      ) : null}
      {message.status === "failed" && onRetry ? (
        <button
          aria-label={ZH_CN.conversation.retryWithProvider(presentation.name)}
          className="conversation-retry"
          disabled={retryDisabled}
          onClick={onRetry}
          title={
            retryDisabled ? ZH_CN.conversation.retryRequiresProvider(presentation.name) : undefined
          }
          type="button"
        >
          {ZH_CN.conversation.retrySameProvider}
        </button>
      ) : null}
      {message.role === "assistant" &&
      (message.status === "completed" ||
        message.status === "failed" ||
        message.status === "cancelled") ? (
        <footer>
          {message.usage
            ? ZH_CN.conversation.usage(
                message.usage.inputTokens,
                message.usage.outputTokens,
                message.usage.totalTokens,
              )
            : ZH_CN.conversation.usageUnavailable}
        </footer>
      ) : null}
    </article>
  );
}

export function ConversationPanel({
  desktopRuntime,
  providerState,
  projectSessionId = null,
  onRunActiveChange,
  onProjectCreateRequested,
  onCanonApplied,
}: ConversationPanelProps) {
  const [conversation, setConversation] = useState(EMPTY_CONVERSATION_STATE);
  const [input, setInput] = useState("");
  const [starting, setStarting] = useState(false);
  const [workflowRegistry, setWorkflowRegistry] = useState<WorkflowRegistrySnapshot | null>(null);
  const [workflowRegistryFailed, setWorkflowRegistryFailed] = useState(false);
  const [workflowRunStatus, setWorkflowRunStatus] = useState<WorkflowRunStatusSnapshot | null>(
    null,
  );
  const [workflowRunLoading, setWorkflowRunLoading] = useState(false);
  const [workflowRunError, setWorkflowRunError] = useState<AppError | null>(null);
  const [workflowRunAction, setWorkflowRunAction] = useState<
    "prepare" | "generate" | "apply" | "cancel" | "abandon" | null
  >(null);
  const [workflowFormOpen, setWorkflowFormOpen] = useState(false);
  const [storyContextOpen, setStoryContextOpen] = useState(false);
  const [workflowBrief, setWorkflowBrief] = useState("");
  const [characterCatalog, setCharacterCatalog] = useState<WorkflowSourceCatalogSnapshot | null>(
    null,
  );
  const [characterOperation, setCharacterOperation] = useState<"create" | "enhance">("create");
  const [characterReplaceTarget, setCharacterReplaceTarget] = useState<string | null>(null);
  const [selectedCharacterSources, setSelectedCharacterSources] = useState<string[]>([]);
  const [workflowFormId, setWorkflowFormId] = useState<WorkflowId>(CHARACTER_CREATE_WORKFLOW_ID);
  const [settingDisplayName, setSettingDisplayName] = useState("");
  const [settingType, setSettingType] = useState<SettingType>("world");
  const [settingObjective, setSettingObjective] = useState("");
  const [settingMustInclude, setSettingMustInclude] = useState("");
  const [settingConstraints, setSettingConstraints] = useState("");
  const [settingCatalog, setSettingCatalog] = useState<WorkflowSourceCatalogSnapshot | null>(null);
  const [settingOperation, setSettingOperation] = useState<"create" | "replace">("create");
  const [settingReplaceTarget, setSettingReplaceTarget] = useState<string | null>(null);
  const [selectedSettingSources, setSelectedSettingSources] = useState<string[]>([]);
  const [materialDisplayName, setMaterialDisplayName] = useState("");
  const [materialType, setMaterialType] = useState<MaterialType>("theme");
  const [materialSeed, setMaterialSeed] = useState("");
  const [materialIntendedUse, setMaterialIntendedUse] = useState("");
  const [materialConstraints, setMaterialConstraints] = useState("");
  const [materialCatalog, setMaterialCatalog] = useState<WorkflowSourceCatalogSnapshot | null>(
    null,
  );
  const [materialOperation, setMaterialOperation] = useState<"create" | "replace">("create");
  const [materialReplaceTarget, setMaterialReplaceTarget] = useState<string | null>(null);
  const [selectedMaterialSources, setSelectedMaterialSources] = useState<string[]>([]);
  const [plotTitle, setPlotTitle] = useState("");
  const [plotGenre, setPlotGenre] = useState("");
  const [plotCoreQuestion, setPlotCoreQuestion] = useState("");
  const [plotDesiredEnding, setPlotDesiredEnding] = useState("");
  const [plotMustInclude, setPlotMustInclude] = useState("");
  const [plotForbiddenDirections, setPlotForbiddenDirections] = useState("");
  const [plotCatalog, setPlotCatalog] = useState<WorkflowSourceCatalogSnapshot | null>(null);
  const [plotOperation, setPlotOperation] = useState<"create" | "replace">("create");
  const [plotReplaceTarget, setPlotReplaceTarget] = useState<string | null>(null);
  const [selectedPlotSources, setSelectedPlotSources] = useState<string[]>([]);
  const [confirmingAbandon, setConfirmingAbandon] = useState(false);
  const [confirmingApply, setConfirmingApply] = useState(false);
  const [workflowRepairing, setWorkflowRepairing] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [startError, setStartError] = useState<AppError | null>(null);
  const [routerSurface, setRouterSurface] = useState<RouterSurface>({ kind: "idle" });
  const [routerStarting, setRouterStarting] = useState(false);
  const [routerCancelling, setRouterCancelling] = useState(false);
  const [routerError, setRouterError] = useState<AppError | null>(null);
  const startingRef = useRef(false);
  const earlyEventsRef = useRef<ProviderRunEvent[]>([]);
  const routerStartingRef = useRef(false);
  const activeRouterRunRef = useRef<RouterRunStarted | null>(null);
  const routerTextRef = useRef("");
  const earlyRouterEventsRef = useRef<RouterRunEvent[]>([]);
  const handleRouterEventRef = useRef<(event: RouterRunEvent) => void>(() => undefined);
  const selectedProvider =
    providerState.providers.find((provider) => provider.selected) ?? providerState.providers[0];
  const selectedPresentation = PROVIDER_PRESENTATION[selectedProvider.providerId];
  const trimmedInput = input.trim();
  const inputBytes = useMemo(() => byteLength(trimmedInput), [trimmedInput]);
  const inputTooLong = inputBytes > MAX_MESSAGE_BYTES;
  const providerReady =
    selectedProvider.configured && selectedProvider.connectionStatus === "connected";
  const activeRun = conversation.activeRun;
  const workflowRunActive = Boolean(workflowRunStatus?.hasActiveRun);
  const workflowRun = workflowRunStatus?.run ?? null;
  const routerRunning = routerSurface.kind === "running";
  const routerBusy = routerStarting || routerRunning;
  const routerInputValid =
    trimmedInput.length > 0 &&
    [...trimmedInput].length <= 1_000 &&
    byteLength(trimmedInput) <= 4 * 1024;
  const canIdentifyWorkflow =
    desktopRuntime &&
    routerInputValid &&
    !activeRun &&
    !starting &&
    !routerBusy &&
    !workflowRunActive;
  const workflowManifest = workflowRunStatus?.contextManifest ?? null;
  const workflowProposal = workflowRunStatus?.proposalView ?? null;
  const normalizedWorkflowBrief = workflowBrief.replace(/\r\n?/gu, "\n").trim();
  const workflowBriefCharacters = [...normalizedWorkflowBrief].length;
  const workflowBriefBytes = byteLength(normalizedWorkflowBrief);
  const workflowBriefInvalid =
    workflowBriefCharacters === 0 ||
    workflowBriefCharacters > 4_000 ||
    workflowBriefBytes > 16 * 1024;
  const characterFormInvalid =
    workflowBriefInvalid ||
    selectedCharacterSources.length > 6 ||
    (characterOperation === "enhance" &&
      (characterReplaceTarget === null ||
        !selectedCharacterSources.includes(characterReplaceTarget)));
  const settingMustIncludeValues = settingMustInclude
    .replace(/\r\n?/gu, "\n")
    .split("\n")
    .map((value) => value.trim())
    .filter(Boolean);
  const settingFormInvalid =
    settingDisplayName.trim().length === 0 ||
    settingDisplayName.includes("\n") ||
    [...settingDisplayName.trim()].length > 80 ||
    byteLength(settingDisplayName.trim()) > 256 ||
    settingObjective.trim().length === 0 ||
    [...settingObjective.trim()].length > 1_000 ||
    byteLength(settingObjective.trim()) > 4 * 1024 ||
    settingMustIncludeValues.length < 1 ||
    settingMustIncludeValues.length > 12 ||
    new Set(settingMustIncludeValues).size !== settingMustIncludeValues.length ||
    settingMustIncludeValues.some((value) => [...value].length > 160 || byteLength(value) > 512) ||
    [...settingConstraints.trim()].length > 1_000 ||
    byteLength(settingConstraints.trim()) > 4 * 1024 ||
    selectedSettingSources.length > 6 ||
    (settingOperation === "replace" &&
      (settingReplaceTarget === null || !selectedSettingSources.includes(settingReplaceTarget)));
  const materialFormInvalid =
    materialDisplayName.trim().length === 0 ||
    materialDisplayName.includes("\n") ||
    [...materialDisplayName.trim()].length > 80 ||
    byteLength(materialDisplayName.trim()) > 256 ||
    materialSeed.trim().length === 0 ||
    [...materialSeed.trim()].length > 2_000 ||
    byteLength(materialSeed.trim()) > 8 * 1024 ||
    materialIntendedUse.trim().length === 0 ||
    [...materialIntendedUse.trim()].length > 1_000 ||
    byteLength(materialIntendedUse.trim()) > 4 * 1024 ||
    [...materialConstraints.trim()].length > 1_000 ||
    byteLength(materialConstraints.trim()) > 4 * 1024 ||
    selectedMaterialSources.length > 6 ||
    (materialOperation === "replace" && materialReplaceTarget === null);
  const plotMustIncludeValues = plotMustInclude
    .replace(/\r\n?/gu, "\n")
    .split("\n")
    .map((value) => value.trim())
    .filter(Boolean);
  const plotForbiddenDirectionValues = plotForbiddenDirections
    .replace(/\r\n?/gu, "\n")
    .split("\n")
    .map((value) => value.trim())
    .filter(Boolean);
  const plotFormInvalid =
    plotTitle.trim().length === 0 ||
    plotTitle.includes("\n") ||
    [...plotTitle.trim()].length > 120 ||
    byteLength(plotTitle.trim()) > 512 ||
    plotGenre.trim().length === 0 ||
    plotGenre.includes("\n") ||
    [...plotGenre.trim()].length > 80 ||
    byteLength(plotGenre.trim()) > 256 ||
    plotCoreQuestion.trim().length === 0 ||
    [...plotCoreQuestion.trim()].length > 1_000 ||
    byteLength(plotCoreQuestion.trim()) > 4 * 1024 ||
    plotDesiredEnding.trim().length === 0 ||
    [...plotDesiredEnding.trim()].length > 2_000 ||
    byteLength(plotDesiredEnding.trim()) > 8 * 1024 ||
    plotMustIncludeValues.length > 8 ||
    new Set(plotMustIncludeValues).size !== plotMustIncludeValues.length ||
    plotMustIncludeValues.some(
      (value) => [...value].length > 500 || byteLength(value) > 2 * 1024,
    ) ||
    plotForbiddenDirectionValues.length > 8 ||
    new Set(plotForbiddenDirectionValues).size !== plotForbiddenDirectionValues.length ||
    plotForbiddenDirectionValues.some(
      (value) => [...value].length > 500 || byteLength(value) > 2 * 1024,
    ) ||
    selectedPlotSources.length > 8 ||
    (plotOperation === "replace" && plotReplaceTarget !== "plot/main.md");
  const workflowFormInvalid =
    workflowFormId === SETTING_CREATE_WORKFLOW_ID
      ? settingFormInvalid
      : workflowFormId === MATERIAL_CREATE_WORKFLOW_ID
        ? materialFormInvalid
        : workflowFormId === PLOT_CREATE_WORKFLOW_ID
          ? plotFormInvalid
          : characterFormInvalid;
  const manifestProviderReady = Boolean(
    workflowManifest &&
    providerReady &&
    selectedProvider.providerId === workflowManifest.providerId,
  );
  const canSend =
    desktopRuntime &&
    providerReady &&
    !activeRun &&
    !starting &&
    !routerBusy &&
    routerSurface.kind === "idle" &&
    !workflowRunActive &&
    trimmedInput.length > 0 &&
    !inputTooLong;

  useEffect(() => {
    onRunActiveChange?.(Boolean(activeRun) || starting || routerBusy || workflowRunActive);
  }, [activeRun, onRunActiveChange, routerBusy, starting, workflowRunActive]);

  useEffect(() => {
    if (!desktopRuntime) {
      return undefined;
    }
    let disposed = false;
    void requestWorkflowRegistry()
      .then((snapshot) => {
        if (!disposed) {
          setWorkflowRegistry(snapshot);
        }
        setWorkflowRegistryFailed(false);
      })
      .catch(() => {
        if (!disposed) {
          setWorkflowRegistry(null);
          setWorkflowRegistryFailed(true);
        }
      });
    return () => {
      disposed = true;
    };
  }, [desktopRuntime]);

  useEffect(() => {
    let disposed = false;
    const timer = window.setTimeout(() => {
      if (!desktopRuntime || !projectSessionId) {
        setWorkflowRunStatus(null);
        setWorkflowRunError(null);
        setWorkflowRunLoading(false);
        setConfirmingAbandon(false);
        setWorkflowRepairing(false);
        setWorkflowFormOpen(false);
        setStoryContextOpen(false);
        setWorkflowBrief("");
        setCharacterCatalog(null);
        setCharacterOperation("create");
        setCharacterReplaceTarget(null);
        setSelectedCharacterSources([]);
        setWorkflowFormId(CHARACTER_CREATE_WORKFLOW_ID);
        setSettingCatalog(null);
        setSettingOperation("create");
        setSettingReplaceTarget(null);
        setSelectedSettingSources([]);
        setSettingDisplayName("");
        setSettingObjective("");
        setSettingMustInclude("");
        setSettingConstraints("");
        setSettingType("world");
        setMaterialCatalog(null);
        setMaterialOperation("create");
        setMaterialReplaceTarget(null);
        setSelectedMaterialSources([]);
        setMaterialDisplayName("");
        setMaterialType("theme");
        setMaterialSeed("");
        setMaterialIntendedUse("");
        setMaterialConstraints("");
        setPlotCatalog(null);
        setPlotOperation("create");
        setPlotReplaceTarget(null);
        setSelectedPlotSources([]);
        setPlotTitle("");
        setPlotGenre("");
        setPlotCoreQuestion("");
        setPlotDesiredEnding("");
        setPlotMustInclude("");
        setPlotForbiddenDirections("");
        return;
      }
      setWorkflowRunLoading(true);
      setWorkflowRunError(null);
      setConfirmingAbandon(false);
      void requestWorkflowRunStatus(projectSessionId)
        .then((snapshot) => {
          if (!disposed) {
            setWorkflowRunStatus(snapshot);
          }
        })
        .catch((error: unknown) => {
          if (!disposed) {
            setWorkflowRunStatus(null);
            setWorkflowRunError(error as AppError);
          }
        })
        .finally(() => {
          if (!disposed) {
            setWorkflowRunLoading(false);
          }
        });
    }, 0);
    return () => {
      disposed = true;
      window.clearTimeout(timer);
    };
  }, [desktopRuntime, projectSessionId]);

  useEffect(() => {
    if (!desktopRuntime) {
      return undefined;
    }
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listenProviderRunEvents((event) => {
      setConversation((current) => {
        if (!current.activeRun && startingRef.current) {
          earlyEventsRef.current.push(event);
          return current;
        }
        return applyProviderRunEvent(current, event);
      });
    })
      .then((stopListening) => {
        if (disposed) {
          stopListening();
        } else {
          unlisten = stopListening;
        }
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [desktopRuntime]);

  useEffect(() => {
    if (!desktopRuntime || !projectSessionId) {
      return undefined;
    }
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listenWorkflowRunEvents((event) => {
      if (disposed) {
        return;
      }
      if (event.kind === "repairing") {
        setWorkflowRepairing(true);
      } else {
        setWorkflowRepairing(false);
      }
      if (event.kind === "proposalReady") {
        setWorkflowRunError(null);
      } else if (event.error) {
        setWorkflowRunError(event.error);
      }
      void requestWorkflowRunStatus(projectSessionId)
        .then((snapshot) => {
          if (!disposed) {
            setWorkflowRunStatus((current) => {
              const currentSequence = current?.run?.sequence ?? -1;
              const nextSequence = snapshot.run?.sequence ?? -1;
              return nextSequence >= currentSequence ? snapshot : current;
            });
          }
        })
        .catch((error: unknown) => {
          if (!disposed) {
            setWorkflowRunError(error as AppError);
          }
        });
    })
      .then((stopListening) => {
        if (disposed) {
          stopListening();
        } else {
          unlisten = stopListening;
        }
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [desktopRuntime, projectSessionId]);

  const startRun = async (
    expectedProviderId: ConversationMessage["providerId"],
    userContent: string,
    requestMessages: ReturnType<typeof toProviderChatMessages>,
    clearInput: boolean,
  ) => {
    if (
      startingRef.current ||
      activeRun ||
      selectedProvider.providerId !== expectedProviderId ||
      !providerReady
    ) {
      return;
    }
    setStarting(true);
    startingRef.current = true;
    earlyEventsRef.current = [];
    setStartError(null);
    try {
      const started = await requestProviderChatStart(expectedProviderId, requestMessages);
      setConversation((current) =>
        earlyEventsRef.current
          .filter(
            (event) =>
              event.runId === started.runId &&
              event.providerId === started.providerId &&
              event.model === started.model,
          )
          .reduce(applyProviderRunEvent, beginConversationRun(current, started, userContent)),
      );
      if (clearInput) {
        setInput("");
      }
    } catch (error: unknown) {
      setStartError(error as AppError);
    } finally {
      earlyEventsRef.current = [];
      startingRef.current = false;
      setStarting(false);
    }
  };

  const send = () => {
    if (!canSend) {
      return;
    }
    const userContent = trimmedInput;
    void startRun(
      selectedProvider.providerId,
      userContent,
      [...toProviderChatMessages(conversation.messages), { role: "user", content: userContent }],
      true,
    );
  };

  const retry = (failedMessage: ConversationMessage) => {
    if (
      failedMessage.role !== "assistant" ||
      failedMessage.status !== "failed" ||
      selectedProvider.providerId !== failedMessage.providerId
    ) {
      return;
    }
    const userIndex = conversation.messages.findIndex(
      (message) => message.runId === failedMessage.runId && message.role === "user",
    );
    const userMessage = conversation.messages[userIndex];
    if (userIndex < 0 || !userMessage) {
      return;
    }
    void startRun(
      failedMessage.providerId,
      userMessage.content,
      [
        ...toProviderChatMessages(conversation.messages.slice(0, userIndex)),
        { role: "user", content: userMessage.content },
      ],
      false,
    );
  };

  const cancel = async () => {
    if (!activeRun || cancelling) {
      return;
    }
    setCancelling(true);
    try {
      await requestProviderChatCancel(activeRun.runId);
    } catch (error: unknown) {
      setStartError(error as AppError);
    } finally {
      setCancelling(false);
    }
  };

  const openWorkflowForm = useCallback(
    (workflowId: WorkflowId) => {
      setWorkflowRunError(null);
      setStoryContextOpen(false);
      setWorkflowFormId(workflowId);
      setWorkflowFormOpen(true);
      if (!projectSessionId) {
        return;
      }
      if (workflowId === CHARACTER_CREATE_WORKFLOW_ID) {
        setCharacterCatalog(null);
        setCharacterOperation("create");
        setCharacterReplaceTarget(null);
        setSelectedCharacterSources([]);
        void requestCharacterWorkflowSourceCatalog(projectSessionId)
          .then(setCharacterCatalog)
          .catch((error: unknown) => setWorkflowRunError(error as AppError));
      } else if (workflowId === SETTING_CREATE_WORKFLOW_ID) {
        setSettingCatalog(null);
        setSettingOperation("create");
        setSettingReplaceTarget(null);
        setSelectedSettingSources([]);
        void requestWorkflowSourceCatalog(projectSessionId)
          .then(setSettingCatalog)
          .catch((error: unknown) => setWorkflowRunError(error as AppError));
      } else if (workflowId === MATERIAL_CREATE_WORKFLOW_ID) {
        setMaterialCatalog(null);
        setMaterialOperation("create");
        setMaterialReplaceTarget(null);
        setSelectedMaterialSources([]);
        void requestMaterialWorkflowSourceCatalog(projectSessionId)
          .then(setMaterialCatalog)
          .catch((error: unknown) => setWorkflowRunError(error as AppError));
      } else if (workflowId === PLOT_CREATE_WORKFLOW_ID) {
        setPlotCatalog(null);
        setPlotOperation("create");
        setPlotReplaceTarget(null);
        setSelectedPlotSources([]);
        void requestPlotWorkflowSourceCatalog(projectSessionId)
          .then((catalog) => {
            setPlotCatalog(catalog);
            if (catalog.replaceTargets.length > 0) {
              setPlotOperation("replace");
              setPlotReplaceTarget("plot/main.md");
              setPlotTitle(catalog.replaceTargets[0]?.displayName ?? "");
            }
          })
          .catch((error: unknown) => setWorkflowRunError(error as AppError));
      }
    },
    [projectSessionId],
  );

  const routeToWorkflow = useCallback(
    (workflowId: RoutableWorkflowId, entryMode: RouterEntryMode = "default"): boolean => {
      const entry = workflowRegistry?.workflows.find(
        (workflow) =>
          workflow.id === workflowId && workflow.executionAvailable && workflow.routable,
      );
      if (!entry || workflowRunActive || workflowRunAction !== null) {
        return false;
      }
      if (workflowId === PROJECT_CREATE_WORKFLOW_ID) {
        if (!desktopRuntime || projectSessionId !== null || !onProjectCreateRequested) {
          return false;
        }
        onProjectCreateRequested();
        return true;
      }
      if (!projectSessionId) {
        return false;
      }
      openWorkflowForm(workflowId as WorkflowId);
      if (workflowId === CHARACTER_CREATE_WORKFLOW_ID) {
        setCharacterOperation(entryMode === "character-enhance" ? "enhance" : "create");
      }
      return true;
    },
    [
      desktopRuntime,
      onProjectCreateRequested,
      openWorkflowForm,
      projectSessionId,
      workflowRegistry,
      workflowRunAction,
      workflowRunActive,
    ],
  );

  const clearRouterSurface = () => {
    activeRouterRunRef.current = null;
    routerTextRef.current = "";
    earlyRouterEventsRef.current = [];
    routerStartingRef.current = false;
    setRouterStarting(false);
    setRouterCancelling(false);
    setRouterError(null);
    setRouterSurface({ kind: "idle" });
  };

  const identifyWorkflow = async () => {
    if (!canIdentifyWorkflow || routerStartingRef.current) {
      return;
    }
    const text = trimmedInput;
    setInput("");
    setRouterError(null);
    setRouterStarting(true);
    routerStartingRef.current = true;
    routerTextRef.current = text;
    try {
      const decision = await requestLocalRouterDecision(text);
      if (decision.outcome === "direct" && decision.workflowId && decision.entryMode) {
        if (routeToWorkflow(decision.workflowId, decision.entryMode)) {
          clearRouterSurface();
        } else {
          setRouterSurface({ kind: "selector", text, reason: "unavailable" });
        }
      } else if (decision.outcome === "needsProvider" && providerReady) {
        setRouterSurface({ kind: "consent", text });
      } else {
        setRouterSurface({
          kind: "selector",
          text,
          reason: decision.outcome === "needsProvider" ? "unavailable" : "local",
        });
      }
    } catch (error: unknown) {
      setRouterError(error as AppError);
      setRouterSurface({ kind: "selector", text, reason: "unavailable" });
    } finally {
      routerStartingRef.current = false;
      setRouterStarting(false);
    }
  };

  const confirmRouterProvider = async () => {
    if (routerSurface.kind !== "consent" || routerStartingRef.current || !providerReady) {
      return;
    }
    const text = routerSurface.text;
    setRouterError(null);
    setRouterStarting(true);
    routerStartingRef.current = true;
    earlyRouterEventsRef.current = [];
    try {
      const started = await requestRouterRunStart(selectedProvider.providerId, text);
      activeRouterRunRef.current = started;
      routerTextRef.current = text;
      setRouterSurface({ kind: "running", text, run: started });
      const early = earlyRouterEventsRef.current;
      earlyRouterEventsRef.current = [];
      early.forEach((event) => handleRouterEventRef.current(event));
    } catch (error: unknown) {
      setRouterError(error as AppError);
      setRouterSurface({ kind: "selector", text, reason: "unavailable" });
    } finally {
      routerStartingRef.current = false;
      setRouterStarting(false);
    }
  };

  const cancelRouterRun = async () => {
    const active = activeRouterRunRef.current;
    if (!active || routerCancelling) {
      return;
    }
    setRouterCancelling(true);
    try {
      await requestRouterRunCancel(active.runId);
    } catch (error: unknown) {
      setRouterError(error as AppError);
      setRouterCancelling(false);
    }
  };

  const selectRouterWorkflow = (
    workflowId: RoutableWorkflowId,
    entryMode: RouterEntryMode = "default",
  ) => {
    if (routeToWorkflow(workflowId, entryMode)) {
      clearRouterSurface();
    }
  };

  const sendRouterTextAsChat = () => {
    if (routerSurface.kind !== "selector" || !providerReady || activeRun || starting) {
      return;
    }
    const text = routerSurface.text;
    clearRouterSurface();
    void startRun(
      selectedProvider.providerId,
      text,
      [...toProviderChatMessages(conversation.messages), { role: "user", content: text }],
      false,
    );
  };

  useEffect(() => {
    if (!desktopRuntime) {
      return undefined;
    }
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const handleEvent = (event: RouterRunEvent) => {
      if (disposed) {
        return;
      }
      const active = activeRouterRunRef.current;
      if (!active) {
        if (routerStartingRef.current) {
          earlyRouterEventsRef.current.push(event);
        }
        return;
      }
      if (
        event.runId !== active.runId ||
        event.providerId !== active.providerId ||
        event.model !== active.model
      ) {
        return;
      }
      const text = routerTextRef.current;
      activeRouterRunRef.current = null;
      setRouterCancelling(false);
      if (event.kind === "completed") {
        if (event.decision.confidence === "high") {
          if (routeToWorkflow(event.decision.workflowId)) {
            clearRouterSurface();
          } else {
            setRouterSurface({ kind: "selector", text, reason: "unavailable" });
          }
        } else {
          setRouterSurface({
            kind: "selector",
            text,
            reason: "confirmation",
            confidence: event.decision.confidence,
            usage: event.usage,
          });
        }
      } else {
        if (event.kind === "failed") {
          setRouterError(event.error);
        }
        setRouterSurface({
          kind: "selector",
          text,
          reason: event.kind === "cancelled" ? "cancelled" : "unavailable",
        });
      }
    };
    handleRouterEventRef.current = handleEvent;
    void listenRouterRunEvents(handleEvent)
      .then((stopListening) => {
        if (disposed) {
          stopListening();
        } else {
          unlisten = stopListening;
        }
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
      handleRouterEventRef.current = () => undefined;
    };
  }, [desktopRuntime, routeToWorkflow]);
  const prepareWorkflowRun = async () => {
    if (!projectSessionId || workflowFormInvalid || workflowRunActive || workflowRunAction) {
      return;
    }
    setWorkflowRunAction("prepare");
    setWorkflowRunError(null);
    try {
      const snapshot =
        workflowFormId === SETTING_CREATE_WORKFLOW_ID
          ? await requestSettingWorkflowRunPrepare(
              projectSessionId,
              {
                schemaVersion: 1,
                displayName: settingDisplayName,
                settingType,
                objective: settingObjective,
                mustInclude: settingMustIncludeValues,
                constraints: settingConstraints.trim() || null,
              } satisfies SettingBriefInput,
              selectedSettingSources,
              settingReplaceTarget,
            )
          : workflowFormId === MATERIAL_CREATE_WORKFLOW_ID
            ? await requestMaterialWorkflowRunPrepare(
                projectSessionId,
                {
                  schemaVersion: 1,
                  displayName: materialDisplayName,
                  materialType,
                  seed: materialSeed,
                  intendedUse: materialIntendedUse,
                  constraints: materialConstraints.trim() || null,
                } satisfies MaterialBriefInput,
                selectedMaterialSources,
                materialReplaceTarget,
              )
            : workflowFormId === PLOT_CREATE_WORKFLOW_ID
              ? await requestPlotWorkflowRunPrepare(
                  projectSessionId,
                  {
                    schemaVersion: 1,
                    title: plotTitle,
                    genre: plotGenre,
                    coreQuestion: plotCoreQuestion,
                    desiredEnding: plotDesiredEnding,
                    mustInclude: plotMustIncludeValues,
                    forbiddenDirections: plotForbiddenDirectionValues,
                  } satisfies PlotBriefInput,
                  selectedPlotSources,
                  plotReplaceTarget,
                )
              : await requestCharacterWorkflowRunPrepare(
                  projectSessionId,
                  {
                    schemaVersion: 2,
                    mode: characterOperation,
                    brief: normalizedWorkflowBrief,
                  } satisfies CharacterBriefInput,
                  selectedCharacterSources,
                  characterReplaceTarget,
                );
      setWorkflowRunStatus(snapshot);
      setWorkflowFormOpen(false);
    } catch (error: unknown) {
      setWorkflowRunError(error as AppError);
    } finally {
      setWorkflowRunAction(null);
    }
  };

  const generateWorkflowRun = async () => {
    if (
      !workflowRun ||
      workflowRun.state !== "context_ready" ||
      !workflowManifest ||
      !manifestProviderReady ||
      workflowRunAction
    ) {
      return;
    }
    setWorkflowRunAction("generate");
    setWorkflowRepairing(false);
    setWorkflowRunError(null);
    try {
      const snapshot = await requestWorkflowRunGenerate(
        workflowRun.runId,
        workflowManifest.manifestHash,
        true,
      );
      setWorkflowRunStatus((current) => {
        const currentSequence = current?.run?.sequence ?? -1;
        const nextSequence = snapshot.run?.sequence ?? -1;
        return nextSequence >= currentSequence ? snapshot : current;
      });
    } catch (error: unknown) {
      setWorkflowRunError(error as AppError);
    } finally {
      setWorkflowRunAction(null);
    }
  };

  const cancelWorkflowRun = async () => {
    if (!projectSessionId || !workflowRun?.canCancel || workflowRunAction) {
      return;
    }
    setWorkflowRunAction("cancel");
    setWorkflowRunError(null);
    try {
      setWorkflowRunStatus(await requestWorkflowRunCancel(projectSessionId));
    } catch (error: unknown) {
      setWorkflowRunError(error as AppError);
    } finally {
      setWorkflowRunAction(null);
    }
  };

  const abandonWorkflowRun = async () => {
    if (!projectSessionId || !workflowRun?.canAbandon || workflowRunAction) {
      return;
    }
    setWorkflowRunAction("abandon");
    setWorkflowRunError(null);
    try {
      setWorkflowRunStatus(await requestWorkflowRunAbandon(projectSessionId, true));
      setConfirmingAbandon(false);
    } catch (error: unknown) {
      setWorkflowRunError(error as AppError);
    } finally {
      setWorkflowRunAction(null);
    }
  };

  const applyWorkflowProposal = async () => {
    if (
      !workflowRun ||
      workflowRun.state !== "awaiting_confirmation" ||
      !workflowProposal ||
      workflowRunAction
    ) {
      return;
    }
    setWorkflowRunAction("apply");
    setWorkflowRunError(null);
    const target = workflowProposal.operation.relativePath;
    try {
      const snapshot = await requestWorkflowProposalApply(
        workflowRun.runId,
        workflowProposal.proposalHash,
        true,
      );
      setWorkflowRunStatus(snapshot);
      setConfirmingApply(false);
      if (snapshot.run?.state === "completed") {
        onCanonApplied?.(target);
      }
    } catch (error: unknown) {
      setWorkflowRunError(error as AppError);
    } finally {
      setWorkflowRunAction(null);
    }
  };

  const shortHash = (hash: string) => `${hash.slice(0, 10)}…${hash.slice(-8)}`;

  const workflowStateLabel = workflowRun
    ? ZH_CN.conversation.workflowRun.states[workflowRun.state as WorkflowRunState]
    : null;
  const workflowProviderLabel = workflowRun?.providerId
    ? PROVIDER_PRESENTATION[workflowRun.providerId].name
    : ZH_CN.conversation.workflowRun.unboundProvider;
  const presentedWorkflowError = workflowRunError ? presentAppError(workflowRunError) : null;
  const presentedStartError = startError ? presentAppError(startError) : null;
  const presentedRouterError = routerError ? presentAppError(routerError) : null;
  const footerStatus = !desktopRuntime
    ? ZH_CN.conversation.desktopOnly
    : routerBusy
      ? ZH_CN.conversation.router.status
      : starting
        ? ZH_CN.conversation.connecting
        : inputTooLong
          ? ZH_CN.conversation.inputTooLong
          : ZH_CN.conversation.selectedStatus(
              selectedPresentation.name,
              getProviderConnectionLabel(selectedProvider.connectionStatus),
            );

  return (
    <>
      <header className="conversation-heading">
        <div>
          <p className="section-kicker">{ZH_CN.conversation.kicker}</p>
          <h1>{ZH_CN.conversation.heading}</h1>
        </div>
        <span className="story-badge">
          {ZH_CN.conversation.currentProvider} · {selectedPresentation.name} ·{" "}
          {selectedPresentation.model}
        </span>
      </header>

      <section aria-label={ZH_CN.conversation.messagesAriaLabel} className="message-canvas">
        <div className="conversation-content">
          <section aria-label={ZH_CN.conversation.workflows.ariaLabel} className="workflow-catalog">
            <header>
              <div>
                <span>{ZH_CN.conversation.workflows.kicker}</span>
                <strong>{ZH_CN.conversation.workflows.heading}</strong>
              </div>
              <small>{ZH_CN.conversation.workflows.builtIn}</small>
            </header>
            {workflowRegistry ? (
              workflowRegistry.workflows.map((workflow) => {
                const isProjectCreate = workflow.id === PROJECT_CREATE_WORKFLOW_ID;
                const isStorytelling = workflow.id === STORYTELLING_WORKFLOW_ID;
                return (
                  <article key={workflow.id}>
                    <div>
                      <strong>{workflow.displayName}</strong>
                      <span>
                        {ZH_CN.conversation.workflows.version(workflow.version)} · {workflow.mode}
                      </span>
                      <p>{workflow.description}</p>
                    </div>
                    <button
                      disabled={
                        isStorytelling
                          ? !desktopRuntime ||
                            !projectSessionId ||
                            workflowRunActive ||
                            workflowRunAction !== null
                          : !workflow.executionAvailable ||
                            (isProjectCreate
                              ? !desktopRuntime ||
                                projectSessionId !== null ||
                                !onProjectCreateRequested
                              : !projectSessionId ||
                                workflowRunActive ||
                                workflowRunAction !== null)
                      }
                      onClick={() => {
                        if (isProjectCreate) {
                          onProjectCreateRequested?.();
                          return;
                        }
                        if (isStorytelling) {
                          setWorkflowFormOpen(false);
                          setStoryContextOpen(true);
                          return;
                        }
                        openWorkflowForm(workflow.id as WorkflowId);
                      }}
                      type="button"
                    >
                      {ZH_CN.conversation.workflows.open(workflow.displayName)}
                    </button>
                  </article>
                );
              })
            ) : (
              <p className="workflow-catalog-status">
                {!desktopRuntime
                  ? ZH_CN.conversation.workflows.desktopOnly
                  : workflowRegistryFailed
                    ? ZH_CN.conversation.workflows.loadFailed
                    : ZH_CN.conversation.workflows.loading}
              </p>
            )}
            {storyContextOpen && projectSessionId && !workflowRun ? (
              <StoryContextBuilder
                key={projectSessionId}
                onClose={() => setStoryContextOpen(false)}
                sessionId={projectSessionId}
              />
            ) : null}
            {workflowFormOpen && projectSessionId && !workflowRun ? (
              <section
                aria-label={
                  workflowFormId === SETTING_CREATE_WORKFLOW_ID
                    ? ZH_CN.conversation.workflowRun.settingFormAriaLabel
                    : workflowFormId === MATERIAL_CREATE_WORKFLOW_ID
                      ? ZH_CN.conversation.workflowRun.materialFormAriaLabel
                      : workflowFormId === PLOT_CREATE_WORKFLOW_ID
                        ? ZH_CN.conversation.workflowRun.plotFormAriaLabel
                        : ZH_CN.conversation.workflowRun.briefFormAriaLabel
                }
                className="workflow-brief-form"
              >
                <header>
                  <div>
                    <strong>
                      {workflowFormId === SETTING_CREATE_WORKFLOW_ID
                        ? ZH_CN.conversation.workflowRun.settingFormTitle
                        : workflowFormId === MATERIAL_CREATE_WORKFLOW_ID
                          ? ZH_CN.conversation.workflowRun.materialFormTitle
                          : workflowFormId === PLOT_CREATE_WORKFLOW_ID
                            ? ZH_CN.conversation.workflowRun.plotFormTitle
                            : ZH_CN.conversation.workflowRun.briefFormTitle}
                    </strong>
                    <span>{ZH_CN.conversation.workflowRun.offlinePrepare}</span>
                  </div>
                </header>
                {workflowFormId === CHARACTER_CREATE_WORKFLOW_ID ? (
                  <>
                    <label htmlFor="character-workflow-operation">
                      {ZH_CN.conversation.workflowRun.characterOperationLabel}
                    </label>
                    <select
                      id="character-workflow-operation"
                      onChange={(event) => {
                        const operation = event.currentTarget.value as "create" | "enhance";
                        setCharacterOperation(operation);
                        if (operation === "create") {
                          setCharacterReplaceTarget(null);
                        }
                      }}
                      value={characterOperation}
                    >
                      <option value="create">
                        {ZH_CN.conversation.workflowRun.characterOperations.create}
                      </option>
                      <option value="enhance">
                        {ZH_CN.conversation.workflowRun.characterOperations.enhance}
                      </option>
                    </select>
                    {characterOperation === "enhance" ? (
                      <>
                        <label htmlFor="character-workflow-replace-target">
                          {ZH_CN.conversation.workflowRun.characterReplaceTargetLabel}
                        </label>
                        <select
                          id="character-workflow-replace-target"
                          onChange={(event) => {
                            const relativePath = event.currentTarget.value || null;
                            setCharacterReplaceTarget(relativePath);
                            if (relativePath) {
                              setSelectedCharacterSources((current) =>
                                current.includes(relativePath)
                                  ? current
                                  : [...current, relativePath].sort(),
                              );
                            }
                          }}
                          value={characterReplaceTarget ?? ""}
                        >
                          <option value="">
                            {ZH_CN.conversation.workflowRun.characterReplaceTargetPlaceholder}
                          </option>
                          {characterCatalog?.replaceTargets.map((target) => (
                            <option key={target.relativePath} value={target.relativePath}>
                              {target.displayName} ·{" "}
                              {target.entitySchemaVersion === 1
                                ? ZH_CN.conversation.workflowRun.characterLegacyBadge
                                : ZH_CN.conversation.workflowRun.characterV2Badge}
                            </option>
                          ))}
                        </select>
                        {characterCatalog?.replaceTargets.find(
                          (target) => target.relativePath === characterReplaceTarget,
                        )?.entitySchemaVersion === 1 ? (
                          <p className="workflow-inline-warning">
                            {ZH_CN.conversation.workflowRun.characterLegacyUpgrade}
                          </p>
                        ) : null}
                      </>
                    ) : null}
                    <label htmlFor="character-workflow-brief">
                      {ZH_CN.conversation.workflowRun.briefLabel}
                    </label>
                    <textarea
                      autoFocus
                      id="character-workflow-brief"
                      maxLength={4_000}
                      onChange={(event) => setWorkflowBrief(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.briefPlaceholder}
                      rows={5}
                      value={workflowBrief}
                    />
                    <small>
                      {ZH_CN.conversation.workflowRun.briefCount(
                        workflowBriefCharacters,
                        workflowBriefBytes,
                      )}
                    </small>
                    <fieldset className="setting-source-picker">
                      <legend>{ZH_CN.conversation.workflowRun.characterSourcesTitle}</legend>
                      <p>{ZH_CN.conversation.workflowRun.characterSourcesBody}</p>
                      <small>
                        {ZH_CN.conversation.workflowRun.characterSourcesCount(
                          selectedCharacterSources.length,
                        )}
                      </small>
                      {characterCatalog === null ? (
                        <p>{ZH_CN.conversation.workflowRun.characterSourcesLoading}</p>
                      ) : characterCatalog.sources.length === 0 ? (
                        <p>{ZH_CN.conversation.workflowRun.characterSourcesEmpty}</p>
                      ) : (
                        characterCatalog.sources.map((source) => {
                          const selected = selectedCharacterSources.includes(source.relativePath);
                          const sourceKindLabel =
                            source.sourceKind === "character"
                              ? source.entitySchemaVersion === 1
                                ? ZH_CN.conversation.workflowRun.characterLegacyBadge
                                : ZH_CN.conversation.workflowRun.characterV2Badge
                              : source.sourceKind === "setting"
                                ? ZH_CN.conversation.workflowRun.characterSourceKinds.setting
                                : source.sourceKind === "plot"
                                  ? ZH_CN.conversation.workflowRun.characterSourceKinds.plot
                                  : ZH_CN.conversation.workflowRun.characterSourceKinds.material;
                          return (
                            <label key={source.relativePath}>
                              <input
                                checked={selected}
                                disabled={
                                  source.relativePath === characterReplaceTarget ||
                                  (!selected && selectedCharacterSources.length >= 6)
                                }
                                onChange={(event) => {
                                  const checked = event.currentTarget.checked;
                                  setSelectedCharacterSources((current) =>
                                    checked
                                      ? [...current, source.relativePath].sort()
                                      : current.filter((path) => path !== source.relativePath),
                                  );
                                }}
                                type="checkbox"
                              />
                              <span>
                                <strong>{source.displayName}</strong>
                                <small>
                                  {sourceKindLabel} · {source.relativePath}
                                </small>
                              </span>
                            </label>
                          );
                        })
                      )}
                    </fieldset>
                  </>
                ) : workflowFormId === SETTING_CREATE_WORKFLOW_ID ? (
                  <>
                    <label htmlFor="setting-workflow-operation">
                      {ZH_CN.conversation.workflowRun.settingOperationLabel}
                    </label>
                    <select
                      id="setting-workflow-operation"
                      onChange={(event) => {
                        const operation = event.currentTarget.value as "create" | "replace";
                        setSettingOperation(operation);
                        if (operation === "create") {
                          setSettingReplaceTarget(null);
                        }
                      }}
                      value={settingOperation}
                    >
                      <option value="create">
                        {ZH_CN.conversation.workflowRun.settingOperations.create}
                      </option>
                      <option value="replace">
                        {ZH_CN.conversation.workflowRun.settingOperations.replace}
                      </option>
                    </select>
                    {settingOperation === "replace" ? (
                      <>
                        <label htmlFor="setting-workflow-replace-target">
                          {ZH_CN.conversation.workflowRun.settingReplaceTargetLabel}
                        </label>
                        <select
                          id="setting-workflow-replace-target"
                          onChange={(event) => {
                            const relativePath = event.currentTarget.value || null;
                            setSettingReplaceTarget(relativePath);
                            if (relativePath) {
                              const source = settingCatalog?.sources.find(
                                (candidate) => candidate.relativePath === relativePath,
                              );
                              if (source) {
                                setSettingDisplayName(source.displayName);
                              }
                              setSelectedSettingSources((current) =>
                                current.includes(relativePath)
                                  ? current
                                  : [...current, relativePath].sort(),
                              );
                            }
                          }}
                          value={settingReplaceTarget ?? ""}
                        >
                          <option value="">
                            {ZH_CN.conversation.workflowRun.settingReplaceTargetPlaceholder}
                          </option>
                          {settingCatalog?.sources.map((source) => (
                            <option key={source.relativePath} value={source.relativePath}>
                              {source.displayName} · {source.relativePath}
                            </option>
                          ))}
                        </select>
                      </>
                    ) : null}
                    <label htmlFor="setting-workflow-name">
                      {ZH_CN.conversation.workflowRun.settingDisplayNameLabel}
                    </label>
                    <input
                      autoFocus
                      id="setting-workflow-name"
                      maxLength={80}
                      onChange={(event) => setSettingDisplayName(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.settingDisplayNamePlaceholder}
                      value={settingDisplayName}
                    />
                    <label htmlFor="setting-workflow-type">
                      {ZH_CN.conversation.workflowRun.settingTypeLabel}
                    </label>
                    <select
                      id="setting-workflow-type"
                      onChange={(event) => setSettingType(event.currentTarget.value as SettingType)}
                      value={settingType}
                    >
                      {(["world", "location", "rule"] as const).map((kind) => (
                        <option key={kind} value={kind}>
                          {ZH_CN.conversation.workflowRun.settingTypes[kind]}
                        </option>
                      ))}
                    </select>
                    <label htmlFor="setting-workflow-objective">
                      {ZH_CN.conversation.workflowRun.settingObjectiveLabel}
                    </label>
                    <textarea
                      id="setting-workflow-objective"
                      maxLength={1_000}
                      onChange={(event) => setSettingObjective(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.settingObjectivePlaceholder}
                      rows={4}
                      value={settingObjective}
                    />
                    <label htmlFor="setting-workflow-must-include">
                      {ZH_CN.conversation.workflowRun.settingMustIncludeLabel}
                    </label>
                    <textarea
                      id="setting-workflow-must-include"
                      onChange={(event) => setSettingMustInclude(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.settingMustIncludePlaceholder}
                      rows={4}
                      value={settingMustInclude}
                    />
                    <label htmlFor="setting-workflow-constraints">
                      {ZH_CN.conversation.workflowRun.settingConstraintsLabel}
                    </label>
                    <textarea
                      id="setting-workflow-constraints"
                      maxLength={1_000}
                      onChange={(event) => setSettingConstraints(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.settingConstraintsPlaceholder}
                      rows={3}
                      value={settingConstraints}
                    />
                    <fieldset className="setting-source-picker">
                      <legend>{ZH_CN.conversation.workflowRun.settingSourcesTitle}</legend>
                      <p>{ZH_CN.conversation.workflowRun.settingSourcesBody}</p>
                      <small>
                        {ZH_CN.conversation.workflowRun.settingSourcesCount(
                          selectedSettingSources.length,
                        )}
                      </small>
                      {settingCatalog === null ? (
                        <p>{ZH_CN.conversation.workflowRun.settingSourcesLoading}</p>
                      ) : settingCatalog.sources.length === 0 ? (
                        <p>{ZH_CN.conversation.workflowRun.settingSourcesEmpty}</p>
                      ) : (
                        settingCatalog.sources.map((source) => {
                          const selected = selectedSettingSources.includes(source.relativePath);
                          return (
                            <label key={source.relativePath}>
                              <input
                                checked={selected}
                                disabled={
                                  source.relativePath === settingReplaceTarget ||
                                  (!selected && selectedSettingSources.length >= 6)
                                }
                                onChange={(event) => {
                                  const checked = event.currentTarget.checked;
                                  setSelectedSettingSources((current) =>
                                    checked
                                      ? [...current, source.relativePath].sort()
                                      : current.filter((path) => path !== source.relativePath),
                                  );
                                }}
                                type="checkbox"
                              />
                              <span>
                                <strong>{source.displayName}</strong>
                                <small>
                                  {source.settingType
                                    ? ZH_CN.conversation.workflowRun.settingTypes[
                                        source.settingType
                                      ]
                                    : ""}{" "}
                                  · {source.relativePath}
                                </small>
                              </span>
                            </label>
                          );
                        })
                      )}
                    </fieldset>
                  </>
                ) : workflowFormId === MATERIAL_CREATE_WORKFLOW_ID ? (
                  <>
                    <label htmlFor="material-workflow-operation">
                      {ZH_CN.conversation.workflowRun.materialOperationLabel}
                    </label>
                    <select
                      id="material-workflow-operation"
                      onChange={(event) => {
                        const operation = event.currentTarget.value as "create" | "replace";
                        setMaterialOperation(operation);
                        if (operation === "create") {
                          setMaterialReplaceTarget(null);
                        }
                      }}
                      value={materialOperation}
                    >
                      <option value="create">
                        {ZH_CN.conversation.workflowRun.materialOperations.create}
                      </option>
                      <option value="replace">
                        {ZH_CN.conversation.workflowRun.materialOperations.replace}
                      </option>
                    </select>
                    {materialOperation === "replace" ? (
                      <>
                        <label htmlFor="material-workflow-replace-target">
                          {ZH_CN.conversation.workflowRun.materialReplaceTargetLabel}
                        </label>
                        <select
                          id="material-workflow-replace-target"
                          onChange={(event) => {
                            const relativePath = event.currentTarget.value || null;
                            setMaterialReplaceTarget(relativePath);
                            const target = materialCatalog?.replaceTargets.find(
                              (candidate) => candidate.relativePath === relativePath,
                            );
                            if (target) {
                              setMaterialDisplayName(target.displayName);
                              if (target.materialType) {
                                setMaterialType(target.materialType);
                              }
                            }
                          }}
                          value={materialReplaceTarget ?? ""}
                        >
                          <option value="">
                            {ZH_CN.conversation.workflowRun.materialReplaceTargetPlaceholder}
                          </option>
                          {materialCatalog?.replaceTargets.map((target) => (
                            <option key={target.materialId} value={target.relativePath}>
                              {target.displayName} · {target.relativePath}
                            </option>
                          ))}
                        </select>
                      </>
                    ) : null}
                    <label htmlFor="material-workflow-name">
                      {ZH_CN.conversation.workflowRun.materialDisplayNameLabel}
                    </label>
                    <input
                      autoFocus
                      id="material-workflow-name"
                      maxLength={80}
                      onChange={(event) => setMaterialDisplayName(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.materialDisplayNamePlaceholder}
                      value={materialDisplayName}
                    />
                    <label htmlFor="material-workflow-type">
                      {ZH_CN.conversation.workflowRun.materialTypeLabel}
                    </label>
                    <select
                      id="material-workflow-type"
                      onChange={(event) =>
                        setMaterialType(event.currentTarget.value as MaterialType)
                      }
                      value={materialType}
                    >
                      {(["theme", "clue", "object", "scene_seed"] as const).map((kind) => (
                        <option key={kind} value={kind}>
                          {ZH_CN.conversation.workflowRun.materialTypes[kind]}
                        </option>
                      ))}
                    </select>
                    <label htmlFor="material-workflow-seed">
                      {ZH_CN.conversation.workflowRun.materialSeedLabel}
                    </label>
                    <textarea
                      id="material-workflow-seed"
                      maxLength={2_000}
                      onChange={(event) => setMaterialSeed(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.materialSeedPlaceholder}
                      rows={5}
                      value={materialSeed}
                    />
                    <label htmlFor="material-workflow-intended-use">
                      {ZH_CN.conversation.workflowRun.materialIntendedUseLabel}
                    </label>
                    <textarea
                      id="material-workflow-intended-use"
                      maxLength={1_000}
                      onChange={(event) => setMaterialIntendedUse(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.materialIntendedUsePlaceholder}
                      rows={4}
                      value={materialIntendedUse}
                    />
                    <label htmlFor="material-workflow-constraints">
                      {ZH_CN.conversation.workflowRun.materialConstraintsLabel}
                    </label>
                    <textarea
                      id="material-workflow-constraints"
                      maxLength={1_000}
                      onChange={(event) => setMaterialConstraints(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.materialConstraintsPlaceholder}
                      rows={3}
                      value={materialConstraints}
                    />
                    <fieldset className="setting-source-picker">
                      <legend>{ZH_CN.conversation.workflowRun.materialSourcesTitle}</legend>
                      <p>{ZH_CN.conversation.workflowRun.materialSourcesBody}</p>
                      <small>
                        {ZH_CN.conversation.workflowRun.materialSourcesCount(
                          selectedMaterialSources.length,
                        )}
                      </small>
                      {materialCatalog === null ? (
                        <p>{ZH_CN.conversation.workflowRun.materialSourcesLoading}</p>
                      ) : materialCatalog.sources.length === 0 ? (
                        <p>{ZH_CN.conversation.workflowRun.materialSourcesEmpty}</p>
                      ) : (
                        materialCatalog.sources.map((source) => {
                          const selected = selectedMaterialSources.includes(source.relativePath);
                          const sourceKind =
                            source.sourceKind === "world" ||
                            source.sourceKind === "characters" ||
                            source.sourceKind === "plot"
                              ? source.sourceKind
                              : "plot";
                          return (
                            <label key={source.relativePath}>
                              <input
                                checked={selected}
                                disabled={!selected && selectedMaterialSources.length >= 6}
                                onChange={(event) => {
                                  const checked = event.currentTarget.checked;
                                  setSelectedMaterialSources((current) =>
                                    checked
                                      ? [...current, source.relativePath].sort()
                                      : current.filter((path) => path !== source.relativePath),
                                  );
                                }}
                                type="checkbox"
                              />
                              <span>
                                <strong>{source.displayName}</strong>
                                <small>
                                  {ZH_CN.conversation.workflowRun.materialSourceKinds[sourceKind]} ·{" "}
                                  {source.relativePath}
                                </small>
                              </span>
                            </label>
                          );
                        })
                      )}
                    </fieldset>
                  </>
                ) : (
                  <>
                    <label htmlFor="plot-workflow-operation">
                      {ZH_CN.conversation.workflowRun.plotOperationLabel}
                    </label>
                    <select
                      id="plot-workflow-operation"
                      onChange={(event) => {
                        const operation = event.currentTarget.value as "create" | "replace";
                        setPlotOperation(operation);
                        setPlotReplaceTarget(operation === "replace" ? "plot/main.md" : null);
                      }}
                      value={plotOperation}
                    >
                      <option
                        disabled={(plotCatalog?.replaceTargets.length ?? 0) > 0}
                        value="create"
                      >
                        {ZH_CN.conversation.workflowRun.plotOperations.create}
                      </option>
                      <option
                        disabled={(plotCatalog?.replaceTargets.length ?? 0) === 0}
                        value="replace"
                      >
                        {ZH_CN.conversation.workflowRun.plotOperations.replace}
                      </option>
                    </select>
                    {plotOperation === "replace" ? (
                      <div className="workflow-fixed-target">
                        <strong>{ZH_CN.conversation.workflowRun.plotReplaceTargetLabel}</strong>
                        <span>
                          {plotCatalog?.replaceTargets[0]?.displayName ?? "plot/main.md"} ·
                          plot/main.md
                        </span>
                      </div>
                    ) : null}
                    <label htmlFor="plot-workflow-title">
                      {ZH_CN.conversation.workflowRun.plotTitleLabel}
                    </label>
                    <input
                      autoFocus
                      id="plot-workflow-title"
                      maxLength={120}
                      onChange={(event) => setPlotTitle(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.plotTitlePlaceholder}
                      value={plotTitle}
                    />
                    <label htmlFor="plot-workflow-genre">
                      {ZH_CN.conversation.workflowRun.plotGenreLabel}
                    </label>
                    <input
                      id="plot-workflow-genre"
                      maxLength={80}
                      onChange={(event) => setPlotGenre(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.plotGenrePlaceholder}
                      value={plotGenre}
                    />
                    <label htmlFor="plot-workflow-core-question">
                      {ZH_CN.conversation.workflowRun.plotCoreQuestionLabel}
                    </label>
                    <textarea
                      id="plot-workflow-core-question"
                      maxLength={1_000}
                      onChange={(event) => setPlotCoreQuestion(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.plotCoreQuestionPlaceholder}
                      rows={4}
                      value={plotCoreQuestion}
                    />
                    <label htmlFor="plot-workflow-ending">
                      {ZH_CN.conversation.workflowRun.plotDesiredEndingLabel}
                    </label>
                    <textarea
                      id="plot-workflow-ending"
                      maxLength={2_000}
                      onChange={(event) => setPlotDesiredEnding(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.plotDesiredEndingPlaceholder}
                      rows={5}
                      value={plotDesiredEnding}
                    />
                    <label htmlFor="plot-workflow-must-include">
                      {ZH_CN.conversation.workflowRun.plotMustIncludeLabel}
                    </label>
                    <textarea
                      id="plot-workflow-must-include"
                      onChange={(event) => setPlotMustInclude(event.currentTarget.value)}
                      placeholder={ZH_CN.conversation.workflowRun.plotMustIncludePlaceholder}
                      rows={4}
                      value={plotMustInclude}
                    />
                    <label htmlFor="plot-workflow-forbidden">
                      {ZH_CN.conversation.workflowRun.plotForbiddenDirectionsLabel}
                    </label>
                    <textarea
                      id="plot-workflow-forbidden"
                      onChange={(event) => setPlotForbiddenDirections(event.currentTarget.value)}
                      placeholder={
                        ZH_CN.conversation.workflowRun.plotForbiddenDirectionsPlaceholder
                      }
                      rows={4}
                      value={plotForbiddenDirections}
                    />
                    <fieldset className="setting-source-picker">
                      <legend>{ZH_CN.conversation.workflowRun.plotSourcesTitle}</legend>
                      <p>{ZH_CN.conversation.workflowRun.plotSourcesBody}</p>
                      <small>
                        {ZH_CN.conversation.workflowRun.plotSourcesCount(
                          selectedPlotSources.length,
                        )}
                      </small>
                      {plotCatalog === null ? (
                        <p>{ZH_CN.conversation.workflowRun.plotSourcesLoading}</p>
                      ) : plotCatalog.sources.length === 0 ? (
                        <p>{ZH_CN.conversation.workflowRun.plotSourcesEmpty}</p>
                      ) : (
                        plotCatalog.sources.map((source) => {
                          const selected = selectedPlotSources.includes(source.relativePath);
                          const sourceKind =
                            source.sourceKind === "world" ||
                            source.sourceKind === "characters" ||
                            source.sourceKind === "materials"
                              ? source.sourceKind
                              : "materials";
                          return (
                            <label key={source.relativePath}>
                              <input
                                checked={selected}
                                disabled={!selected && selectedPlotSources.length >= 8}
                                onChange={(event) => {
                                  const checked = event.currentTarget.checked;
                                  setSelectedPlotSources((current) =>
                                    checked
                                      ? [...current, source.relativePath].sort()
                                      : current.filter((path) => path !== source.relativePath),
                                  );
                                }}
                                type="checkbox"
                              />
                              <span>
                                <strong>{source.displayName}</strong>
                                <small>
                                  {ZH_CN.conversation.workflowRun.plotSourceKinds[sourceKind]} ·{" "}
                                  {source.relativePath}
                                </small>
                              </span>
                            </label>
                          );
                        })
                      )}
                    </fieldset>
                  </>
                )}
                <div className="workflow-form-actions">
                  <button
                    disabled={workflowFormInvalid || workflowRunAction !== null}
                    onClick={() => void prepareWorkflowRun()}
                    type="button"
                  >
                    {workflowRunAction === "prepare"
                      ? ZH_CN.conversation.workflowRun.preparing
                      : ZH_CN.conversation.workflowRun.prepare}
                  </button>
                  <button
                    disabled={workflowRunAction !== null}
                    onClick={() => setWorkflowFormOpen(false)}
                    type="button"
                  >
                    {ZH_CN.conversation.workflowRun.closeBrief}
                  </button>
                </div>
              </section>
            ) : null}
            <section
              aria-label={ZH_CN.conversation.workflowRun.ariaLabel}
              className="workflow-run-status"
              data-state={workflowRun?.state ?? "none"}
            >
              {presentedWorkflowError ? (
                <div className="workflow-run-error" role="alert">
                  <strong>{presentedWorkflowError.title}</strong>
                  <span>{presentedWorkflowError.message}</span>
                  <small>{presentedWorkflowError.recovery}</small>
                </div>
              ) : null}
              {!projectSessionId ? (
                <p>{ZH_CN.conversation.workflowRun.noProject}</p>
              ) : workflowRunLoading || !workflowRunStatus ? (
                <p>{ZH_CN.conversation.workflowRun.loading}</p>
              ) : !workflowRunStatus.available ? (
                <div>
                  <strong>{ZH_CN.conversation.workflowRun.unavailableTitle}</strong>
                  <p>{ZH_CN.conversation.workflowRun.unavailableBody}</p>
                </div>
              ) : workflowRun ? (
                <div className="workflow-run-detail">
                  {workflowRepairing ? (
                    <div className="workflow-repair-status" role="status">
                      <strong>{ZH_CN.conversation.workflowRun.repairingTitle}</strong>
                      <span>{ZH_CN.conversation.workflowRun.repairingBody}</span>
                    </div>
                  ) : null}
                  <header>
                    <div>
                      <strong>
                        {workflowRun.recoveryRequired
                          ? ZH_CN.conversation.workflowRun.recoveryTitle
                          : workflowRun.workflowId === SETTING_CREATE_WORKFLOW_ID
                            ? ZH_CN.conversation.workflowRun.settingStatusTitle
                            : workflowRun.workflowId === MATERIAL_CREATE_WORKFLOW_ID
                              ? ZH_CN.conversation.workflowRun.materialStatusTitle
                              : workflowRun.workflowId === PLOT_CREATE_WORKFLOW_ID
                                ? ZH_CN.conversation.workflowRun.plotStatusTitle
                                : ZH_CN.conversation.workflowRun.statusTitle}
                      </strong>
                      <span>{workflowStateLabel}</span>
                    </div>
                  </header>
                  <dl>
                    <div>
                      <dt>{ZH_CN.conversation.workflowRun.stateLabel}</dt>
                      <dd>{workflowStateLabel}</dd>
                    </div>
                    <div>
                      <dt>{ZH_CN.conversation.workflowRun.providerLabel}</dt>
                      <dd>{workflowProviderLabel}</dd>
                    </div>
                  </dl>
                  {workflowRun.state === "context_ready" && workflowManifest ? (
                    <section
                      aria-label={ZH_CN.conversation.workflowRun.manifestAriaLabel}
                      className="context-manifest"
                    >
                      <header>
                        <div>
                          <strong>{ZH_CN.conversation.workflowRun.manifestTitle}</strong>
                          <span>{ZH_CN.conversation.workflowRun.reviewBeforeSend}</span>
                        </div>
                      </header>
                      <dl>
                        <div>
                          <dt>{ZH_CN.conversation.workflowRun.receiverLabel}</dt>
                          <dd>
                            {PROVIDER_PRESENTATION[workflowManifest.providerId].name} ·{" "}
                            {workflowManifest.model}
                          </dd>
                        </div>
                        <div>
                          <dt>
                            {workflowManifest.workflowId === SETTING_CREATE_WORKFLOW_ID
                              ? ZH_CN.conversation.workflowRun.settingBriefSummaryLabel
                              : workflowManifest.workflowId === MATERIAL_CREATE_WORKFLOW_ID
                                ? ZH_CN.conversation.workflowRun.materialBriefSummaryLabel
                                : workflowManifest.workflowId === PLOT_CREATE_WORKFLOW_ID
                                  ? ZH_CN.conversation.workflowRun.plotBriefSummaryLabel
                                  : ZH_CN.conversation.workflowRun.briefSummaryLabel}
                          </dt>
                          <dd>
                            {ZH_CN.conversation.workflowRun.characterCount(
                              workflowManifest.briefCharacterCount,
                            )}{" "}
                            · {shortHash(workflowManifest.briefHash)}
                          </dd>
                        </div>
                        <div>
                          <dt>{ZH_CN.conversation.workflowRun.manifestHashLabel}</dt>
                          <dd title={workflowManifest.manifestHash}>
                            {shortHash(workflowManifest.manifestHash)}
                          </dd>
                        </div>
                      </dl>
                      <div className="manifest-source-list">
                        <strong>{ZH_CN.conversation.workflowRun.sourcesTitle}</strong>
                        {workflowManifest.sources.map((source) => (
                          <div className="manifest-source" key={source.relativePath}>
                            <header>
                              <strong>{source.relativePath}</strong>
                              <span>
                                {
                                  ZH_CN.conversation.workflowRun.inclusionReasons[
                                    source.inclusionReason
                                  ]
                                }{" "}
                                · {ZH_CN.conversation.workflowRun.trustClasses[source.trustClass]}
                              </span>
                            </header>
                            <dl>
                              <div>
                                <dt>{ZH_CN.conversation.workflowRun.includedLabel}</dt>
                                <dd>
                                  {ZH_CN.conversation.workflowRun.sourceCount(
                                    source.includedCharacterCount,
                                    source.includedByteCount,
                                  )}
                                  {source.truncated
                                    ? ` · ${ZH_CN.conversation.workflowRun.truncated}`
                                    : ""}
                                </dd>
                              </div>
                              <div>
                                <dt>{ZH_CN.conversation.workflowRun.sourceHashLabel}</dt>
                                <dd title={source.contentHash}>{shortHash(source.contentHash)}</dd>
                              </div>
                            </dl>
                            <span>{ZH_CN.conversation.workflowRun.previewLabel}</span>
                            <pre>{source.boundedPreview}</pre>
                          </div>
                        ))}
                      </div>
                      <div className="workflow-send-warning">
                        <strong>{ZH_CN.conversation.workflowRun.apiWarningTitle}</strong>
                        <p>{ZH_CN.conversation.workflowRun.apiWarningBody}</p>
                      </div>
                      {!manifestProviderReady ? (
                        <p className="workflow-provider-warning">
                          {ZH_CN.conversation.workflowRun.providerNotReady(
                            PROVIDER_PRESENTATION[workflowManifest.providerId].name,
                          )}
                        </p>
                      ) : null}
                      <button
                        className="workflow-generate-button"
                        disabled={!manifestProviderReady || workflowRunAction !== null}
                        onClick={() => void generateWorkflowRun()}
                        type="button"
                      >
                        {workflowRunAction === "generate"
                          ? ZH_CN.conversation.workflowRun.generating
                          : ZH_CN.conversation.workflowRun.confirmGenerate}
                      </button>
                    </section>
                  ) : null}
                  {workflowProposal ? (
                    <section
                      aria-label={ZH_CN.conversation.workflowRun.proposalAriaLabel}
                      className="workflow-proposal"
                    >
                      <header>
                        <div>
                          <strong>
                            {workflowRun.workflowId === MATERIAL_CREATE_WORKFLOW_ID
                              ? ZH_CN.conversation.workflowRun.materialProposalTitle
                              : workflowRun.workflowId === PLOT_CREATE_WORKFLOW_ID
                                ? ZH_CN.conversation.workflowRun.plotProposalTitle
                                : ZH_CN.conversation.workflowRun.proposalTitle}
                          </strong>
                          <span>{ZH_CN.conversation.workflowRun.proposalSafetyStatus}</span>
                        </div>
                      </header>
                      <dl>
                        <div>
                          <dt>{ZH_CN.conversation.workflowRun.proposalTargetLabel}</dt>
                          <dd>{workflowProposal.operation.relativePath}</dd>
                        </div>
                        <div>
                          <dt>{ZH_CN.conversation.workflowRun.proposalOperationLabel}</dt>
                          <dd>
                            {
                              ZH_CN.conversation.workflowRun.proposalOperationKinds[
                                workflowProposal.operation.kind
                              ]
                            }
                          </dd>
                        </div>
                        <div>
                          <dt>{ZH_CN.conversation.workflowRun.proposalProviderLabel}</dt>
                          <dd>
                            {PROVIDER_PRESENTATION[workflowProposal.providerId].name} ·{" "}
                            {workflowProposal.model}
                          </dd>
                        </div>
                        <div>
                          <dt>{ZH_CN.conversation.workflowRun.proposalSourceLabel}</dt>
                          <dd title={workflowProposal.operation.expectedSourceHash}>
                            {workflowProposal.operation.expectedSourceHash === "absent"
                              ? ZH_CN.conversation.workflowRun.proposalNewFile
                              : shortHash(workflowProposal.operation.expectedSourceHash)}
                          </dd>
                        </div>
                        <div>
                          <dt>{ZH_CN.conversation.workflowRun.proposalValidationLabel}</dt>
                          <dd>{ZH_CN.conversation.workflowRun.proposalValidationPassed}</dd>
                        </div>
                        <div>
                          <dt>{ZH_CN.conversation.workflowRun.proposalHashLabel}</dt>
                          <dd title={workflowProposal.proposalHash}>
                            {shortHash(workflowProposal.proposalHash)}
                          </dd>
                        </div>
                      </dl>
                      {workflowProposal.warningCodes.length > 0 ? (
                        <div className="proposal-warnings">
                          <strong>{ZH_CN.conversation.workflowRun.proposalWarningsTitle}</strong>
                          <ul>
                            {workflowProposal.warningCodes.map((warning) => (
                              <li key={warning}>{warning}</li>
                            ))}
                          </ul>
                        </div>
                      ) : null}
                      <div className="proposal-preview">
                        <strong>{ZH_CN.conversation.workflowRun.proposalFullPreviewTitle}</strong>
                        <small>
                          {ZH_CN.conversation.workflowRun.proposalByteCount(
                            workflowProposal.operation.proposedByteCount,
                          )}
                        </small>
                        <pre>{workflowProposal.fullContent}</pre>
                      </div>
                      {workflowProposal.operation.kind === "replace" ? (
                        <div className="proposal-diff">
                          <strong>{ZH_CN.conversation.workflowRun.proposalDiffTitle}</strong>
                          <div role="table">
                            {workflowProposal.lineDiff.map((line, index) => (
                              <div
                                data-kind={line.kind}
                                key={`${line.kind}:${line.oldLine ?? "-"}:${line.newLine ?? "-"}:${index}`}
                                role="row"
                              >
                                <span>{line.oldLine ?? ""}</span>
                                <span>{line.newLine ?? ""}</span>
                                <code>
                                  {line.kind === "add" ? "+" : line.kind === "remove" ? "−" : " "}
                                  {line.content}
                                </code>
                              </div>
                            ))}
                          </div>
                        </div>
                      ) : null}
                      {workflowRun.state === "awaiting_confirmation" && !confirmingApply ? (
                        <button
                          className="workflow-apply-button"
                          disabled={workflowRunAction !== null}
                          onClick={() => setConfirmingApply(true)}
                          type="button"
                        >
                          {workflowRun.workflowId === MATERIAL_CREATE_WORKFLOW_ID
                            ? ZH_CN.conversation.workflowRun.materialReviewedApply
                            : workflowRun.workflowId === PLOT_CREATE_WORKFLOW_ID
                              ? ZH_CN.conversation.workflowRun.plotReviewedApply
                              : ZH_CN.conversation.workflowRun.reviewedApply}
                        </button>
                      ) : null}
                      {workflowRun.state === "awaiting_confirmation" && confirmingApply ? (
                        <div className="workflow-apply-confirmation" role="group">
                          <strong>
                            {workflowRun.workflowId === MATERIAL_CREATE_WORKFLOW_ID
                              ? ZH_CN.conversation.workflowRun.materialApplyConfirmationTitle
                              : workflowRun.workflowId === PLOT_CREATE_WORKFLOW_ID
                                ? ZH_CN.conversation.workflowRun.plotApplyConfirmationTitle
                                : ZH_CN.conversation.workflowRun.applyConfirmationTitle}
                          </strong>
                          <p>
                            {ZH_CN.conversation.workflowRun.applyConfirmationBody(
                              workflowProposal.operation.relativePath,
                            )}
                          </p>
                          <div>
                            <button
                              disabled={workflowRunAction !== null}
                              onClick={() => void applyWorkflowProposal()}
                              type="button"
                            >
                              {workflowRunAction === "apply"
                                ? ZH_CN.conversation.workflowRun.applying
                                : workflowRun.workflowId === MATERIAL_CREATE_WORKFLOW_ID
                                  ? ZH_CN.conversation.workflowRun.confirmMaterialCanon
                                  : workflowRun.workflowId === PLOT_CREATE_WORKFLOW_ID
                                    ? ZH_CN.conversation.workflowRun.confirmPlotWrite
                                    : ZH_CN.conversation.workflowRun.confirmApply}
                            </button>
                            <button
                              disabled={workflowRunAction !== null}
                              onClick={() => setConfirmingApply(false)}
                              type="button"
                            >
                              {ZH_CN.conversation.workflowRun.keepReviewing}
                            </button>
                          </div>
                        </div>
                      ) : null}
                    </section>
                  ) : null}
                  {workflowRun.recoveryRequired ? (
                    <p className="workflow-recovery-copy">
                      {ZH_CN.conversation.workflowRun.recoveryBody}
                    </p>
                  ) : null}
                  <small>{ZH_CN.conversation.workflowRun.checkpointOnly}</small>
                  {workflowRun.canCancel ? (
                    <button
                      disabled={workflowRunAction !== null}
                      onClick={() => void cancelWorkflowRun()}
                      type="button"
                    >
                      {workflowRunAction === "cancel"
                        ? ZH_CN.conversation.workflowRun.cancelling
                        : ZH_CN.conversation.workflowRun.cancel}
                    </button>
                  ) : workflowRun.canAbandon && !confirmingAbandon ? (
                    <button
                      disabled={workflowRunAction !== null}
                      onClick={() => setConfirmingAbandon(true)}
                      type="button"
                    >
                      {workflowRun.workflowId === MATERIAL_CREATE_WORKFLOW_ID
                        ? ZH_CN.conversation.workflowRun.materialAbandon
                        : workflowRun.workflowId === PLOT_CREATE_WORKFLOW_ID
                          ? ZH_CN.conversation.workflowRun.plotAbandon
                          : ZH_CN.conversation.workflowRun.abandon}
                    </button>
                  ) : null}
                  {workflowRun.canAbandon && confirmingAbandon ? (
                    <div className="workflow-abandon-confirmation" role="group">
                      <p>
                        {workflowRun.workflowId === MATERIAL_CREATE_WORKFLOW_ID
                          ? ZH_CN.conversation.workflowRun.materialAbandonPrompt
                          : workflowRun.workflowId === PLOT_CREATE_WORKFLOW_ID
                            ? ZH_CN.conversation.workflowRun.plotAbandonPrompt
                            : ZH_CN.conversation.workflowRun.abandonPrompt}
                      </p>
                      <div>
                        <button
                          disabled={workflowRunAction !== null}
                          onClick={() => void abandonWorkflowRun()}
                          type="button"
                        >
                          {workflowRunAction === "abandon"
                            ? ZH_CN.conversation.workflowRun.abandoning
                            : ZH_CN.conversation.workflowRun.confirmAbandon}
                        </button>
                        <button
                          disabled={workflowRunAction !== null}
                          onClick={() => setConfirmingAbandon(false)}
                          type="button"
                        >
                          {ZH_CN.conversation.workflowRun.keepRun}
                        </button>
                      </div>
                    </div>
                  ) : null}
                </div>
              ) : (
                <div>
                  <strong>{ZH_CN.conversation.workflowRun.idleTitle}</strong>
                  <p>{ZH_CN.conversation.workflowRun.idleBody}</p>
                </div>
              )}
            </section>
          </section>
          {routerSurface.kind !== "idle" ? (
            <section aria-label={ZH_CN.conversation.router.ariaLabel} className="router-surface">
              <header>
                <div>
                  <span>{ZH_CN.conversation.router.kicker}</span>
                  <strong>
                    {routerSurface.kind === "consent"
                      ? ZH_CN.conversation.router.consentTitle
                      : routerSurface.kind === "running"
                        ? ZH_CN.conversation.router.runningTitle
                        : ZH_CN.conversation.router.selectorTitle}
                  </strong>
                </div>
                {routerSurface.kind !== "running" ? (
                  <button onClick={clearRouterSurface} type="button">
                    {ZH_CN.conversation.router.close}
                  </button>
                ) : null}
              </header>
              {presentedRouterError ? (
                <div className="conversation-error" role="alert">
                  <strong>{presentedRouterError.title}</strong>
                  <span>{presentedRouterError.message}</span>
                </div>
              ) : null}
              {routerSurface.kind === "consent" ? (
                <>
                  <p>{ZH_CN.conversation.router.consentBody}</p>
                  <dl>
                    <div>
                      <dt>{ZH_CN.conversation.router.receiverLabel}</dt>
                      <dd>{selectedPresentation.name}</dd>
                    </div>
                    <div>
                      <dt>{ZH_CN.conversation.router.modelLabel}</dt>
                      <dd>{selectedPresentation.model}</dd>
                    </div>
                    <div>
                      <dt>{ZH_CN.conversation.router.scopeLabel}</dt>
                      <dd>{ZH_CN.conversation.router.scopeValue}</dd>
                    </div>
                  </dl>
                  <small>{ZH_CN.conversation.router.usageWarning}</small>
                  <div className="router-actions">
                    <button
                      disabled={routerStarting || !providerReady}
                      onClick={() => void confirmRouterProvider()}
                      type="button"
                    >
                      {routerStarting
                        ? ZH_CN.conversation.router.starting
                        : ZH_CN.conversation.router.confirm}
                    </button>
                    <button disabled={routerStarting} onClick={clearRouterSurface} type="button">
                      {ZH_CN.conversation.router.cancel}
                    </button>
                  </div>
                </>
              ) : routerSurface.kind === "running" ? (
                <>
                  <p>{ZH_CN.conversation.router.runningBody}</p>
                  <small>
                    {selectedPresentation.name} · {selectedPresentation.model}
                  </small>
                  <div className="router-actions">
                    <button
                      disabled={routerCancelling}
                      onClick={() => void cancelRouterRun()}
                      type="button"
                    >
                      {routerCancelling
                        ? ZH_CN.conversation.router.cancelling
                        : ZH_CN.conversation.router.cancelRun}
                    </button>
                  </div>
                </>
              ) : (
                <>
                  <p>
                    {routerSurface.reason === "confirmation"
                      ? ZH_CN.conversation.router.confirmationBody(
                          ZH_CN.conversation.router.confidenceLabels[
                            routerSurface.confidence ?? "medium"
                          ],
                        )
                      : routerSurface.reason === "cancelled"
                        ? ZH_CN.conversation.router.cancelledBody
                        : routerSurface.reason === "local"
                          ? ZH_CN.conversation.router.localBody
                          : ZH_CN.conversation.router.unavailableBody}
                  </p>
                  {routerSurface.usage ? (
                    <small>
                      {ZH_CN.conversation.router.usage(
                        routerSurface.usage.inputTokens,
                        routerSurface.usage.outputTokens,
                        routerSurface.usage.totalTokens,
                      )}
                    </small>
                  ) : null}
                  <div className="router-workflow-selector">
                    {workflowRegistry?.workflows.map((workflow) => {
                      const projectCreate = workflow.id === PROJECT_CREATE_WORKFLOW_ID;
                      const unavailable = projectCreate
                        ? projectSessionId !== null || !onProjectCreateRequested
                        : !projectSessionId || workflowRunActive || workflowRunAction !== null;
                      return (
                        <button
                          disabled={
                            !workflow.executionAvailable || !workflow.routable || unavailable
                          }
                          key={workflow.id}
                          onClick={() => selectRouterWorkflow(workflow.id as RoutableWorkflowId)}
                          type="button"
                        >
                          {workflow.displayName}
                        </button>
                      );
                    })}
                    <button
                      disabled={!providerReady || Boolean(activeRun) || starting}
                      onClick={sendRouterTextAsChat}
                      type="button"
                    >
                      {ZH_CN.conversation.router.ordinaryChat}
                    </button>
                  </div>
                  <small>{ZH_CN.conversation.router.stopBoundary}</small>
                </>
              )}
            </section>
          ) : null}
          {conversation.messages.length === 0 ? (
            <div className="empty-conversation">
              <span aria-hidden="true" className="empty-icon">
                {ZH_CN.conversation.icon}
              </span>
              <p className="empty-eyebrow">{ZH_CN.conversation.ready}</p>
              <h2>{ZH_CN.conversation.title}</h2>
              <p>{ZH_CN.conversation.enabled}</p>
              <div className="capability-note">
                <strong>{ZH_CN.conversation.boundaryTitle}</strong>
                <span>{ZH_CN.conversation.boundaryBody}</span>
              </div>
            </div>
          ) : (
            <div className="conversation-message-list">
              {conversation.messages.map((message) => (
                <MessageCard
                  key={message.id}
                  message={message}
                  onRetry={
                    message.role === "assistant" && message.status === "failed"
                      ? () => retry(message)
                      : undefined
                  }
                  retryDisabled={
                    Boolean(activeRun) ||
                    starting ||
                    workflowRunActive ||
                    !providerReady ||
                    selectedProvider.providerId !== message.providerId
                  }
                />
              ))}
            </div>
          )}
        </div>
      </section>

      <section aria-label={ZH_CN.conversation.composerAriaLabel} className="composer-shell">
        {presentedStartError ? (
          <div className="composer-error" role="alert">
            <strong>{presentedStartError.title}</strong>
            <span>{presentedStartError.message}</span>
          </div>
        ) : null}
        <textarea
          aria-label={ZH_CN.conversation.inputAriaLabel}
          disabled={
            !desktopRuntime ||
            Boolean(activeRun) ||
            starting ||
            routerSurface.kind !== "idle" ||
            workflowRunActive
          }
          onChange={(event) => setInput(event.currentTarget.value)}
          placeholder={ZH_CN.conversation.placeholder}
          rows={3}
          value={input}
        />
        <div className="composer-footer">
          <span>{footerStatus}</span>
          {activeRun ? (
            <button
              className="send-button stop-button"
              disabled={cancelling}
              onClick={() => void cancel()}
              type="button"
            >
              {cancelling ? ZH_CN.conversation.stopping : ZH_CN.conversation.stop}
            </button>
          ) : (
            <div className="composer-actions">
              <button
                className="route-button"
                disabled={!canIdentifyWorkflow}
                onClick={() => void identifyWorkflow()}
                type="button"
              >
                {ZH_CN.conversation.router.identify}
              </button>
              <button className="send-button" disabled={!canSend} onClick={send} type="button">
                {ZH_CN.conversation.send}
                <span aria-hidden="true">↑</span>
              </button>
            </div>
          )}
        </div>
      </section>
    </>
  );
}
