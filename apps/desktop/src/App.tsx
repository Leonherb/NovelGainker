import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import "./App.css";
import { AgentTimelinePanel } from "./components/AgentTimelinePanel";
import { HealthRuntimeSummary, HealthStatusCard } from "./components/HealthStatus";
import { ProviderSettings } from "./components/ProviderSettings";
import { DEFAULT_PROVIDER_STATE } from "./components/provider-settings-model";
import { ProjectWorkspace } from "./components/ProjectWorkspace";
import type { HealthViewState } from "./presentation/health";
import { ZH_CN } from "./i18n/zh-CN";
import {
  requestAppState,
  requestSaveUiPreferences,
  type RecentProject,
} from "./lib/tauri/app-state";
import type { ProjectSession } from "./lib/tauri/project";
import { isTauriRuntime, normalizeAppError, requestHealthStatus } from "./lib/tauri/health";

const NARROW_LAYOUT_QUERY = "(max-width: 900px)";
const COMPACT_WIDE_QUERY = "(max-width: 1120px)";
const DEFAULT_RIGHT_PANEL_WIDTH = 350;
const COMPACT_RIGHT_PANEL_WIDTH = 304;
const MIN_RIGHT_PANEL_WIDTH = 278;
const RIGHT_PANEL_RESIZE_STEP = 32;

function useNarrowLayout() {
  const [isNarrow, setIsNarrow] = useState(() =>
    typeof window.matchMedia === "function"
      ? window.matchMedia(NARROW_LAYOUT_QUERY).matches
      : false,
  );

  useEffect(() => {
    if (typeof window.matchMedia !== "function") {
      return undefined;
    }

    const mediaQuery = window.matchMedia(NARROW_LAYOUT_QUERY);
    const handleChange = (event: MediaQueryListEvent) => {
      setIsNarrow(event.matches);
    };

    mediaQuery.addEventListener("change", handleChange);
    return () => mediaQuery.removeEventListener("change", handleChange);
  }, []);

  return isNarrow;
}

export type AppProps = {
  diagnosticsEnabled?: boolean;
};

function App({ diagnosticsEnabled = false }: AppProps) {
  const isNarrow = useNarrowLayout();
  const [leftCollapsed, setLeftCollapsed] = useState(false);
  const [rightCollapsed, setRightCollapsed] = useState(false);
  const [rightPanelWidth, setRightPanelWidth] = useState<number | null>(null);
  const [rightPanelResizing, setRightPanelResizing] = useState(false);
  const [narrowPanel, setNarrowPanel] = useState<"left" | "right" | null>(null);
  const [apiCardExpanded, setApiCardExpanded] = useState(true);
  const [appStateReady, setAppStateReady] = useState(false);
  const [appStateWritable, setAppStateWritable] = useState(false);
  const [recentProjects, setRecentProjects] = useState<RecentProject[]>([]);
  const [appStateNotice, setAppStateNotice] = useState<string | null>(null);
  const [healthState, setHealthState] = useState<HealthViewState>({
    kind: "idle",
  });
  const [providerState, setProviderState] = useState(DEFAULT_PROVIDER_STATE);
  const [providerRunActive, setProviderRunActive] = useState(false);
  const healthCheckStarted = useRef(false);
  const workspaceRef = useRef<HTMLDivElement>(null);
  const leftPanelRef = useRef<HTMLElement>(null);
  const rightPanelRef = useRef<HTMLElement>(null);
  const rightResizePointerRef = useRef<number | null>(null);
  const [projectSessionId, setProjectSessionId] = useState<string | null>(null);
  const [workspaceRefreshToken] = useState(0);
  const [workspaceOpenPath] = useState<string | null>(null);
  const desktopRuntime = isTauriRuntime();

  const runHealthCheck = useCallback(async () => {
    if (!desktopRuntime) {
      return;
    }

    setHealthState({ kind: "checking" });
    try {
      const value = await requestHealthStatus();
      setHealthState({ kind: "ready", value });
    } catch (error: unknown) {
      setHealthState({ kind: "error", error: normalizeAppError(error) });
    }
  }, [desktopRuntime]);
  const handleProjectSessionChange = useCallback((session: ProjectSession | null) => {
    setProjectSessionId(session?.sessionId ?? null);
  }, []);

  const loadAppState = useCallback(
    async (applyUi: boolean) => {
      if (!desktopRuntime) {
        return;
      }
      try {
        const snapshot = await requestAppState();
        setRecentProjects(snapshot.recentProjects);
        setAppStateReady(true);
        setAppStateWritable(!snapshot.recoveredWithDefaults);
        setAppStateNotice(snapshot.recoveredWithDefaults ? ZH_CN.appState.recoveredDefaults : null);
        if (applyUi) {
          setLeftCollapsed(snapshot.ui.leftCollapsed);
          setRightCollapsed(snapshot.ui.rightCollapsed);
          setApiCardExpanded(snapshot.ui.apiCardExpanded);
          setRightPanelWidth(snapshot.ui.rightPanelWidth);
        }
      } catch {
        setAppStateReady(true);
        setAppStateWritable(false);
        setRecentProjects([]);
        setAppStateNotice(ZH_CN.appState.loadFailed);
      }
    },
    [desktopRuntime],
  );

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void loadAppState(true);
    }, 0);
    return () => window.clearTimeout(timer);
  }, [loadAppState]);

  useEffect(() => {
    if (!desktopRuntime || !appStateReady || !appStateWritable || isNarrow) {
      return undefined;
    }
    const timer = window.setTimeout(() => {
      void requestSaveUiPreferences({
        leftCollapsed,
        rightCollapsed,
        apiCardExpanded,
        rightPanelWidth,
      }).catch(() => setAppStateNotice(ZH_CN.appState.saveFailed));
    }, 200);
    return () => window.clearTimeout(timer);
  }, [
    apiCardExpanded,
    appStateReady,
    appStateWritable,
    desktopRuntime,
    isNarrow,
    leftCollapsed,
    rightCollapsed,
    rightPanelWidth,
  ]);

  useEffect(() => {
    if (!desktopRuntime || healthCheckStarted.current) {
      return;
    }

    healthCheckStarted.current = true;
    void runHealthCheck();
  }, [desktopRuntime, runHealthCheck]);

  const leftPanelOpen = isNarrow ? narrowPanel === "left" : !leftCollapsed;
  const rightPanelOpen = isNarrow ? narrowPanel === "right" : !rightCollapsed;

  const toggleLeftPanel = () => {
    if (isNarrow) {
      setNarrowPanel((current) => (current === "left" ? null : "left"));
      return;
    }
    setLeftCollapsed((current) => !current);
  };

  const toggleRightPanel = () => {
    if (isNarrow) {
      setNarrowPanel((current) => (current === "right" ? null : "right"));
      return;
    }
    setRightCollapsed((current) => !current);
  };

  const revealProjectCreation = () => {
    setRightCollapsed(false);
    if (isNarrow) {
      setNarrowPanel("right");
    }
    window.requestAnimationFrame(() => {
      document.getElementById("project-create-action")?.focus();
    });
  };

  const clampRightPanelWidth = useCallback(
    (requestedWidth: number) => {
      const compactWide =
        window.innerWidth <= 1_120 ||
        (typeof window.matchMedia === "function" && window.matchMedia(COMPACT_WIDE_QUERY).matches);
      const workspaceWidth =
        workspaceRef.current?.getBoundingClientRect().width || window.innerWidth;
      const measuredLeftWidth = leftPanelOpen
        ? leftPanelRef.current?.getBoundingClientRect().width
        : 0;
      const leftWidth = leftPanelOpen ? measuredLeftWidth || (compactWide ? 245 : 294) : 0;
      const minimumConversationWidth = compactWide ? 360 : 400;
      const maximumWidth = Math.min(
        1_600,
        Math.max(MIN_RIGHT_PANEL_WIDTH, workspaceWidth - leftWidth - minimumConversationWidth),
      );
      return Math.min(maximumWidth, Math.max(MIN_RIGHT_PANEL_WIDTH, requestedWidth));
    },
    [leftPanelOpen],
  );

  const currentRightPanelWidth = () => {
    const measured = rightPanelRef.current?.getBoundingClientRect().width;
    if (measured) {
      return measured;
    }
    if (rightPanelWidth !== null) {
      return rightPanelWidth;
    }
    const compactWide =
      window.innerWidth <= 1_120 ||
      (typeof window.matchMedia === "function" && window.matchMedia(COMPACT_WIDE_QUERY).matches);
    return compactWide ? COMPACT_RIGHT_PANEL_WIDTH : DEFAULT_RIGHT_PANEL_WIDTH;
  };

  const resizeRightPanelFromClientX = (clientX: number) => {
    const workspaceRight = workspaceRef.current?.getBoundingClientRect().right || window.innerWidth;
    setRightPanelWidth(clampRightPanelWidth(workspaceRight - clientX));
  };

  const handleRightResizePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    rightResizePointerRef.current = event.pointerId;
    event.currentTarget.setPointerCapture?.(event.pointerId);
    setRightPanelResizing(true);
    resizeRightPanelFromClientX(event.clientX);
  };

  const handleRightResizePointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (rightResizePointerRef.current !== event.pointerId) {
      return;
    }
    resizeRightPanelFromClientX(event.clientX);
  };

  const stopRightResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (rightResizePointerRef.current !== event.pointerId) {
      return;
    }
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    rightResizePointerRef.current = null;
    setRightPanelResizing(false);
  };

  const handleRightResizeKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Home") {
      event.preventDefault();
      setRightPanelWidth(null);
      return;
    }
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
      return;
    }
    event.preventDefault();
    const direction = event.key === "ArrowLeft" ? 1 : -1;
    setRightPanelWidth(
      clampRightPanelWidth(currentRightPanelWidth() + direction * RIGHT_PANEL_RESIZE_STEP),
    );
  };

  useEffect(() => {
    if (isNarrow) {
      return undefined;
    }
    const keepWidthInBounds = () => {
      setRightPanelWidth((current) => (current === null ? null : clampRightPanelWidth(current)));
    };
    window.addEventListener("resize", keepWidthInBounds);
    keepWidthInBounds();
    return () => window.removeEventListener("resize", keepWidthInBounds);
  }, [clampRightPanelWidth, isNarrow]);

  const shellStyle =
    rightPanelWidth === null
      ? undefined
      : ({ "--right-panel-width": `${rightPanelWidth}px` } as CSSProperties);

  return (
    <div
      className="app-shell"
      data-layout={isNarrow ? "narrow" : "wide"}
      data-left-collapsed={!leftPanelOpen}
      data-right-collapsed={!rightPanelOpen}
      data-right-resizing={rightPanelResizing}
      style={shellStyle}
    >
      <header className="titlebar">
        <div className="brand" aria-label={ZH_CN.brand.name}>
          <span className="brand-mark" aria-hidden="true">
            N
          </span>
          <span className="brand-copy">
            <strong>{ZH_CN.brand.name}</strong>
            <small>{ZH_CN.brand.subtitle}</small>
          </span>
        </div>

        <nav className="panel-controls" aria-label={ZH_CN.panels.controls}>
          <button
            aria-controls="provider-sidebar"
            aria-expanded={leftPanelOpen}
            aria-label={leftPanelOpen ? ZH_CN.panels.collapseApi : ZH_CN.panels.expandApi}
            className="panel-toggle"
            onClick={toggleLeftPanel}
            type="button"
          >
            <span aria-hidden="true">◧</span>
            <span>{ZH_CN.panels.api}</span>
          </button>
          <button
            aria-controls="file-sidebar"
            aria-expanded={rightPanelOpen}
            aria-label={rightPanelOpen ? ZH_CN.panels.collapseFiles : ZH_CN.panels.expandFiles}
            className="panel-toggle"
            onClick={toggleRightPanel}
            type="button"
          >
            <span aria-hidden="true">◨</span>
            <span>{ZH_CN.panels.files}</span>
          </button>
        </nav>

        <HealthRuntimeSummary state={healthState} />
      </header>

      <div className="workspace" ref={workspaceRef}>
        {isNarrow && (leftPanelOpen || rightPanelOpen) ? (
          <button
            aria-label={ZH_CN.panels.closeDrawer}
            className="drawer-backdrop"
            onClick={() => setNarrowPanel(null)}
            type="button"
          />
        ) : null}

        {leftPanelOpen ? (
          <aside
            aria-label={ZH_CN.providers.ariaLabel}
            className="side-panel left-panel"
            id="provider-sidebar"
            ref={leftPanelRef}
          >
            <header className="panel-heading">
              <div>
                <p className="section-kicker">{ZH_CN.providers.kicker}</p>
                <h1>{ZH_CN.providers.heading}</h1>
              </div>
              <button
                aria-label={ZH_CN.panels.collapseApiSettings}
                className="icon-button"
                onClick={toggleLeftPanel}
                title={ZH_CN.panels.collapseApiSettings}
                type="button"
              >
                ‹
              </button>
            </header>

            <div className="panel-scroll">
              <ProviderSettings
                desktopRuntime={desktopRuntime}
                expanded={apiCardExpanded}
                onExpandedChange={setApiCardExpanded}
                onStateChange={setProviderState}
                runActive={providerRunActive}
              />

              <HealthStatusCard
                desktopRuntime={desktopRuntime}
                diagnosticsEnabled={diagnosticsEnabled}
                onCheck={() => void runHealthCheck()}
                state={healthState}
              />
            </div>
          </aside>
        ) : null}

        <main aria-label={ZH_CN.conversation.ariaLabel} className="conversation-panel">
          <AgentTimelinePanel
            desktopRuntime={desktopRuntime}
            projectSessionId={projectSessionId}
            onRunActiveChange={setProviderRunActive}
            onProjectCreateRequested={revealProjectCreation}
            providerState={providerState}
          />
        </main>

        <aside
          aria-label={ZH_CN.files.ariaLabel}
          className="side-panel right-panel"
          hidden={!rightPanelOpen}
          id="file-sidebar"
          ref={rightPanelRef}
        >
          {!isNarrow && rightPanelOpen ? (
            <div
              aria-label={ZH_CN.panels.resizeFiles}
              aria-orientation="vertical"
              aria-valuemax={1_600}
              aria-valuemin={MIN_RIGHT_PANEL_WIDTH}
              aria-valuenow={Math.round(
                rightPanelWidth ??
                  (window.innerWidth <= 1_120
                    ? COMPACT_RIGHT_PANEL_WIDTH
                    : DEFAULT_RIGHT_PANEL_WIDTH),
              )}
              className="panel-resize-handle"
              onDoubleClick={() => setRightPanelWidth(null)}
              onKeyDown={handleRightResizeKeyDown}
              onPointerCancel={stopRightResize}
              onPointerDown={handleRightResizePointerDown}
              onPointerMove={handleRightResizePointerMove}
              onPointerUp={stopRightResize}
              role="separator"
              tabIndex={0}
              title={ZH_CN.panels.resizeFilesTitle}
            />
          ) : null}
          <header className="panel-heading right-heading">
            <div>
              <p className="section-kicker">{ZH_CN.files.kicker}</p>
              <h1>{ZH_CN.files.heading}</h1>
            </div>
            <button
              aria-label={ZH_CN.panels.collapseFilePreview}
              className="icon-button"
              onClick={toggleRightPanel}
              title={ZH_CN.panels.collapseFilePreview}
              type="button"
            >
              ›
            </button>
          </header>

          <div className="right-content">
            <ProjectWorkspace
              desktopRuntime={desktopRuntime}
              externalOpenPath={workspaceOpenPath}
              externalRefreshToken={workspaceRefreshToken}
              onAppStateChanged={() => void loadAppState(false)}
              recentProjects={recentProjects}
              onSessionChange={handleProjectSessionChange}
            />
          </div>
        </aside>
      </div>

      <footer className="statusbar">
        <span>{ZH_CN.footer.workspace}</span>
        {appStateNotice ? <span role="status">{appStateNotice}</span> : null}
        <span>{ZH_CN.footer.encoding}</span>
      </footer>
    </div>
  );
}

export default App;
