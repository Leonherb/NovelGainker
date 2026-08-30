import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ProjectSearch } from "./ProjectSearch";
import { SourceDiagnostics } from "./SourceDiagnostics";
import { presentAppError, type PresentedAppError } from "../errors/present-app-error";
import { ZH_CN } from "../i18n/zh-CN";
import {
  requestProjectDraftValidation,
  requestProjectSearch,
  type SearchReport,
  type ValidationReport,
} from "../lib/tauri/analysis";
import { requestSetCurrentFile, type RecentProject } from "../lib/tauri/app-state";
import { normalizeAppError, type AppError } from "../lib/tauri/health";
import {
  requestProjectClose,
  requestProjectCreate,
  requestProjectOpen,
  requestProjectCreatePreview,
  requestProjectOpenRecent,
  requestProjectScan,
  type ProjectCandidate,
  type ProjectConflictKind,
  type ProjectCreateInputV1,
  type ProjectCreatePreview,
  type ProjectScanMode,
  type ProjectSession,
} from "../lib/tauri/project";
import { desktopWindowClosePort, type WindowClosePort } from "../lib/tauri/window-close";
import {
  requestProjectFile,
  requestProjectFileIndex,
  requestProjectFileSave,
  requestProjectFileWatch,
  type ProjectFileDocument,
  type ProjectFileIndex,
  type ProjectFileNode,
  type ProjectFileWatchStatus,
} from "../lib/tauri/workspace";

export type ProjectWorkspaceProps = {
  desktopRuntime: boolean;
  externalOpenPath?: string | null;
  externalRefreshToken?: number;
  navigatorHost?: HTMLElement | null;
  onAppStateChanged?: () => void;
  onFileOpenRequested?: () => void;
  recentProjects?: RecentProject[];
  onSessionChange?: (session: ProjectSession | null) => void;
  windowClosePort?: WindowClosePort;
};

type BusyAction =
  "scan" | "preview" | "create" | "open" | "index" | "read" | "save" | "search" | "close" | null;
type PendingAction =
  | { kind: "select"; relativePath: string; line?: number }
  | { kind: "closeProject" }
  | { kind: "closeWindow" };

type ProjectCreateForm = {
  title: string;
  authorDisplayName: string;
  genre: string;
  premise: string;
};

const EMPTY_PROJECT_CREATE_FORM: ProjectCreateForm = {
  title: "",
  authorDisplayName: "",
  genre: "",
  premise: "",
};

function presentProjectError(error: unknown): PresentedAppError {
  return presentAppError(normalizeAppError(error));
}

function invalidResponseError(): AppError {
  return {
    code: "contract.invalid_response",
    messageKey: "errors.invalidRuntimeResponse",
  };
}

function windowCloseError(): AppError {
  return {
    code: "runtime.unavailable",
    messageKey: "errors.runtimeUnavailable",
  };
}

type FileTreeProps = {
  nodes: ProjectFileNode[];
  selectedPath: string | null;
  onSelect: (relativePath: string) => void;
  disabled: boolean;
};

function FileTree({ nodes, selectedPath, onSelect, disabled }: FileTreeProps) {
  const copy = ZH_CN.files.tree;
  return (
    <ul className="project-file-list">
      {nodes.map((node) => (
        <li key={`${node.kind}:${node.relativePath}`}>
          {node.kind === "directory" ? (
            <details open>
              <summary>
                <span aria-hidden="true">▾</span>
                <span>{node.name}</span>
              </summary>
              {node.children.length > 0 ? (
                <FileTree
                  disabled={disabled}
                  nodes={node.children}
                  onSelect={onSelect}
                  selectedPath={selectedPath}
                />
              ) : (
                <span className="empty-directory">{copy.emptyDirectory}</span>
              )}
            </details>
          ) : (
            <button
              aria-current={selectedPath === node.relativePath ? "page" : undefined}
              className="project-file-button"
              disabled={disabled}
              onClick={() => onSelect(node.relativePath)}
              title={node.relativePath}
              type="button"
            >
              <span aria-hidden="true">{node.kind === "metadata" ? "◇" : "#"}</span>
              <span>{node.name}</span>
              {node.readOnly ? <small>{copy.readOnlyShort}</small> : null}
            </button>
          )}
        </li>
      ))}
    </ul>
  );
}

export function ProjectWorkspace({
  desktopRuntime,
  externalOpenPath = null,
  externalRefreshToken = 0,
  navigatorHost,
  onAppStateChanged = () => undefined,
  onFileOpenRequested = () => undefined,
  recentProjects = [],
  onSessionChange = () => undefined,
  windowClosePort = desktopWindowClosePort,
}: ProjectWorkspaceProps) {
  const copy = ZH_CN.files.project;
  const editorCopy = ZH_CN.files.editor;
  const diagnosticsCopy = editorCopy.diagnostics;
  const [candidate, setCandidate] = useState<ProjectCandidate | null>(null);
  const [session, setSession] = useState<ProjectSession | null>(null);

  const [createForm, setCreateForm] = useState<ProjectCreateForm>(EMPTY_PROJECT_CREATE_FORM);
  const [createPreview, setCreatePreview] = useState<ProjectCreatePreview | null>(null);
  useEffect(() => {
    onSessionChange(session);
  }, [onSessionChange, session]);
  const [fileIndex, setFileIndex] = useState<ProjectFileIndex | null>(null);
  const [document, setDocument] = useState<ProjectFileDocument | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState<BusyAction>(null);
  const [error, setError] = useState<PresentedAppError | null>(null);
  const [editorError, setEditorError] = useState<PresentedAppError | null>(null);
  const [saveError, setSaveError] = useState<PresentedAppError | null>(null);
  const [watchError, setWatchError] = useState<PresentedAppError | null>(null);
  const [externalConflict, setExternalConflict] = useState<ProjectFileWatchStatus | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [editorNotice, setEditorNotice] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<PendingAction | null>(null);
  const [validation, setValidation] = useState<ValidationReport | null>(null);
  const [validationError, setValidationError] = useState<PresentedAppError | null>(null);
  const [validationStale, setValidationStale] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [searchReport, setSearchReport] = useState<SearchReport | null>(null);
  const [searchError, setSearchError] = useState<PresentedAppError | null>(null);
  const editorRef = useRef<HTMLTextAreaElement>(null);
  const dirtyRef = useRef(false);
  const saveInFlightRef = useRef(false);
  const watchInFlightRef = useRef(false);
  const windowCloseInFlightRef = useRef(false);
  const requestFileSelectionRef = useRef<(relativePath: string, line?: number) => void>(
    () => undefined,
  );

  const dirty = document !== null && !document.readOnly && draft !== document.content;
  const watchedSessionId = session?.sessionId ?? null;
  const watchedRelativePath = document?.relativePath ?? null;
  const watchedContentHash = document?.contentHash ?? null;
  useEffect(() => {
    dirtyRef.current = dirty;
  }, [dirty]);

  const exitProjectAndCloseWindow = useCallback(
    async (sessionId: string | null) => {
      if (windowCloseInFlightRef.current) {
        return;
      }
      windowCloseInFlightRef.current = true;

      if (sessionId) {
        try {
          await requestProjectClose(sessionId);
        } catch {
          // App shutdown must not be blocked by best-effort in-memory project cleanup.
        }
      }

      try {
        await windowClosePort.destroy();
      } catch {
        windowCloseInFlightRef.current = false;
        setError(presentProjectError(windowCloseError()));
      }
    },
    [windowClosePort],
  );

  useEffect(() => {
    if (!watchedSessionId || !watchedRelativePath || !watchedContentHash) {
      return undefined;
    }

    let active = true;
    const checkForExternalChange = async () => {
      if (watchInFlightRef.current || saveInFlightRef.current) {
        return;
      }
      watchInFlightRef.current = true;
      try {
        const status = await requestProjectFileWatch(
          watchedSessionId,
          watchedRelativePath,
          watchedContentHash,
        );
        if (!active) {
          return;
        }
        if (status.relativePath !== watchedRelativePath) {
          throw invalidResponseError();
        }
        setWatchError(null);
        setExternalConflict(status.state === "unchanged" ? null : status);
      } catch (watchFailure: unknown) {
        if (active) {
          setWatchError(presentProjectError(watchFailure));
        }
      } finally {
        watchInFlightRef.current = false;
      }
    };

    void checkForExternalChange();
    const timer = window.setInterval(() => void checkForExternalChange(), 1_000);
    return () => {
      active = false;
      window.clearInterval(timer);
      watchInFlightRef.current = false;
    };
  }, [watchedContentHash, watchedRelativePath, watchedSessionId]);

  useEffect(() => {
    if (!desktopRuntime || !session) {
      return undefined;
    }

    let active = true;
    let unlisten: (() => void) | undefined;
    void windowClosePort
      .onCloseRequested((event) => {
        event.preventDefault();
        if (dirtyRef.current) {
          setPendingAction({ kind: "closeWindow" });
          return;
        }
        void exitProjectAndCloseWindow(session.sessionId);
      })
      .then((dispose) => {
        if (active) {
          unlisten = dispose;
        } else {
          dispose();
        }
      })
      .catch((closeGuardError: unknown) => {
        if (active) {
          setError(presentProjectError(closeGuardError));
        }
      });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [desktopRuntime, exitProjectAndCloseWindow, session, windowClosePort]);

  const resetDocument = () => {
    setDocument(null);
    setSelectedPath(null);
    setDraft("");
    setEditorError(null);
    setSaveError(null);
    setWatchError(null);
    setExternalConflict(null);
    setValidation(null);
    setValidationError(null);
    setValidationStale(false);
    setSearchReport(null);
    setSearchError(null);
    setEditorNotice(null);

    dirtyRef.current = false;
  };

  const refreshIndex = async (activeSession: ProjectSession) => {
    setBusy("index");
    setError(null);
    try {
      const nextIndex = await requestProjectFileIndex(activeSession.sessionId);
      setFileIndex(nextIndex);
    } catch (indexError: unknown) {
      setFileIndex(null);
      setError(presentProjectError(indexError));
    } finally {
      setBusy(null);
    }
  };

  useEffect(() => {
    if (!session || externalRefreshToken === 0) {
      return undefined;
    }
    let active = true;
    const timer = window.setTimeout(() => {
      void requestProjectFileIndex(session.sessionId)
        .then((nextIndex) => {
          if (active) {
            setFileIndex(nextIndex);
            setError(null);
            if (externalOpenPath) {
              requestFileSelectionRef.current(externalOpenPath);
            }
          }
        })
        .catch((indexError: unknown) => {
          if (active) {
            setError(presentProjectError(indexError));
          }
        });
    }, 0);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [externalOpenPath, externalRefreshToken, session]);

  const focusEditorAtLine = (content: string, line = 1) => {
    const boundedLine = Math.max(1, line);
    let start = 0;
    for (let currentLine = 1; currentLine < boundedLine; currentLine += 1) {
      const nextBreak = content.indexOf("\n", start);
      if (nextBreak < 0) {
        start = content.length;
        break;
      }
      start = nextBreak + 1;
    }
    const end = content.indexOf("\n", start);
    requestAnimationFrame(() => {
      editorRef.current?.focus();
      editorRef.current?.setSelectionRange(start, end < 0 ? content.length : end);
    });
  };

  const loadFile = async (
    relativePath: string,
    line?: number,
    activeSession: ProjectSession | null = session,
  ) => {
    if (!activeSession) {
      return;
    }
    setBusy("read");
    setSelectedPath(relativePath);
    setDocument(null);
    setDraft("");
    setEditorError(null);
    setSaveError(null);
    setWatchError(null);
    setExternalConflict(null);
    setEditorNotice(null);

    dirtyRef.current = false;
    setValidation(null);
    setValidationError(null);
    setValidationStale(false);
    try {
      const nextDocument = await requestProjectFile(activeSession.sessionId, relativePath);
      if (nextDocument.relativePath !== relativePath) {
        throw invalidResponseError();
      }
      setDocument({
        ...nextDocument,
        readOnly: nextDocument.readOnly || !activeSession.capabilities.canSaveFiles,
      });
      setDraft(nextDocument.content);
      focusEditorAtLine(nextDocument.content, line);
      try {
        await requestSetCurrentFile(activeSession.sessionId, nextDocument.relativePath);
        onAppStateChanged();
      } catch {
        setEditorNotice(copy.preferenceSaveFailed);
      }
    } catch (readError: unknown) {
      setEditorError(presentProjectError(readError));
    } finally {
      setBusy(null);
    }
  };

  const saveCurrentFile = async () => {
    if (
      !session ||
      !document ||
      document.readOnly ||
      !session.capabilities.canSaveFiles ||
      externalConflict !== null ||
      draft === document.content ||
      saveInFlightRef.current
    ) {
      if (externalConflict) {
        setEditorNotice(editorCopy.externalChange.saveBlocked);
      } else if (document && !document.readOnly && draft === document.content) {
        setEditorNotice(editorCopy.alreadySavedNotice);
      }
      return;
    }

    const savingDocument = document;
    const savingContent = draft;
    saveInFlightRef.current = true;
    setBusy("save");
    setSaveError(null);
    setEditorNotice(null);
    setValidationError(null);
    try {
      const advisory = await requestProjectDraftValidation(
        session.sessionId,
        savingDocument.relativePath,
        savingContent,
      );
      setValidation(advisory);
      setValidationStale(false);
    } catch (validationFailure: unknown) {
      setValidationError(presentProjectError(validationFailure));
    }
    try {
      const result = await requestProjectFileSave(
        session.sessionId,
        savingDocument.relativePath,
        savingContent,
        savingDocument.contentHash,
      );
      if (result.relativePath !== savingDocument.relativePath) {
        throw invalidResponseError();
      }
      setDocument({
        ...savingDocument,
        content: savingContent,
        contentHash: result.contentHash,
        modifiedTimeMs: result.modifiedTimeMs,
        sizeBytes: result.sizeBytes,
        bom: result.bom,
        lineEnding: result.lineEnding,
      });
      dirtyRef.current = false;
      setExternalConflict(null);
      setWatchError(null);
      setValidation(result.validation);
      setValidationStale(false);
      setValidationError(null);
      setEditorNotice(
        result.validation.diagnostics.length > 0
          ? diagnosticsCopy.savedWithIssues
          : diagnosticsCopy.savedValid,
      );
    } catch (saveFailure: unknown) {
      setSaveError(presentProjectError(saveFailure));
      setEditorNotice(editorCopy.saveFailedDraftKept);
      dirtyRef.current = true;
    } finally {
      saveInFlightRef.current = false;
      setBusy(null);
    }
  };
  const scan = async (mode: ProjectScanMode) => {
    setBusy("scan");
    setError(null);
    setNotice(null);
    setCreateForm(EMPTY_PROJECT_CREATE_FORM);
    setCreatePreview(null);
    try {
      const result = await requestProjectScan(mode);
      setCandidate(result);
      if (result === null) {
        setNotice(copy.cancelled);
      }
    } catch (scanError: unknown) {
      setCandidate(null);
      setError(presentProjectError(scanError));
    } finally {
      setBusy(null);
    }
  };

  const updateCreateForm = (field: keyof ProjectCreateForm, value: string) => {
    setCreateForm((current) => ({ ...current, [field]: value }));
    setCreatePreview(null);
    setError(null);
  };

  const prepareCreatePreview = async () => {
    if (
      !candidate ||
      candidate.mode !== "create" ||
      !candidate.canConfirm ||
      createForm.title.trim().length === 0
    ) {
      return;
    }
    const optional = (value: string) => (value.trim().length > 0 ? value : null);
    const input: ProjectCreateInputV1 = {
      schemaVersion: 1,
      title: createForm.title,
      authorDisplayName: optional(createForm.authorDisplayName),
      genre: optional(createForm.genre),
      premise: optional(createForm.premise),
    };
    setBusy("preview");
    setError(null);
    try {
      const preview = await requestProjectCreatePreview(candidate.candidateId, input);
      setCreatePreview(preview);
    } catch (previewError: unknown) {
      setCreatePreview(null);
      setError(presentProjectError(previewError));
    } finally {
      setBusy(null);
    }
  };

  const searchProject = async () => {
    if (!session || searchQuery.length === 0 || busy !== null) {
      return;
    }
    setBusy("search");
    setSearchError(null);
    try {
      setSearchReport(await requestProjectSearch(session.sessionId, searchQuery, caseSensitive));
    } catch (searchFailure: unknown) {
      setSearchReport(null);
      setSearchError(presentProjectError(searchFailure));
    } finally {
      setBusy(null);
    }
  };

  const confirm = async () => {
    if (
      !candidate ||
      !candidate.canConfirm ||
      (candidate.mode === "create" && createPreview === null)
    ) {
      return;
    }
    setBusy(candidate.mode);
    setError(null);
    try {
      const opened =
        candidate.mode === "create"
          ? await requestProjectCreate(candidate.candidateId, createPreview?.previewHash ?? "")
          : await requestProjectOpen(candidate.candidateId, candidate.requiresMetadataConfirmation);
      setSession(opened);
      setCandidate(null);
      setNotice(null);
      resetDocument();
      const nextIndex = await requestProjectFileIndex(opened.sessionId);
      setFileIndex(nextIndex);
      if (opened.restoredCurrentFile) {
        await loadFile(opened.restoredCurrentFile, undefined, opened);
      }
      setCreatePreview(null);
      onAppStateChanged();
    } catch (confirmError: unknown) {
      setError(presentProjectError(confirmError));
    } finally {
      setBusy(null);
    }
  };

  const openRecentProject = async (projectId: string) => {
    setBusy("open");
    setError(null);
    try {
      const opened = await requestProjectOpenRecent(projectId);
      setSession(opened);
      setCandidate(null);
      setNotice(null);
      resetDocument();
      const nextIndex = await requestProjectFileIndex(opened.sessionId);
      setFileIndex(nextIndex);
      if (opened.restoredCurrentFile) {
        await loadFile(opened.restoredCurrentFile, undefined, opened);
      }
      onAppStateChanged();
    } catch (openError: unknown) {
      setError(presentProjectError(openError));
      setCreatePreview(null);
      onAppStateChanged();
    } finally {
      setBusy(null);
    }
  };

  const closeProjectNow = async () => {
    if (!session) {
      return;
    }
    setBusy("close");
    setError(null);
    try {
      await requestProjectClose(session.sessionId);
      setSession(null);
      setFileIndex(null);
      setNotice(null);
      resetDocument();
    } catch (closeError: unknown) {
      setError(presentProjectError(closeError));
    } finally {
      setBusy(null);
    }
  };

  const requestFileSelection = (relativePath: string, line?: number) => {
    onFileOpenRequested();
    if (relativePath === selectedPath) {
      if (line) {
        focusEditorAtLine(draft, line);
      }
      return;
    }
    if (dirtyRef.current) {
      setPendingAction({ kind: "select", relativePath, line });
      return;
    }
    void loadFile(relativePath, line);
  };
  useEffect(() => {
    requestFileSelectionRef.current = requestFileSelection;
  });

  const requestProjectClosing = () => {
    if (dirtyRef.current) {
      setPendingAction({ kind: "closeProject" });
      return;
    }
    void closeProjectNow();
  };

  const confirmDiscard = async () => {
    const action = pendingAction;
    setPendingAction(null);
    if (!action) {
      return;
    }
    if (action.kind === "select") {
      await loadFile(action.relativePath, action.line);
      return;
    }
    if (action.kind === "closeProject") {
      await closeProjectNow();
      return;
    }

    await exitProjectAndCloseWindow(session?.sessionId ?? null);
  };

  const copyDraftToClipboard = async () => {
    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error("clipboard unavailable");
      }
      await navigator.clipboard.writeText(draft);
      setEditorNotice(editorCopy.externalChange.copied);
    } catch {
      editorRef.current?.focus();
      editorRef.current?.select();
      setEditorNotice(editorCopy.externalChange.copyFallback);
    }
  };

  const conflictCopy: Record<ProjectConflictKind, string> = copy.conflictKinds;
  const statusLabel = session
    ? copy.opened
    : candidate
      ? candidate.mode === "create"
        ? createPreview
          ? copy.scaffoldPreview
          : copy.createPreview
        : copy.openPreview
      : ZH_CN.files.unopened;
  const busyLabel =
    busy === "scan"
      ? copy.selecting
      : busy === "preview"
        ? copy.preparingPreview
        : busy === "create"
          ? copy.creating
          : busy === "open"
            ? copy.opening
            : busy === "index"
              ? copy.indexing
              : busy === "read"
                ? copy.reading
                : busy === "save"
                  ? copy.saving
                  : busy === "search"
                    ? copy.searching
                    : busy === "close"
                      ? copy.closing
                      : null;
  const editorStatus =
    busy === "save"
      ? editorCopy.saving
      : saveError
        ? editorCopy.saveFailed
        : externalConflict
          ? editorCopy.externalChange.status
          : document?.readOnly
            ? editorCopy.readOnly
            : dirty
              ? editorCopy.unsaved
              : document
                ? editorCopy.saved
                : editorCopy.noFile;
  const discardBody =
    pendingAction?.kind === "select"
      ? editorCopy.discardSwitchBody
      : pendingAction?.kind === "closeProject"
        ? editorCopy.discardProjectBody
        : editorCopy.discardWindowBody;

  const projectNavigator = (
    <section aria-labelledby="file-tree-title" className="file-tree-card project-workspace">
      <div className="section-title-row">
        <h2 id="file-tree-title">{ZH_CN.files.projectFiles}</h2>
        <span className="quiet-pill">{statusLabel}</span>
      </div>

      {busyLabel ? (
        <div aria-live="polite" className="project-progress">
          <span aria-hidden="true">◌</span>
          <p>{busyLabel}</p>
        </div>
      ) : null}

      {!desktopRuntime && !session && !candidate && !busy ? (
        <p className="project-runtime-note">{copy.desktopOnly}</p>
      ) : null}

      {!session && !candidate && !busy ? (
        <div className="project-actions">
          <button
            disabled={!desktopRuntime}
            id="project-create-action"
            onClick={() => void scan("create")}
            type="button"
          >
            {copy.create}
          </button>
          <button disabled={!desktopRuntime} onClick={() => void scan("open")} type="button">
            {copy.open}
          </button>
        </div>
      ) : null}

      {!session && !candidate && !busy && desktopRuntime ? (
        <section aria-label={copy.recentHeading} className="recent-projects">
          <h3>{copy.recentHeading}</h3>
          {recentProjects.length > 0 ? (
            <ul>
              {recentProjects.map((project) => (
                <li key={project.projectId}>
                  <span>{project.displayName}</span>
                  <button
                    aria-label={`${copy.openRecent} ${project.displayName}`}
                    onClick={() => void openRecentProject(project.projectId)}
                    type="button"
                  >
                    {copy.openRecent}
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p>{copy.noRecent}</p>
          )}
        </section>
      ) : null}

      {notice ? (
        <p aria-live="polite" className="project-notice">
          {notice}
        </p>
      ) : null}

      {candidate ? (
        <div className="project-preview">
          <div className="project-summary">
            <strong>{candidate.displayName}</strong>
            <dl>
              <div>
                <dt>{copy.existingEntries}</dt>
                <dd>{candidate.existingEntryCount}</dd>
              </div>
              <div>
                <dt>{copy.markdownFiles}</dt>
                <dd>{candidate.markdownFileCount}</dd>
              </div>
            </dl>
            <span className="metadata-state">
              {candidate.hasProjectMetadata ? copy.metadataReady : copy.metadataMissing}
            </span>
          </div>

          {candidate.requiresMetadataConfirmation ? (
            <p className="project-warning">{copy.importWarning}</p>
          ) : null}

          {candidate.mode === "create" ? (
            <section aria-label={copy.createFormAriaLabel} className="project-create-form">
              <div className="project-create-heading">
                <div>
                  <span>{copy.createStepMetadata}</span>
                  <h3>{copy.createFormTitle}</h3>
                </div>
                <small>{copy.fixedWritingLanguage}</small>
              </div>
              <label>
                <span>{copy.titleLabel}</span>
                <input
                  autoComplete="off"
                  disabled={busy !== null}
                  maxLength={120}
                  onChange={(event) => updateCreateForm("title", event.currentTarget.value)}
                  required
                  value={createForm.title}
                />
              </label>
              <div className="project-create-optional-grid">
                <label>
                  <span>{copy.authorLabel}</span>
                  <input
                    autoComplete="off"
                    disabled={busy !== null}
                    maxLength={120}
                    onChange={(event) =>
                      updateCreateForm("authorDisplayName", event.currentTarget.value)
                    }
                    value={createForm.authorDisplayName}
                  />
                </label>
                <label>
                  <span>{copy.genreLabel}</span>
                  <input
                    autoComplete="off"
                    disabled={busy !== null}
                    maxLength={80}
                    onChange={(event) => updateCreateForm("genre", event.currentTarget.value)}
                    value={createForm.genre}
                  />
                </label>
              </div>
              <label>
                <span>{copy.premiseLabel}</span>
                <textarea
                  disabled={busy !== null}
                  maxLength={300}
                  onChange={(event) => updateCreateForm("premise", event.currentTarget.value)}
                  rows={3}
                  value={createForm.premise}
                />
              </label>
              <p>{copy.createFormBoundary}</p>
              <button
                disabled={
                  busy !== null || !candidate.canConfirm || createForm.title.trim().length === 0
                }
                onClick={() => void prepareCreatePreview()}
                type="button"
              >
                {createPreview ? copy.regeneratePreview : copy.generatePreview}
              </button>
            </section>
          ) : null}

          {candidate.mode !== "create" || createPreview === null ? (
            <div className="project-plan">
              <h3>{copy.planTitle}</h3>
              {candidate.plannedEntries.length > 0 ? (
                <ul>
                  {candidate.plannedEntries.map((entry) => (
                    <li key={`${entry.kind}:${entry.relativePath}`}>
                      <span aria-hidden="true">{entry.kind === "directory" ? "▱" : "·"}</span>
                      <code>{entry.relativePath}</code>
                    </li>
                  ))}
                </ul>
              ) : (
                <p>{copy.noPlan}</p>
              )}
            </div>
          ) : null}

          {createPreview ? (
            <section aria-label={copy.scaffoldPreviewAriaLabel} className="scaffold-preview">
              <div className="scaffold-preview-heading">
                <div>
                  <span>{copy.createStepPreview}</span>
                  <h3>{copy.scaffoldPreviewTitle}</h3>
                </div>
                <span className="metadata-state">{copy.zeroWritesSoFar}</span>
              </div>
              <dl className="scaffold-metadata">
                <div>
                  <dt>{copy.projectIdLabel}</dt>
                  <dd>
                    <code>{createPreview.projectId}</code>
                  </dd>
                </div>
                <div>
                  <dt>{copy.workflowPackLabel}</dt>
                  <dd>{createPreview.workflowPackVersion}</dd>
                </div>
                <div>
                  <dt>{copy.language}</dt>
                  <dd>{createPreview.writingLanguage}</dd>
                </div>
                <div>
                  <dt>{copy.previewHashLabel}</dt>
                  <dd>
                    <code>{createPreview.previewHash}</code>
                  </dd>
                </div>
              </dl>
              {createPreview.existingUnrelatedEntryCount > 0 ? (
                <p className="project-warning">
                  {copy.unrelatedEntriesNotice.replace(
                    "{count}",
                    String(createPreview.existingUnrelatedEntryCount),
                  )}
                </p>
              ) : null}
              <div className="scaffold-directories">
                <h4>{copy.directoriesTitle}</h4>
                <ul>
                  {createPreview.directories.map((directory) => (
                    <li key={directory}>
                      <code>{directory}/</code>
                    </li>
                  ))}
                </ul>
              </div>
              <div className="scaffold-files">
                <h4>{copy.filesPreviewTitle}</h4>
                {createPreview.files.map((file) => (
                  <details key={file.relativePath} open={file.relativePath === "project.md"}>
                    <summary>
                      <code>{file.relativePath}</code>
                      <span>{copy.bytesLabel.replace("{count}", String(file.byteLength))}</span>
                    </summary>
                    <pre>{file.content}</pre>
                  </details>
                ))}
              </div>
            </section>
          ) : null}

          <div className="project-conflicts" data-safe={candidate.canConfirm}>
            <h3>{copy.conflictsTitle}</h3>
            {candidate.conflicts.length > 0 ? (
              <ul>
                {candidate.conflicts.map((conflict) => (
                  <li key={`${conflict.kind}:${conflict.relativePath}`}>
                    <code>{conflict.relativePath}</code>
                    <span>{conflictCopy[conflict.kind]}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p>{copy.noConflicts}</p>
            )}
          </div>

          {candidate.mode === "create" ? (
            <p className="project-safe-note">{copy.safeCreateNotice}</p>
          ) : null}

          <div className="project-confirm-actions">
            <button
              className="primary-project-action"
              disabled={
                !candidate.canConfirm ||
                busy !== null ||
                (candidate.mode === "create" && createPreview === null)
              }
              onClick={() => void confirm()}
              type="button"
            >
              {candidate.mode === "create"
                ? copy.confirmCreate
                : candidate.requiresMetadataConfirmation
                  ? copy.confirmImport
                  : copy.confirmOpen}
            </button>
            <button
              disabled={busy !== null}
              onClick={() => {
                setCandidate(null);
                setCreatePreview(null);
                setCreateForm(EMPTY_PROJECT_CREATE_FORM);
                setError(null);
              }}
              type="button"
            >
              {copy.back}
            </button>
          </div>
        </div>
      ) : null}

      {session ? (
        <div className="project-session">
          <div className="project-session-heading">
            <span aria-hidden="true">◆</span>
            <div>
              <strong>{session.displayName}</strong>
              <small>{session.accessMode === "writer" ? copy.writer : copy.readOnlySession}</small>
            </div>
          </div>
          <dl>
            <div>
              <dt>{copy.schema}</dt>
              <dd>{session.schemaVersion}</dd>
            </div>
            <div>
              <dt>{copy.language}</dt>
              <dd>{session.writingLanguage}</dd>
            </div>
            <div>
              <dt>{copy.markdownFiles}</dt>
              <dd>{session.markdownFileCount}</dd>
            </div>
          </dl>
          {session.accessMode === "readOnly" ? (
            <p className="project-warning" role="status">
              {copy.readOnlyNotice}
            </p>
          ) : null}
          {session.recoveryStatus === "recovered" ? (
            <p className="project-safe-note" role="status">
              {copy.recoveredNotice.replace("{count}", String(session.recoveredTransactionCount))}
            </p>
          ) : null}
          {session.recoveryStatus === "required" ? (
            <p className="project-warning" role="alert">
              {copy.recoveryRequiredNotice}
            </p>
          ) : null}
          <div className="project-session-actions">
            <button
              disabled={busy !== null}
              onClick={() => void refreshIndex(session)}
              type="button"
            >
              {copy.refresh}
            </button>
            <button disabled={busy !== null} onClick={requestProjectClosing} type="button">
              {copy.close}
            </button>
          </div>
          <ProjectSearch
            busy={busy === "search"}
            caseSensitive={caseSensitive}
            error={searchError}
            onCaseSensitiveChange={setCaseSensitive}
            onNavigate={requestFileSelection}
            onQueryChange={(value) => {
              setSearchQuery(value);
              setSearchReport(null);
              setSearchError(null);
            }}
            onSearch={() => void searchProject()}
            query={searchQuery}
            report={searchReport}
          />
          <div aria-label={ZH_CN.files.tree.ariaLabel} className="project-tree">
            {fileIndex && fileIndex.nodes.length > 0 ? (
              <FileTree
                disabled={busy !== null}
                nodes={fileIndex.nodes}
                onSelect={requestFileSelection}
                selectedPath={selectedPath}
              />
            ) : busy !== "index" ? (
              <p className="project-tree-empty">{copy.emptyProject}</p>
            ) : null}
          </div>
          <p>{copy.ng105Boundary}</p>
        </div>
      ) : null}

      {error ? (
        <div className="project-error" role="alert">
          <strong>{error.title}</strong>
          <p>{error.message}</p>
          <p>{error.recovery}</p>
          <code>
            {copy.errorCode}：{error.code}
          </code>
        </div>
      ) : null}
    </section>
  );

  return (
    <>
      {navigatorHost === undefined
        ? projectNavigator
        : navigatorHost
          ? createPortal(projectNavigator, navigatorHost)
          : null}

      <div className="project-workspace-shell project-workspace-editor-only">
        <section aria-labelledby="editor-title" className="editor-card real-editor-card">
          <div className="editor-heading">
            <div>
              <p className="section-kicker">{editorCopy.kicker}</p>
              <h2 id="editor-title">{selectedPath ?? editorCopy.heading}</h2>
            </div>
            <span
              data-state={
                busy === "save"
                  ? "saving"
                  : saveError || externalConflict
                    ? "error"
                    : dirty
                      ? "dirty"
                      : document?.readOnly
                        ? "readonly"
                        : "saved"
              }
            >
              {editorStatus}
            </span>
          </div>

          {document ? (
            <textarea
              aria-describedby="editor-note"
              aria-label={editorCopy.ariaLabel}
              onChange={(event) => {
                if (!document.readOnly) {
                  setDraft(event.target.value);
                  dirtyRef.current = event.target.value !== document.content;
                  setSaveError(null);
                  setValidationStale(true);
                  setValidationError(null);
                  setEditorNotice(null);
                }
              }}
              onKeyDown={(event) => {
                if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
                  event.preventDefault();
                  void saveCurrentFile();
                }
              }}
              readOnly={document.readOnly || busy === "save"}
              ref={editorRef}
              spellCheck={false}
              value={draft}
            />
          ) : (
            <div className="editor-empty" role={editorError ? "alert" : undefined}>
              <span aria-hidden="true">{editorError ? "!" : "#"}</span>
              <strong>{editorError ? editorError.title : editorCopy.emptyTitle}</strong>
              <p>{editorError ? editorError.message : editorCopy.emptyBody}</p>
              {editorError ? <p>{editorError.recovery}</p> : null}
              {selectedPath && editorError ? <code>{selectedPath}</code> : null}
            </div>
          )}

          <SourceDiagnostics
            error={validationError}
            onNavigate={(line) => focusEditorAtLine(draft, line)}
            stale={validationStale || externalConflict !== null}
            report={validation}
          />

          {document && externalConflict ? (
            <div className="project-error editor-save-error external-change-alert" role="alert">
              <strong>{editorCopy.externalChange.title}</strong>
              <p>
                {externalConflict.state === "missing"
                  ? editorCopy.externalChange.missing
                  : editorCopy.externalChange.modified}
              </p>
              <p>{editorCopy.externalChange.body}</p>
              <div className="project-confirm-actions">
                <button onClick={() => void loadFile(document.relativePath)} type="button">
                  {editorCopy.externalChange.reload}
                </button>
                <button
                  onClick={() => {
                    setEditorNotice(editorCopy.externalChange.kept);
                    requestAnimationFrame(() => editorRef.current?.focus());
                  }}
                  type="button"
                >
                  {editorCopy.externalChange.keep}
                </button>
                <button onClick={() => void copyDraftToClipboard()} type="button">
                  {editorCopy.externalChange.copy}
                </button>
              </div>
            </div>
          ) : null}

          {document && watchError && !externalConflict ? (
            <div className="project-error editor-save-error" role="alert">
              <strong>{watchError.title}</strong>
              <p>{watchError.message}</p>
              <p>{watchError.recovery}</p>
            </div>
          ) : null}

          {document && saveError ? (
            <div className="project-error editor-save-error" role="alert">
              <strong>{saveError.title}</strong>
              <p>{saveError.message}</p>
              <p>{saveError.recovery}</p>
              <code>
                {copy.errorCode}：{saveError.code}
              </code>
            </div>
          ) : null}
          <div className="editor-footer" id="editor-note">
            <div className="editor-meta">
              {document ? (
                <>
                  <span>
                    {draft.length} {ZH_CN.files.characterUnit}
                  </span>
                  <span>{editorCopy.lineEndings[document.lineEnding]}</span>
                  <span>{editorCopy.bom[document.bom]}</span>
                </>
              ) : (
                <span>{editorCopy.processMemoryOnly}</span>
              )}
            </div>
            <button
              disabled={
                !document ||
                document.readOnly ||
                !dirty ||
                externalConflict !== null ||
                busy !== null
              }
              onClick={() => void saveCurrentFile()}
              type="button"
            >
              {busy === "save" ? editorCopy.saving : editorCopy.save}
            </button>
          </div>
          <p aria-live="polite" className="editor-notice">
            {editorNotice ?? editorCopy.saveBoundary}
          </p>
        </section>

        {pendingAction ? (
          <div
            aria-labelledby="discard-title"
            aria-modal="true"
            className="discard-backdrop"
            role="alertdialog"
          >
            <div className="discard-dialog">
              <p className="section-kicker">{editorCopy.unsaved}</p>
              <h2 id="discard-title">{editorCopy.discardTitle}</h2>
              <p>{discardBody}</p>
              <div className="discard-actions">
                <button
                  onClick={() => {
                    setPendingAction(null);
                    requestAnimationFrame(() => editorRef.current?.focus());
                  }}
                  type="button"
                >
                  {editorCopy.keepEditing}
                </button>
                <button
                  className="discard-confirm"
                  onClick={() => void confirmDiscard()}
                  type="button"
                >
                  {editorCopy.discard}
                </button>
              </div>
            </div>
          </div>
        ) : null}
      </div>
    </>
  );
}
