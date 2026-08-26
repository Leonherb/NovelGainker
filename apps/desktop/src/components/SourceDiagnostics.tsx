import type { PresentedAppError } from "../errors/present-app-error";
import { ZH_CN } from "../i18n/zh-CN";
import type { ValidationReport } from "../lib/tauri/analysis";

type SourceDiagnosticsProps = {
  report: ValidationReport | null;
  error: PresentedAppError | null;
  onNavigate: (line: number) => void;
  stale: boolean;
};

export function SourceDiagnostics({ report, error, onNavigate, stale }: SourceDiagnosticsProps) {
  const aiApplyEligible = !stale && report?.eligibleForAiApply === true;
  const finalEligible = !stale && report?.eligibleForFinal === true;
  const epubEligible = !stale && report?.eligibleForEpub === true;
  const copy = ZH_CN.files.editor.diagnostics;
  if (!report && !error) {
    return null;
  }

  return (
    <section
      aria-label={copy.ariaLabel}
      className="source-diagnostics"
      data-stale={stale}
      data-valid={!stale && report?.diagnostics.length === 0}
    >
      {report ? (
        <>
          <div className="source-diagnostics-heading">
            <div>
              <strong>
                {stale
                  ? copy.staleTitle
                  : report.diagnostics.length === 0
                    ? copy.validTitle
                    : copy.issueTitle}
              </strong>
              <small>
                {stale
                  ? copy.staleBody
                  : report.diagnostics.length === 0
                    ? copy.validBody
                    : copy.issueCount.replace("{count}", String(report.diagnostics.length))}
              </small>
            </div>
            <span>{report.contentHash.slice(0, 8)}</span>
          </div>
          <div aria-label={copy.eligibilityAriaLabel} className="source-eligibility">
            <span data-eligible={aiApplyEligible}>
              {copy.aiApply} · {aiApplyEligible ? copy.available : copy.paused}
            </span>
            <span data-eligible={finalEligible}>
              {copy.final} · {finalEligible ? copy.available : copy.paused}
            </span>
            <span data-eligible={epubEligible}>
              {copy.epub} · {epubEligible ? copy.available : copy.paused}
            </span>
          </div>
          {report.diagnostics.length > 0 ? (
            <ul>
              {report.diagnostics.map((diagnostic) => (
                <li key={`${diagnostic.code}:${diagnostic.line}:${diagnostic.column}`}>
                  <button onClick={() => onNavigate(diagnostic.line)} type="button">
                    <span>
                      <strong>{diagnostic.message}</strong>
                      <small>
                        {diagnostic.code} · {copy.errorSeverity} · {diagnostic.relativePath}:
                        {diagnostic.line}:{diagnostic.column}
                      </small>
                    </span>
                    <span>{diagnostic.recovery}</span>
                  </button>
                </li>
              ))}
            </ul>
          ) : null}
        </>
      ) : null}
      {error ? (
        <div className="project-error" role="alert">
          <strong>{error.title}</strong>
          <p>{error.message}</p>
          <p>{error.recovery}</p>
        </div>
      ) : null}
    </section>
  );
}
