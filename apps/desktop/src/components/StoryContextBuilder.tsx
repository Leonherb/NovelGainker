import { useEffect, useMemo, useState } from "react";
import { presentAppError } from "../errors/present-app-error";
import { ZH_CN } from "../i18n/zh-CN";
import type { AppError } from "../lib/tauri/health";
import {
  requestStoryChapterCancel,
  requestStoryChapterGenerateStart,
  requestStoryChapterStatus,
  type StoryChapterRun,
} from "../lib/tauri/story-chapter";
import {
  requestStoryChapterPatchApply,
  requestStoryChapterPatchPreview,
  requestStoryChapterPatchStatus,
  type ChapterPatchApplyResult,
  type ChapterPatchPreview,
} from "../lib/tauri/story-chapter-patch";
import {
  requestStoryContextBuild,
  requestStoryContextCatalog,
  requestStoryContextRequestPreview,
  type StoryAction,
  type StoryContextCatalogEntry,
  type StoryContextCatalogSnapshot,
  type StoryContextManifest,
  type StoryContextManifestSource,
  type StoryContextRequestSnapshot,
  type StoryContextSelectionReason,
  type StoryContextSourceKind,
  type StoryReaderMentalModel,
} from "../lib/tauri/story-context";
import {
  requestStoryContinuityCheck,
  requestStoryContinuityStatus,
  requestStoryContinuityWarningConfirm,
  type StoryContinuityReport,
} from "../lib/tauri/story-continuity";
import {
  requestStoryChapterLifecycleStatus,
  requestStoryChapterStateApply,
  requestStoryChapterStatePreview,
  requestStoryLedgerUpdateApply,
  requestStoryLedgerUpdatePreview,
  type ChapterStatusPreview,
  type LedgerUpdatePreview,
  type StoryChapterLifecycleStatus,
} from "../lib/tauri/story-lifecycle";

type StoryContextBuilderProps = {
  sessionId: string;
  onClose: () => void;
};

const copy = ZH_CN.conversation.storyContext;
const EMPTY_READER_MODEL: StoryReaderMentalModel = {
  readerKnows: "",
  readerInfers: "",
  readerMisreads: "",
  readerQuestions: "",
  chapterPromises: "",
  forbiddenReveals: "",
};
const READER_MODEL_FIELDS = [
  "readerKnows",
  "readerInfers",
  "readerMisreads",
  "readerQuestions",
  "chapterPromises",
  "forbiddenReveals",
] as const satisfies readonly (keyof StoryReaderMentalModel)[];

function shortHash(value: string): string {
  return `${value.slice(0, 10)}…${value.slice(-8)}`;
}

function formatTimestamp(value: number | null): string {
  return value === null
    ? copy.pendingTimestamp
    : new Date(value * 1_000).toLocaleString("zh-CN", { hour12: false });
}

function sourceLocation(source: {
  relativePath: string | null;
  builtinResourceId: string | null;
}): string {
  return source.relativePath ?? source.builtinResourceId ?? copy.unknownSource;
}

function sourceKind(kind: StoryContextSourceKind): string {
  return copy.sourceKinds[kind];
}

function selectionReason(reason: StoryContextSelectionReason): string {
  return copy.selectionReasons[reason];
}

function SourceMetadata({ source }: { source: StoryContextCatalogEntry }) {
  return (
    <span>
      <strong>{source.displayName}</strong>
      <small>
        {sourceKind(source.kind)} · {sourceLocation(source)}
      </small>
      <small>
        {selectionReason(source.selectionReason)} · {copy.bytes(source.byteLength)} · Hash{" "}
        <code title={source.contentHash}>{shortHash(source.contentHash)}</code>
      </small>
    </span>
  );
}

function ManifestSource({ source }: { source: StoryContextManifestSource }) {
  return (
    <article className="story-manifest-source" data-included={source.included}>
      <header>
        <div>
          <strong>{source.displayName}</strong>
          <span>{sourceKind(source.kind)}</span>
        </div>
        <small>{source.included ? copy.included : copy.excluded}</small>
      </header>
      <p>{sourceLocation(source)}</p>
      <dl>
        <div>
          <dt>{copy.reasonLabel}</dt>
          <dd>{selectionReason(source.selectionReason)}</dd>
        </div>
        <div>
          <dt>{copy.priorityLabel}</dt>
          <dd>{source.priority}</dd>
        </div>
        <div>
          <dt>{copy.byteLabel}</dt>
          <dd>{copy.includedBytes(source.includedByteLength, source.originalByteLength)}</dd>
        </div>
        <div>
          <dt>{copy.hashLabel}</dt>
          <dd title={source.contentHash}>{shortHash(source.contentHash)}</dd>
        </div>
      </dl>
      {source.truncation ? (
        <small className="story-truncation">
          {copy.truncationReasons[source.truncation.reason]}
        </small>
      ) : null}
    </article>
  );
}

export function StoryContextBuilder({ sessionId, onClose }: StoryContextBuilderProps) {
  const [catalog, setCatalog] = useState<StoryContextCatalogSnapshot | null>(null);
  const [catalogLoading, setCatalogLoading] = useState(true);
  const [catalogError, setCatalogError] = useState<AppError | null>(null);
  const [action, setAction] = useState<StoryAction>("create");
  const [chapterNumber, setChapterNumber] = useState(1);
  const [chapterTitle, setChapterTitle] = useState("");
  const [selectedPaths, setSelectedPaths] = useState<string[]>([]);
  const [manifest, setManifest] = useState<StoryContextManifest | null>(null);
  const [building, setBuilding] = useState(false);
  const [buildError, setBuildError] = useState<AppError | null>(null);
  const [readerModel, setReaderModel] = useState<StoryReaderMentalModel>({
    ...EMPTY_READER_MODEL,
  });
  const [requestSnapshot, setRequestSnapshot] = useState<StoryContextRequestSnapshot | null>(null);
  const [previewingRequest, setPreviewingRequest] = useState(false);
  const [previewError, setPreviewError] = useState<AppError | null>(null);
  const [chapterRun, setChapterRun] = useState<StoryChapterRun | null>(null);
  const [chapterError, setChapterError] = useState<AppError | null>(null);
  const [continuityReport, setContinuityReport] = useState<StoryContinuityReport | null>(null);
  const [continuityChecking, setContinuityChecking] = useState(false);
  const [continuityError, setContinuityError] = useState<AppError | null>(null);
  const [patchPreview, setPatchPreview] = useState<ChapterPatchPreview | null>(null);
  const [patchApplied, setPatchApplied] = useState<ChapterPatchApplyResult | null>(null);
  const [patchPreviewing, setPatchPreviewing] = useState(false);
  const [patchApplying, setPatchApplying] = useState(false);
  const [patchError, setPatchError] = useState<AppError | null>(null);
  const [acceptedOrFinalUnlock, setAcceptedOrFinalUnlock] = useState(false);
  const [lifecycleStatus, setLifecycleStatus] = useState<StoryChapterLifecycleStatus | null>(null);
  const [statePreview, setStatePreview] = useState<ChapterStatusPreview | null>(null);
  const [stateApplied, setStateApplied] = useState<ChapterPatchApplyResult | null>(null);
  const [statePreviewing, setStatePreviewing] = useState(false);
  const [stateApplying, setStateApplying] = useState(false);
  const [ledgerPreview, setLedgerPreview] = useState<LedgerUpdatePreview | null>(null);
  const [ledgerApplied, setLedgerApplied] = useState<ChapterPatchApplyResult | null>(null);
  const [ledgerPreviewing, setLedgerPreviewing] = useState(false);
  const [ledgerApplying, setLedgerApplying] = useState(false);
  const [ledgerDeferred, setLedgerDeferred] = useState(false);
  const [lifecycleError, setLifecycleError] = useState<AppError | null>(null);

  const clearLifecycle = () => {
    setLifecycleStatus(null);
    setStatePreview(null);
    setStateApplied(null);
    setStatePreviewing(false);
    setStateApplying(false);
    setLedgerPreview(null);
    setLedgerApplied(null);
    setLedgerPreviewing(false);
    setLedgerApplying(false);
    setLedgerDeferred(false);
    setLifecycleError(null);
  };

  const clearPatch = () => {
    setPatchPreview(null);
    setPatchApplied(null);
    setPatchError(null);
    setAcceptedOrFinalUnlock(false);
    clearLifecycle();
  };

  useEffect(() => {
    let active = true;
    void requestStoryContextCatalog(sessionId)
      .then((value) => {
        if (!active) {
          return;
        }
        setCatalog(value);
        setChapterNumber(value.suggestedChapterNumber);
        setSelectedPaths(
          value.sources
            .filter(
              (source) =>
                !source.required && source.defaultSelected && source.relativePath !== null,
            )
            .map((source) => source.relativePath as string)
            .sort(),
        );
      })
      .catch((error: unknown) => {
        if (active) {
          setCatalogError(error as AppError);
        }
      })
      .finally(() => {
        if (active) {
          setCatalogLoading(false);
        }
      });
    return () => {
      active = false;
    };
  }, [sessionId]);

  useEffect(() => {
    const target = patchApplied?.file.relativePath;
    if (target === undefined) return;
    let active = true;
    void requestStoryChapterLifecycleStatus(sessionId, target)
      .then((status) => {
        if (!active) return;
        setLifecycleStatus(status);
        setStatePreview(status.statusPreview);
        setStateApplied(status.statusApplied);
        setLedgerPreview(status.ledgerPreview);
        setLedgerApplied(status.ledgerApplied);
      })
      .catch((error: unknown) => {
        if (active) setLifecycleError(error as AppError);
      });
    return () => {
      active = false;
    };
  }, [patchApplied?.file.relativePath, sessionId]);

  useEffect(() => {
    let active = true;
    void requestStoryChapterPatchStatus(sessionId)
      .then((status) => {
        if (active) {
          setPatchPreview(status.preview);
          setPatchApplied(status.applied);
        }
      })
      .catch((error: unknown) => {
        if (active) setPatchError(error as AppError);
      });
    return () => {
      active = false;
    };
  }, [sessionId]);

  useEffect(() => {
    let active = true;
    void requestStoryContinuityStatus(sessionId)
      .then((snapshot) => {
        if (active) setContinuityReport(snapshot.report);
      })
      .catch((error: unknown) => {
        if (active) setContinuityError(error as AppError);
      });
    return () => {
      active = false;
    };
  }, [sessionId]);

  useEffect(() => {
    let active = true;
    void requestStoryChapterStatus(sessionId)
      .then((snapshot) => {
        if (active) {
          setChapterRun(snapshot.run);
        }
      })
      .catch((error: unknown) => {
        if (active) {
          setChapterError(error as AppError);
        }
      });
    return () => {
      active = false;
    };
  }, [sessionId]);

  useEffect(() => {
    if (chapterRun?.state !== "generating" && chapterRun?.state !== "repairing") {
      return;
    }
    let active = true;
    let polling = false;
    const timer = window.setInterval(() => {
      if (polling) {
        return;
      }
      polling = true;
      void requestStoryChapterStatus(sessionId)
        .then((snapshot) => {
          if (active) {
            setChapterRun(snapshot.run);
          }
        })
        .catch((error: unknown) => {
          if (active) {
            setChapterError(error as AppError);
          }
        })
        .finally(() => {
          polling = false;
        });
    }, 350);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [chapterRun?.runId, chapterRun?.state, sessionId]);

  const requiredSources = useMemo(
    () => catalog?.sources.filter((source) => source.required) ?? [],
    [catalog],
  );
  const optionalSources = useMemo(
    () => catalog?.sources.filter((source) => !source.required) ?? [],
    [catalog],
  );
  const presentedCatalogError = catalogError ? presentAppError(catalogError) : null;
  const presentedBuildError = buildError ? presentAppError(buildError) : null;
  const presentedPreviewError = previewError ? presentAppError(previewError) : null;
  const presentedChapterError = chapterError ? presentAppError(chapterError) : null;
  const presentedContinuityError = continuityError ? presentAppError(continuityError) : null;
  const presentedPatchError = patchError ? presentAppError(patchError) : null;
  const presentedLifecycleError = lifecycleError ? presentAppError(lifecycleError) : null;
  const chapterValid =
    Number.isSafeInteger(chapterNumber) && chapterNumber >= 1 && chapterNumber <= 999_999;
  const chapterActive = chapterRun?.state === "generating" || chapterRun?.state === "repairing";
  const canBuild =
    catalog !== null &&
    chapterValid &&
    chapterTitle.trim().length > 0 &&
    !building &&
    !chapterActive;
  const canPreviewRequest = manifest !== null && !previewingRequest && !chapterActive;
  const canGenerateChapter = requestSnapshot?.sourceReview.status === "current" && !chapterActive;
  const canCheckContinuity =
    chapterRun?.state === "completed" &&
    chapterRun.proposal !== null &&
    requestSnapshot !== null &&
    !continuityChecking;
  const canPreviewPatch =
    chapterRun?.state === "completed" &&
    chapterRun.proposal !== null &&
    requestSnapshot !== null &&
    continuityReport?.proposalHash === chapterRun.proposal.proposalHash &&
    continuityReport.reviewReady &&
    !patchPreviewing &&
    !patchApplying;
  const nextLifecycleStatus =
    lifecycleStatus?.chapterStatus === "draft"
      ? "accepted"
      : lifecycleStatus?.chapterStatus === "accepted"
        ? "final"
        : null;
  const pendingLedgerCount =
    lifecycleStatus?.ledgerPending.filter((item) => item.resolution === "pending").length ?? 0;

  const toggleSource = (relativePath: string, checked: boolean) => {
    setManifest(null);
    setRequestSnapshot(null);
    setPreviewError(null);
    setChapterRun(null);
    setChapterError(null);
    setContinuityReport(null);
    setContinuityError(null);
    clearPatch();
    setSelectedPaths((current) =>
      checked
        ? [...new Set([...current, relativePath])].sort()
        : current.filter((path) => path !== relativePath),
    );
  };

  const buildManifest = async () => {
    if (!canBuild) {
      return;
    }
    setBuilding(true);
    setBuildError(null);
    setChapterRun(null);
    setChapterError(null);
    setContinuityReport(null);
    setContinuityError(null);
    clearPatch();
    try {
      setManifest(
        await requestStoryContextBuild(sessionId, {
          action,
          chapterNumber,
          chapterTitle,
          selectedSourcePaths: selectedPaths,
        }),
      );
      setRequestSnapshot(null);
      setPreviewError(null);
    } catch (error: unknown) {
      setBuildError(error as AppError);
      setManifest(null);
    } finally {
      setBuilding(false);
    }
  };

  const updateReaderModel = (field: keyof StoryReaderMentalModel, value: string) => {
    setReaderModel((current) => ({ ...current, [field]: value }));
    setRequestSnapshot(null);
    setPreviewError(null);
    setChapterRun(null);
    setChapterError(null);
    setContinuityReport(null);
    setContinuityError(null);
    clearPatch();
  };

  const previewRequest = async () => {
    if (!canPreviewRequest || manifest === null) {
      return;
    }
    setPreviewingRequest(true);
    setPreviewError(null);
    setChapterRun(null);
    setChapterError(null);
    setContinuityReport(null);
    setContinuityError(null);
    clearPatch();
    try {
      const snapshot = await requestStoryContextRequestPreview(sessionId, {
        action,
        chapterNumber,
        chapterTitle,
        selectedSourcePaths: selectedPaths,
        reviewedManifestHash: manifest.aggregateHash,
        readerMentalModel: readerModel,
      });
      setManifest(snapshot.manifest);
      setReaderModel(snapshot.readerMentalModel);
      setRequestSnapshot(snapshot);
    } catch (error: unknown) {
      setPreviewError(error as AppError);
      setRequestSnapshot(null);
    } finally {
      setPreviewingRequest(false);
    }
  };

  const generateChapter = async (expectedProviderId?: StoryChapterRun["providerId"]) => {
    if (!canGenerateChapter || requestSnapshot === null) {
      return;
    }
    setChapterError(null);
    setContinuityReport(null);
    setContinuityError(null);
    clearPatch();
    try {
      setChapterRun(
        await requestStoryChapterGenerateStart(sessionId, {
          action,
          chapterNumber,
          chapterTitle,
          selectedSourcePaths: selectedPaths,
          reviewedManifestHash: requestSnapshot.manifest.aggregateHash,
          readerMentalModel: requestSnapshot.readerMentalModel,
          expectedRequestHash: requestSnapshot.requestHash,
          ...(expectedProviderId === undefined ? {} : { expectedProviderId }),
        }),
      );
    } catch (error: unknown) {
      setChapterError(error as AppError);
    }
  };

  const cancelChapter = async () => {
    if (!chapterActive || chapterRun === null) {
      return;
    }
    setChapterError(null);
    try {
      setChapterRun(await requestStoryChapterCancel(sessionId, chapterRun.runId));
    } catch (error: unknown) {
      setChapterError(error as AppError);
    }
  };

  const checkContinuity = async () => {
    if (!canCheckContinuity || chapterRun?.proposal === null || requestSnapshot === null) return;
    setContinuityChecking(true);
    setContinuityError(null);
    clearPatch();
    try {
      setContinuityReport(
        await requestStoryContinuityCheck(sessionId, {
          runId: chapterRun.runId,
          proposalHash: chapterRun.proposal.proposalHash,
          action,
          chapterNumber,
          chapterTitle,
          selectedSourcePaths: selectedPaths,
          reviewedManifestHash: requestSnapshot.manifest.aggregateHash,
          readerMentalModel: requestSnapshot.readerMentalModel,
        }),
      );
    } catch (error: unknown) {
      setContinuityError(error as AppError);
    } finally {
      setContinuityChecking(false);
    }
  };

  const confirmContinuityWarnings = async () => {
    if (continuityReport === null || chapterRun === null) return;
    setContinuityError(null);
    try {
      setContinuityReport(
        await requestStoryContinuityWarningConfirm(sessionId, chapterRun.runId, continuityReport),
      );
    } catch (error: unknown) {
      setContinuityError(error as AppError);
    }
  };

  const previewChapterPatch = async () => {
    if (
      !canPreviewPatch ||
      chapterRun?.proposal === null ||
      chapterRun === null ||
      requestSnapshot === null ||
      continuityReport === null
    )
      return;
    setPatchPreviewing(true);
    setPatchError(null);
    setPatchApplied(null);
    try {
      setPatchPreview(
        await requestStoryChapterPatchPreview(sessionId, {
          runId: chapterRun.runId,
          proposalHash: chapterRun.proposal.proposalHash,
          continuityReportHash: continuityReport.reportHash,
          action,
          chapterNumber,
          chapterTitle,
          selectedSourcePaths: selectedPaths,
          reviewedManifestHash: requestSnapshot.manifest.aggregateHash,
          readerMentalModel: requestSnapshot.readerMentalModel,
          acceptedOrFinalUnlockConfirmed: acceptedOrFinalUnlock,
        }),
      );
    } catch (error: unknown) {
      setPatchError(error as AppError);
      setPatchPreview(null);
    } finally {
      setPatchPreviewing(false);
    }
  };

  const applyChapterPatch = async () => {
    if (patchPreview === null || chapterRun === null || patchApplying || patchApplied !== null)
      return;
    setPatchApplying(true);
    setPatchError(null);
    try {
      setPatchApplied(
        await requestStoryChapterPatchApply(sessionId, chapterRun.runId, patchPreview),
      );
    } catch (error: unknown) {
      setPatchError(error as AppError);
    } finally {
      setPatchApplying(false);
    }
  };

  const refreshLifecycle = async (targetRelativePath: string) => {
    const status = await requestStoryChapterLifecycleStatus(sessionId, targetRelativePath);
    setLifecycleStatus(status);
    setStatePreview(status.statusPreview);
    setStateApplied(status.statusApplied);
    setLedgerPreview(status.ledgerPreview);
    setLedgerApplied(status.ledgerApplied);
    return status;
  };

  const previewChapterState = async () => {
    const targetRelativePath = patchApplied?.file.relativePath;
    if (
      targetRelativePath === undefined ||
      nextLifecycleStatus === null ||
      statePreviewing ||
      stateApplying
    )
      return;
    const binding =
      nextLifecycleStatus === "accepted" &&
      chapterRun?.state === "completed" &&
      chapterRun.proposal !== null
        ? { runId: chapterRun.runId, proposalHash: chapterRun.proposal.proposalHash }
        : null;
    if (nextLifecycleStatus === "accepted" && binding === null) return;
    setStatePreviewing(true);
    setLifecycleError(null);
    setStateApplied(null);
    setLedgerPreview(null);
    setLedgerApplied(null);
    setLedgerDeferred(false);
    try {
      setStatePreview(
        await requestStoryChapterStatePreview(
          sessionId,
          targetRelativePath,
          nextLifecycleStatus,
          binding,
        ),
      );
    } catch (error: unknown) {
      setLifecycleError(error as AppError);
      setStatePreview(null);
    } finally {
      setStatePreviewing(false);
    }
  };

  const applyChapterState = async () => {
    if (statePreview === null || stateApplying || stateApplied !== null) return;
    setStateApplying(true);
    setLifecycleError(null);
    try {
      const applied = await requestStoryChapterStateApply(sessionId, statePreview);
      setStateApplied(applied);
      await refreshLifecycle(applied.file.relativePath);
    } catch (error: unknown) {
      setLifecycleError(error as AppError);
    } finally {
      setStateApplying(false);
    }
  };

  const previewLedgerUpdate = async () => {
    const targetRelativePath = patchApplied?.file.relativePath;
    if (
      targetRelativePath === undefined ||
      pendingLedgerCount === 0 ||
      ledgerPreviewing ||
      ledgerApplying
    )
      return;
    setLedgerPreviewing(true);
    setLifecycleError(null);
    setLedgerApplied(null);
    setLedgerDeferred(false);
    try {
      setLedgerPreview(await requestStoryLedgerUpdatePreview(sessionId, targetRelativePath));
    } catch (error: unknown) {
      setLifecycleError(error as AppError);
      setLedgerPreview(null);
    } finally {
      setLedgerPreviewing(false);
    }
  };

  const applyLedgerUpdate = async () => {
    if (ledgerPreview === null || ledgerApplying || ledgerApplied !== null) return;
    setLedgerApplying(true);
    setLifecycleError(null);
    try {
      const applied = await requestStoryLedgerUpdateApply(sessionId, ledgerPreview);
      setLedgerApplied(applied);
      await refreshLifecycle(ledgerPreview.chapterRelativePath);
    } catch (error: unknown) {
      setLifecycleError(error as AppError);
    } finally {
      setLedgerApplying(false);
    }
  };

  return (
    <section aria-label={copy.ariaLabel} className="story-context-builder">
      <header>
        <div>
          <span>{copy.kicker}</span>
          <strong>{copy.title}</strong>
        </div>
        <button onClick={onClose} type="button">
          {copy.close}
        </button>
      </header>
      <p>{copy.boundary}</p>
      {presentedCatalogError ? (
        <div className="conversation-error" role="alert">
          <strong>{presentedCatalogError.title}</strong>
          <span>{presentedCatalogError.message}</span>
          <small>{presentedCatalogError.recovery}</small>
        </div>
      ) : null}
      {catalogLoading ? (
        <p>{copy.loading}</p>
      ) : catalog ? (
        <>
          <div className="story-target-grid">
            <label>
              {copy.actionLabel}
              <select
                disabled={chapterActive}
                onChange={(event) => {
                  setAction(event.currentTarget.value as StoryAction);
                  setManifest(null);
                  setRequestSnapshot(null);
                  setPreviewError(null);
                  setChapterRun(null);
                  setChapterError(null);
                  setContinuityReport(null);
                  setContinuityError(null);
                  clearPatch();
                }}
                value={action}
              >
                <option value="create">{copy.actions.create}</option>
                <option value="revise">{copy.actions.revise}</option>
              </select>
            </label>
            <label>
              {copy.chapterNumberLabel}
              <input
                disabled={chapterActive}
                max={999_999}
                min={1}
                onChange={(event) => {
                  setChapterNumber(event.currentTarget.valueAsNumber);
                  setManifest(null);
                  setRequestSnapshot(null);
                  setPreviewError(null);
                  setChapterRun(null);
                  setChapterError(null);
                  setContinuityReport(null);
                  setContinuityError(null);
                  clearPatch();
                }}
                type="number"
                value={Number.isNaN(chapterNumber) ? "" : chapterNumber}
              />
            </label>
            <label>
              {copy.chapterTitleLabel}
              <input
                disabled={chapterActive}
                maxLength={120}
                onChange={(event) => {
                  setChapterTitle(event.currentTarget.value);
                  setManifest(null);
                  setRequestSnapshot(null);
                  setPreviewError(null);
                  setChapterRun(null);
                  setChapterError(null);
                  setContinuityReport(null);
                  setContinuityError(null);
                  clearPatch();
                }}
                placeholder={copy.chapterTitlePlaceholder}
                value={chapterTitle}
              />
            </label>
          </div>
          <fieldset className="story-source-list">
            <legend>{copy.requiredTitle}</legend>
            <p>{copy.requiredBody}</p>
            {requiredSources.map((source) => (
              <label key={source.sourceId}>
                <input checked disabled readOnly type="checkbox" />
                <SourceMetadata source={source} />
              </label>
            ))}
          </fieldset>
          <fieldset className="story-source-list">
            <legend>{copy.optionalTitle}</legend>
            <p>{copy.optionalBody}</p>
            <small>{copy.selectedCount(selectedPaths.length, optionalSources.length)}</small>
            {optionalSources.length === 0 ? <p>{copy.optionalEmpty}</p> : null}
            {optionalSources.map((source) => {
              const relativePath = source.relativePath;
              if (relativePath === null) {
                return null;
              }
              return (
                <label key={source.sourceId}>
                  <input
                    checked={selectedPaths.includes(relativePath)}
                    disabled={chapterActive}
                    onChange={(event) => toggleSource(relativePath, event.currentTarget.checked)}
                    type="checkbox"
                  />
                  <SourceMetadata source={source} />
                </label>
              );
            })}
          </fieldset>
          <button
            className="story-build-button"
            disabled={!canBuild}
            onClick={buildManifest}
            type="button"
          >
            {building ? copy.building : copy.build}
          </button>
        </>
      ) : null}
      {presentedBuildError ? (
        <div className="conversation-error" role="alert">
          <strong>{presentedBuildError.title}</strong>
          <span>{presentedBuildError.message}</span>
          <small>{presentedBuildError.recovery}</small>
        </div>
      ) : null}
      {presentedPreviewError ? (
        <div className="conversation-error" role="alert">
          <strong>{presentedPreviewError.title}</strong>
          <span>{presentedPreviewError.message}</span>
          <small>{presentedPreviewError.recovery}</small>
        </div>
      ) : null}
      {presentedChapterError ? (
        <div className="conversation-error" role="alert">
          <strong>{presentedChapterError.title}</strong>
          <span>{presentedChapterError.message}</span>
          <small>{presentedChapterError.recovery}</small>
        </div>
      ) : null}
      {manifest ? (
        <section aria-label={copy.manifestAriaLabel} className="story-context-manifest">
          <header>
            <div>
              <span>{copy.manifestKicker}</span>
              <strong>{copy.manifestTitle}</strong>
            </div>
            <small title={manifest.aggregateHash}>{shortHash(manifest.aggregateHash)}</small>
          </header>
          <dl className="story-manifest-summary">
            <div>
              <dt>{copy.targetLabel}</dt>
              <dd>{manifest.targetChapter.relativePath}</dd>
            </div>
            <div>
              <dt>{copy.sourceCountLabel}</dt>
              <dd>{manifest.selectedSources.length}</dd>
            </div>
            <div>
              <dt>{copy.excludedCountLabel}</dt>
              <dd>{manifest.excludedSources.length}</dd>
            </div>
            <div>
              <dt>{copy.totalBytesLabel}</dt>
              <dd>{copy.bytes(manifest.budget.totalIncludedBytes)}</dd>
            </div>
          </dl>
          <div className="story-budget-grid">
            <label>
              {copy.protocolBudgetLabel}
              <progress
                max={manifest.budget.protocolBudgetBytes}
                value={manifest.budget.protocolBytesUsed}
              />
              <small>
                {copy.budgetUse(
                  manifest.budget.protocolBytesUsed,
                  manifest.budget.protocolBudgetBytes,
                )}
              </small>
            </label>
            <label>
              {copy.contextBudgetLabel}
              <progress
                max={manifest.budget.projectContextBudgetBytes}
                value={manifest.budget.projectContextBytesUsed}
              />
              <small>
                {copy.budgetUse(
                  manifest.budget.projectContextBytesUsed,
                  manifest.budget.projectContextBudgetBytes,
                )}
              </small>
            </label>
            <label>
              {copy.safetyBudgetLabel}
              <progress
                max={manifest.budget.safetyReserveBytes}
                value={manifest.budget.safetyReserveBytes}
              />
              <small>{copy.safetyBudgetBody(manifest.budget.safetyReserveBytes)}</small>
            </label>
          </div>
          <section aria-label={copy.readerModelAriaLabel} className="story-reader-model">
            <header>
              <div>
                <span>{copy.readerModelKicker}</span>
                <strong>{copy.readerModelTitle}</strong>
              </div>
              <small>{copy.readerModelMemoryOnly}</small>
            </header>
            <p>{copy.readerModelBody}</p>
            <div className="story-reader-model-grid">
              {READER_MODEL_FIELDS.map((field) => (
                <label key={field}>
                  <span>{copy.readerFields[field].label}</span>
                  <small>{copy.readerFields[field].help}</small>
                  <textarea
                    disabled={chapterActive}
                    maxLength={4_096}
                    onChange={(event) => updateReaderModel(field, event.currentTarget.value)}
                    placeholder={copy.readerFields[field].placeholder}
                    rows={4}
                    value={readerModel[field]}
                  />
                </label>
              ))}
            </div>
            <p className="story-reader-empty-note">{copy.readerModelEmpty}</p>
            <button
              className="story-build-button"
              disabled={!canPreviewRequest}
              onClick={previewRequest}
              type="button"
            >
              {previewingRequest
                ? copy.previewingRequest
                : requestSnapshot?.sourceReview.status === "needsReview"
                  ? copy.reviewChangedSources
                  : copy.previewRequest}
            </button>
          </section>
          {requestSnapshot ? (
            <section
              aria-label={copy.requestSnapshotAriaLabel}
              className="story-request-snapshot"
              data-review-status={requestSnapshot.sourceReview.status}
            >
              <header>
                <div>
                  <span>{copy.requestSnapshotKicker}</span>
                  <strong>{copy.requestSnapshotTitle}</strong>
                </div>
                <small>{copy.sourceReviewStatus[requestSnapshot.sourceReview.status]}</small>
              </header>
              {requestSnapshot.sourceReview.status === "needsReview" ? (
                <p className="story-source-review-warning" role="status">
                  {copy.sourceReviewWarning}
                </p>
              ) : (
                <p className="story-source-review-current">{copy.sourceReviewCurrent}</p>
              )}
              <dl className="story-manifest-summary">
                <div>
                  <dt>{copy.readerModelBytesLabel}</dt>
                  <dd>{copy.bytes(requestSnapshot.budget.readerModelBytes)}</dd>
                </div>
                <div>
                  <dt>{copy.requestBytesLabel}</dt>
                  <dd>{copy.bytes(requestSnapshot.budget.requestBytesUsed)}</dd>
                </div>
                <div>
                  <dt>{copy.hardBudgetLabel}</dt>
                  <dd>
                    {copy.budgetUse(
                      requestSnapshot.budget.requestBytesWithSafetyReserve,
                      requestSnapshot.budget.requestLimitBytes,
                    )}
                  </dd>
                </div>
                <div>
                  <dt>{copy.maximumOutputLabel}</dt>
                  <dd>{copy.outputTokens(requestSnapshot.budget.maximumOutputTokens)}</dd>
                </div>
              </dl>
              <div className="story-request-hashes">
                <span>
                  {copy.readerModelHashLabel}{" "}
                  <code title={requestSnapshot.readerModelHash}>
                    {shortHash(requestSnapshot.readerModelHash)}
                  </code>
                </span>
                <span>
                  {copy.requestHashLabel}{" "}
                  <code title={requestSnapshot.requestHash}>
                    {shortHash(requestSnapshot.requestHash)}
                  </code>
                </span>
              </div>
              <div className="story-chapter-actions">
                {chapterActive && chapterRun ? (
                  <button
                    className="story-cancel-button"
                    disabled={!chapterRun.canCancel}
                    onClick={cancelChapter}
                    type="button"
                  >
                    {copy.cancelProposal}
                  </button>
                ) : (
                  <button
                    className="story-build-button"
                    disabled={!canGenerateChapter}
                    onClick={() =>
                      generateChapter(chapterRun?.canRetry ? chapterRun.providerId : undefined)
                    }
                    type="button"
                  >
                    {chapterRun?.canRetry
                      ? copy.retryProposal(chapterRun.providerId)
                      : chapterRun?.state === "completed"
                        ? copy.regenerateProposal
                        : copy.generateProposal}
                  </button>
                )}
                <small>{copy.providerBoundary}</small>
              </div>
            </section>
          ) : null}
          {chapterRun ? (
            <section
              aria-label={copy.proposalRunAriaLabel}
              className="story-chapter-run"
              data-run-state={chapterRun.state}
            >
              <header>
                <div>
                  <span>{copy.proposalRunKicker}</span>
                  <strong>{copy.runStates[chapterRun.state]}</strong>
                </div>
                <small>
                  {chapterRun.providerId} · {chapterRun.modelId}
                </small>
              </header>
              <dl className="story-manifest-summary">
                <div>
                  <dt>{copy.startedAtLabel}</dt>
                  <dd>{formatTimestamp(chapterRun.startedAt)}</dd>
                </div>
                <div>
                  <dt>{copy.completedAtLabel}</dt>
                  <dd>{formatTimestamp(chapterRun.completedAt)}</dd>
                </div>
                <div>
                  <dt>{copy.manifestHashLabel}</dt>
                  <dd title={chapterRun.sourceManifestHash}>
                    {shortHash(chapterRun.sourceManifestHash)}
                  </dd>
                </div>
                <div>
                  <dt>{copy.requestHashLabel}</dt>
                  <dd title={chapterRun.requestHash}>{shortHash(chapterRun.requestHash)}</dd>
                </div>
              </dl>
              {chapterRun.state === "repairing" ? (
                <p className="story-source-review-warning" role="status">
                  {copy.repairingProposal}
                </p>
              ) : null}
              {chapterRun.state === "cancelled" ? (
                <p className="story-source-review-warning" role="status">
                  {copy.cancelledProposal}
                </p>
              ) : null}
              {chapterRun.invalidResponsePreview ? (
                <details className="story-invalid-response">
                  <summary>{copy.invalidResponseTitle}</summary>
                  <p>{copy.invalidResponseBody}</p>
                  <pre>{chapterRun.invalidResponsePreview}</pre>
                </details>
              ) : null}
              {chapterRun.proposal ? (
                <article className="story-chapter-proposal">
                  <header>
                    <div>
                      <span>{copy.proposalKicker}</span>
                      <strong>{chapterRun.proposal.title}</strong>
                    </div>
                    <small>{copy.sourceStatuses[chapterRun.proposal.sourceStatus]}</small>
                  </header>
                  {chapterRun.proposal.sourceStatus === "stale" ? (
                    <p className="story-source-review-warning" role="status">
                      {copy.staleProposal}
                    </p>
                  ) : (
                    <p className="story-source-review-current">{copy.currentProposal}</p>
                  )}
                  <dl className="story-manifest-summary">
                    <div>
                      <dt>{copy.targetLabel}</dt>
                      <dd>{chapterRun.proposal.targetChapter.relativePath}</dd>
                    </div>
                    <div>
                      <dt>{copy.finishReasonLabel}</dt>
                      <dd>{chapterRun.proposal.finishReason}</dd>
                    </div>
                    <div>
                      <dt>{copy.completenessLabel}</dt>
                      <dd>{copy.complete}</dd>
                    </div>
                    <div>
                      <dt>{copy.continuityVersionLabel}</dt>
                      <dd>{copy.notChecked}</dd>
                    </div>
                  </dl>
                  <section>
                    <strong>{copy.intentSummaryLabel}</strong>
                    <p>{chapterRun.proposal.intentSummary}</p>
                  </section>
                  <section>
                    <strong>{copy.draftPreviewLabel}</strong>
                    <pre className="story-draft-preview">{chapterRun.proposal.draftMarkdown}</pre>
                  </section>
                  <details>
                    <summary>
                      {copy.claimsTitle(chapterRun.proposal.continuityClaims.length)}
                    </summary>
                    {chapterRun.proposal.continuityClaims.length === 0 ? (
                      <p>{copy.noClaims}</p>
                    ) : (
                      <ul>
                        {chapterRun.proposal.continuityClaims.map((claim, index) => (
                          <li key={`${claim.claim}-${index}`}>
                            {claim.claim}
                            <small>{claim.sourceIds.join(" · ") || copy.noSourceReference}</small>
                          </li>
                        ))}
                      </ul>
                    )}
                  </details>
                  <details>
                    <summary>
                      {copy.ledgerCandidatesTitle(chapterRun.proposal.ledgerCandidates.length)}
                    </summary>
                    {chapterRun.proposal.ledgerCandidates.length === 0 ? (
                      <p>{copy.noLedgerCandidates}</p>
                    ) : (
                      <ul>
                        {chapterRun.proposal.ledgerCandidates.map((candidate, index) => (
                          <li key={`${candidate.summary}-${index}`}>
                            {candidate.summary}
                            <small>
                              {candidate.sourceIds.join(" · ") || copy.noSourceReference}
                            </small>
                          </li>
                        ))}
                      </ul>
                    )}
                  </details>
                  <section className="story-continuity-panel" aria-label={copy.continuityAriaLabel}>
                    <header>
                      <div>
                        <span>{copy.continuityKicker}</span>
                        <strong>{copy.continuityTitle}</strong>
                      </div>
                      <button
                        disabled={!canCheckContinuity}
                        onClick={checkContinuity}
                        type="button"
                      >
                        {continuityChecking ? copy.continuityChecking : copy.runContinuityCheck}
                      </button>
                    </header>
                    <p>{copy.continuityLocalBoundary}</p>
                    {presentedContinuityError ? (
                      <div className="conversation-error" role="alert">
                        <strong>{presentedContinuityError.title}</strong>
                        <span>{presentedContinuityError.message}</span>
                        <small>{presentedContinuityError.recovery}</small>
                      </div>
                    ) : null}
                    {continuityReport?.proposalHash === chapterRun.proposal.proposalHash ? (
                      <>
                        <dl className="story-continuity-summary">
                          <div data-severity="blocking">
                            <dt>{copy.continuitySeverities.blocking}</dt>
                            <dd>{continuityReport.summary.blocking}</dd>
                          </div>
                          <div data-severity="warning">
                            <dt>{copy.continuitySeverities.warning}</dt>
                            <dd>{continuityReport.summary.warning}</dd>
                          </div>
                          <div data-severity="unknown">
                            <dt>{copy.continuitySeverities.unknown}</dt>
                            <dd>{continuityReport.summary.unknown}</dd>
                          </div>
                          <div data-severity="passed">
                            <dt>{copy.continuitySeverities.passed}</dt>
                            <dd>{continuityReport.summary.passed}</dd>
                          </div>
                        </dl>
                        {continuityReport.summary.blocking > 0 ? (
                          <p className="story-continuity-blocked" role="alert">
                            {copy.continuityBlocked}
                          </p>
                        ) : null}
                        <div className="story-continuity-results">
                          {continuityReport.results.map((result) => (
                            <details
                              key={result.ruleId}
                              data-severity={result.severity}
                              open={result.severity !== "passed"}
                            >
                              <summary>
                                <span>{copy.continuitySeverities[result.severity]}</span>
                                <strong>{result.title}</strong>
                                <code>{result.ruleId}</code>
                              </summary>
                              <p>{result.explanation}</p>
                              <small>
                                {copy.continuityEvidence}: {result.evidence.join(" · ")}
                              </small>
                              <small>
                                {copy.continuitySources}:{" "}
                                {result.sourceIds.join(" · ") || copy.noSourceReference}
                              </small>
                              <p>
                                {copy.continuityAction}: {result.suggestedAction}
                              </p>
                            </details>
                          ))}
                        </div>
                        {(continuityReport.summary.warning > 0 &&
                          !continuityReport.warningsConfirmed) ||
                        (continuityReport.summary.unknown > 0 &&
                          !continuityReport.unknownsConfirmed) ? (
                          <button
                            className="story-warning-confirm"
                            onClick={confirmContinuityWarnings}
                            type="button"
                          >
                            {copy.confirmContinuityWarnings}
                          </button>
                        ) : null}
                        {continuityReport.warningsConfirmed &&
                        continuityReport.unknownsConfirmed &&
                        (continuityReport.summary.warning > 0 ||
                          continuityReport.summary.unknown > 0) ? (
                          <p className="story-source-review-current">
                            {copy.continuityWarningsConfirmed}
                          </p>
                        ) : null}
                        <p className="story-zero-write-note">{copy.continuityZeroWrite}</p>
                      </>
                    ) : (
                      <p>{copy.continuityNotRun}</p>
                    )}
                  </section>
                  <section className="story-patch-panel" aria-label={copy.patchAriaLabel}>
                    <header>
                      <div>
                        <span>{copy.patchKicker}</span>
                        <strong>{copy.patchTitle}</strong>
                      </div>
                      <button
                        disabled={!canPreviewPatch}
                        onClick={previewChapterPatch}
                        type="button"
                      >
                        {patchPreviewing ? copy.patchPreviewing : copy.previewPatch}
                      </button>
                    </header>
                    <p>{copy.patchBoundary}</p>
                    {action === "revise" ? (
                      <label className="story-patch-unlock">
                        <input
                          checked={acceptedOrFinalUnlock}
                          disabled={patchPreviewing || patchApplying || patchApplied !== null}
                          onChange={(event) => {
                            setAcceptedOrFinalUnlock(event.currentTarget.checked);
                            setPatchPreview(null);
                            setPatchApplied(null);
                            setPatchError(null);
                          }}
                          type="checkbox"
                        />
                        <span>
                          <strong>{copy.patchUnlockLabel}</strong>
                          <small>{copy.patchUnlockBody}</small>
                        </span>
                      </label>
                    ) : null}
                    {presentedPatchError ? (
                      <div className="conversation-error" role="alert">
                        <strong>{presentedPatchError.title}</strong>
                        <span>{presentedPatchError.message}</span>
                        <small>{presentedPatchError.recovery}</small>
                      </div>
                    ) : null}
                    {patchPreview?.proposalHash === chapterRun.proposal.proposalHash ? (
                      <>
                        <dl className="story-patch-summary">
                          <div>
                            <dt>{copy.patchTargetLabel}</dt>
                            <dd>{patchPreview.targetRelativePath}</dd>
                          </div>
                          <div>
                            <dt>{copy.patchOperationLabel}</dt>
                            <dd>{copy.patchOperations[patchPreview.operation]}</dd>
                          </div>
                          <div data-kind="added">
                            <dt>{copy.patchAddedLabel}</dt>
                            <dd>{patchPreview.summary.addedLines}</dd>
                          </div>
                          <div data-kind="removed">
                            <dt>{copy.patchRemovedLabel}</dt>
                            <dd>{patchPreview.summary.removedLines}</dd>
                          </div>
                        </dl>
                        {patchPreview.previousStatus === "accepted" ||
                        patchPreview.previousStatus === "final" ? (
                          <p className="story-patch-status-warning">
                            {copy.patchStatusReset(patchPreview.previousStatus)}
                          </p>
                        ) : null}
                        <div
                          className="story-patch-diff"
                          role="region"
                          aria-label={copy.diffAriaLabel}
                        >
                          {patchPreview.hunks.map((hunk, hunkIndex) => (
                            <section key={`${hunk.oldStart}-${hunk.newStart}-${hunkIndex}`}>
                              <header>
                                @@ -{hunk.oldStart},{hunk.oldLineCount} +{hunk.newStart},
                                {hunk.newLineCount} @@
                              </header>
                              {hunk.lines.map((line, lineIndex) => (
                                <div
                                  data-kind={line.kind}
                                  key={`${line.kind}-${line.oldLineNumber}-${line.newLineNumber}-${lineIndex}`}
                                >
                                  <span>{line.oldLineNumber ?? ""}</span>
                                  <span>{line.newLineNumber ?? ""}</span>
                                  <code>
                                    {line.kind === "added"
                                      ? "+"
                                      : line.kind === "removed"
                                        ? "-"
                                        : " "}
                                    {line.content}
                                  </code>
                                </div>
                              ))}
                            </section>
                          ))}
                        </div>
                        <p className="story-patch-confirmation">{copy.patchConfirmation}</p>
                        <button
                          className="story-patch-apply"
                          disabled={patchApplying || patchApplied !== null}
                          onClick={applyChapterPatch}
                          type="button"
                        >
                          {patchApplying
                            ? copy.patchApplying
                            : patchApplied
                              ? copy.patchApplied
                              : copy.applyPatch}
                        </button>
                        {patchApplied ? (
                          <p className="story-patch-receipt">
                            {copy.patchReceipt(
                              patchApplied.file.relativePath,
                              patchApplied.transactionId,
                            )}
                          </p>
                        ) : null}
                      </>
                    ) : (
                      <p>
                        {continuityReport?.reviewReady
                          ? copy.patchReady
                          : copy.patchNeedsContinuity}
                      </p>
                    )}
                  </section>
                  {patchApplied ? (
                    <section
                      className="story-patch-panel story-lifecycle-panel"
                      aria-label={copy.lifecycleAriaLabel}
                    >
                      <header>
                        <div>
                          <span>{copy.lifecycleKicker}</span>
                          <strong>{copy.lifecycleTitle}</strong>
                        </div>
                        {nextLifecycleStatus ? (
                          <button
                            disabled={statePreviewing || stateApplying}
                            onClick={previewChapterState}
                            type="button"
                          >
                            {statePreviewing
                              ? copy.lifecyclePreviewing
                              : copy.previewLifecycle(nextLifecycleStatus)}
                          </button>
                        ) : null}
                      </header>
                      <p>{copy.lifecycleBoundary}</p>
                      {presentedLifecycleError ? (
                        <div className="conversation-error" role="alert">
                          <strong>{presentedLifecycleError.title}</strong>
                          <span>{presentedLifecycleError.message}</span>
                          <small>{presentedLifecycleError.recovery}</small>
                        </div>
                      ) : null}
                      {lifecycleStatus ? (
                        <>
                          <dl className="story-patch-summary">
                            <div>
                              <dt>{copy.lifecycleCurrentStatus}</dt>
                              <dd>{copy.lifecycleStatuses[lifecycleStatus.chapterStatus]}</dd>
                            </div>
                            <div>
                              <dt>{copy.lifecycleTarget}</dt>
                              <dd>{lifecycleStatus.targetRelativePath}</dd>
                            </div>
                            <div>
                              <dt>{copy.ledgerPendingCount}</dt>
                              <dd>{pendingLedgerCount}</dd>
                            </div>
                          </dl>
                          {lifecycleStatus.chapterStatus === "final" ? (
                            <p className="story-source-review-current">{copy.lifecycleFinal}</p>
                          ) : null}
                        </>
                      ) : (
                        <p>{copy.lifecycleLoading}</p>
                      )}
                      {statePreview ? (
                        <>
                          <p className="story-patch-status-warning">
                            {copy.lifecycleTransition(
                              statePreview.previousStatus,
                              statePreview.nextStatus,
                            )}
                          </p>
                          <div
                            className="story-patch-diff"
                            role="region"
                            aria-label={copy.lifecycleDiffAriaLabel}
                          >
                            {statePreview.hunks.map((hunk, hunkIndex) => (
                              <section key={[hunk.oldStart, hunk.newStart, hunkIndex].join("-")}>
                                <header>
                                  @@ -{hunk.oldStart},{hunk.oldLineCount} +{hunk.newStart},
                                  {hunk.newLineCount} @@
                                </header>
                                {hunk.lines.map((line, lineIndex) => (
                                  <div
                                    data-kind={line.kind}
                                    key={[
                                      line.kind,
                                      line.oldLineNumber,
                                      line.newLineNumber,
                                      lineIndex,
                                    ].join("-")}
                                  >
                                    <span>{line.oldLineNumber ?? ""}</span>
                                    <span>{line.newLineNumber ?? ""}</span>
                                    <code>
                                      {line.kind === "added"
                                        ? "+"
                                        : line.kind === "removed"
                                          ? "-"
                                          : " "}
                                      {line.content}
                                    </code>
                                  </div>
                                ))}
                              </section>
                            ))}
                          </div>
                          <p className="story-patch-confirmation">{copy.lifecycleConfirmation}</p>
                          <button
                            className="story-patch-apply"
                            disabled={stateApplying || stateApplied !== null}
                            onClick={applyChapterState}
                            type="button"
                          >
                            {stateApplying
                              ? copy.lifecycleApplying
                              : stateApplied
                                ? copy.lifecycleApplied
                                : copy.applyLifecycle(statePreview.nextStatus)}
                          </button>
                          {stateApplied ? (
                            <p className="story-patch-receipt">
                              {copy.patchReceipt(
                                stateApplied.file.relativePath,
                                stateApplied.transactionId,
                              )}
                            </p>
                          ) : null}
                        </>
                      ) : null}
                      <section className="story-ledger-panel">
                        <header>
                          <div>
                            <span>{copy.ledgerKicker}</span>
                            <strong>{copy.ledgerTitle}</strong>
                          </div>
                          <div className="story-ledger-actions">
                            <button
                              disabled={
                                pendingLedgerCount === 0 || ledgerPreviewing || ledgerApplying
                              }
                              onClick={previewLedgerUpdate}
                              type="button"
                            >
                              {ledgerPreviewing ? copy.ledgerPreviewing : copy.previewLedger}
                            </button>
                            <button
                              disabled={pendingLedgerCount === 0 || ledgerApplying}
                              onClick={() => {
                                setLedgerDeferred(true);
                                setLedgerPreview(null);
                                setLedgerApplied(null);
                                setLifecycleError(null);
                              }}
                              type="button"
                            >
                              {copy.deferLedger}
                            </button>
                          </div>
                        </header>
                        <p>{copy.ledgerBoundary}</p>
                        {ledgerDeferred ? (
                          <p className="story-patch-status-warning">{copy.ledgerDeferred}</p>
                        ) : null}
                        {pendingLedgerCount === 0 && lifecycleStatus ? (
                          <p className="story-source-review-current">{copy.ledgerCurrent}</p>
                        ) : null}
                        {ledgerPreview ? (
                          <>
                            <dl className="story-patch-summary">
                              <div>
                                <dt>{copy.patchTargetLabel}</dt>
                                <dd>{ledgerPreview.targetRelativePath}</dd>
                              </div>
                              <div>
                                <dt>{copy.ledgerCandidateCount}</dt>
                                <dd>{ledgerPreview.pendingIds.length}</dd>
                              </div>
                              <div data-kind="added">
                                <dt>{copy.patchAddedLabel}</dt>
                                <dd>{ledgerPreview.summary.addedLines}</dd>
                              </div>
                            </dl>
                            <div
                              className="story-patch-diff"
                              role="region"
                              aria-label={copy.ledgerDiffAriaLabel}
                            >
                              {ledgerPreview.hunks.map((hunk, hunkIndex) => (
                                <section key={[hunk.oldStart, hunk.newStart, hunkIndex].join("-")}>
                                  <header>
                                    @@ -{hunk.oldStart},{hunk.oldLineCount} +{hunk.newStart},
                                    {hunk.newLineCount} @@
                                  </header>
                                  {hunk.lines.map((line, lineIndex) => (
                                    <div
                                      data-kind={line.kind}
                                      key={[
                                        line.kind,
                                        line.oldLineNumber,
                                        line.newLineNumber,
                                        lineIndex,
                                      ].join("-")}
                                    >
                                      <span>{line.oldLineNumber ?? ""}</span>
                                      <span>{line.newLineNumber ?? ""}</span>
                                      <code>
                                        {line.kind === "added"
                                          ? "+"
                                          : line.kind === "removed"
                                            ? "-"
                                            : " "}
                                        {line.content}
                                      </code>
                                    </div>
                                  ))}
                                </section>
                              ))}
                            </div>
                            <p className="story-patch-confirmation">{copy.ledgerConfirmation}</p>
                            <button
                              className="story-patch-apply"
                              disabled={ledgerApplying || ledgerApplied !== null}
                              onClick={applyLedgerUpdate}
                              type="button"
                            >
                              {ledgerApplying
                                ? copy.ledgerApplying
                                : ledgerApplied
                                  ? copy.ledgerApplied
                                  : copy.applyLedger}
                            </button>
                            {ledgerApplied ? (
                              <p className="story-patch-receipt">
                                {copy.patchReceipt(
                                  ledgerApplied.file.relativePath,
                                  ledgerApplied.transactionId,
                                )}
                              </p>
                            ) : null}
                          </>
                        ) : null}
                      </section>
                    </section>
                  ) : null}
                  <p className="story-zero-write-note">{copy.proposalZeroWrite}</p>
                </article>
              ) : null}
            </section>
          ) : null}
          <div className="story-manifest-source-list">
            <strong>{copy.includedTitle}</strong>
            {manifest.selectedSources.map((source) => (
              <ManifestSource key={source.sourceId} source={source} />
            ))}
          </div>
          <details>
            <summary>{copy.excludedTitle(manifest.excludedSources.length)}</summary>
            <div className="story-manifest-source-list">
              {manifest.excludedSources.map((source) => (
                <ManifestSource key={source.sourceId} source={source} />
              ))}
            </div>
          </details>
          <p className="story-zero-write-note">{copy.zeroWrite}</p>
        </section>
      ) : null}
    </section>
  );
}
