import { presentAppError } from "../errors/present-app-error";
import { ZH_CN } from "../i18n/zh-CN";
import { presentHealthStatus, type HealthViewState } from "../presentation/health";

export function HealthRuntimeSummary({ state }: { state: HealthViewState }) {
  const text =
    state.kind === "ready"
      ? presentHealthStatus(state.value).topLine
      : state.kind === "checking"
        ? ZH_CN.health.topChecking
        : state.kind === "error"
          ? `${ZH_CN.health.topError} · ${presentAppError(state.error).code}`
          : ZH_CN.health.topIdle;

  return (
    <div aria-live="polite" className="runtime-status" data-state={state.kind} role="status">
      <span aria-hidden="true" className="status-light" />
      {text}
    </div>
  );
}

type HealthStatusCardProps = {
  state: HealthViewState;
  desktopRuntime: boolean;
  diagnosticsEnabled: boolean;
  onCheck: () => void;
};

export function HealthStatusCard({
  state,
  desktopRuntime,
  diagnosticsEnabled,
  onCheck,
}: HealthStatusCardProps) {
  const ready = state.kind === "ready" ? presentHealthStatus(state.value) : null;
  const error = state.kind === "error" ? presentAppError(state.error) : null;
  const action =
    state.kind === "checking"
      ? ZH_CN.health.checkingAction
      : state.kind === "ready" || state.kind === "error"
        ? ZH_CN.health.retry
        : ZH_CN.health.run;

  return (
    <section
      aria-labelledby="system-status-title"
      className="system-status-card"
      data-state={state.kind}
    >
      <div className="system-status-heading">
        <div>
          <p className="section-kicker">{ZH_CN.health.kicker}</p>
          <h2 id="system-status-title">{ZH_CN.health.heading}</h2>
        </div>
        <span aria-hidden="true" className="system-status-light" />
      </div>

      <div aria-live="polite" className="system-status-copy" role="status">
        {ready ? (
          <>
            <strong>{ready.title}</strong>
            <span>{ready.detail}</span>
          </>
        ) : error ? (
          <>
            <strong>{error.title}</strong>
            <span className="status-message">{error.message}</span>
            <span className="status-recovery">{error.recovery}</span>
            <span>
              {ZH_CN.health.errorCodeLabel}：{error.code}
            </span>
            {diagnosticsEnabled && error.diagnostic ? (
              <details className="diagnostic-details">
                <summary>{ZH_CN.health.diagnosticSummary}</summary>
                <code>{error.diagnostic}</code>
              </details>
            ) : null}
          </>
        ) : (
          <>
            <strong>
              {state.kind === "checking" ? ZH_CN.health.checkingTitle : ZH_CN.health.idleTitle}
            </strong>
            <span>
              {state.kind === "checking"
                ? ZH_CN.health.checkingBody
                : desktopRuntime
                  ? ZH_CN.health.idleDesktopBody
                  : ZH_CN.health.idleBrowserBody}
            </span>
          </>
        )}
      </div>

      <button
        aria-label={ZH_CN.health.actionAriaLabel}
        className="health-check-button"
        disabled={!desktopRuntime || state.kind === "checking"}
        onClick={onCheck}
        type="button"
      >
        {action}
      </button>
    </section>
  );
}
