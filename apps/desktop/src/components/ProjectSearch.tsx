import type { PresentedAppError } from "../errors/present-app-error";
import { ZH_CN } from "../i18n/zh-CN";
import type { SearchReport } from "../lib/tauri/analysis";

type ProjectSearchProps = {
  query: string;
  caseSensitive: boolean;
  report: SearchReport | null;
  error: PresentedAppError | null;
  busy: boolean;
  onQueryChange: (value: string) => void;
  onCaseSensitiveChange: (value: boolean) => void;
  onSearch: () => void;
  onNavigate: (relativePath: string, line: number) => void;
};

export function ProjectSearch({
  query,
  caseSensitive,
  report,
  error,
  busy,
  onQueryChange,
  onCaseSensitiveChange,
  onSearch,
  onNavigate,
}: ProjectSearchProps) {
  const copy = ZH_CN.files.search;
  return (
    <form
      aria-label={copy.ariaLabel}
      className="project-search"
      onSubmit={(event) => {
        event.preventDefault();
        onSearch();
      }}
    >
      <label htmlFor="project-search-query">{copy.label}</label>
      <div className="project-search-row">
        <input
          autoComplete="off"
          id="project-search-query"
          maxLength={128}
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder={copy.placeholder}
          spellCheck={false}
          type="search"
          value={query}
        />
        <button disabled={busy || query.length === 0} type="submit">
          {busy ? copy.searching : copy.submit}
        </button>
      </div>
      <label className="project-search-option">
        <input
          checked={caseSensitive}
          onChange={(event) => onCaseSensitiveChange(event.target.checked)}
          type="checkbox"
        />
        <span>{copy.caseSensitive}</span>
      </label>

      {report ? (
        <div aria-live="polite" className="project-search-results">
          <p>
            {copy.summary
              .replace("{count}", String(report.matches.length))
              .replace("{files}", String(report.scannedFileCount))}
          </p>
          {report.matches.length > 0 ? (
            <ul>
              {report.matches.map((match) => (
                <li key={`${match.relativePath}:${match.line}:${match.column}`}>
                  <button onClick={() => onNavigate(match.relativePath, match.line)} type="button">
                    <strong>{match.relativePath}</strong>
                    <small>
                      {copy.position
                        .replace("{line}", String(match.line))
                        .replace("{column}", String(match.column))}
                    </small>
                    <span>{match.snippet}</span>
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p>{copy.empty}</p>
          )}
          {report.truncated ? <p>{copy.truncated}</p> : null}
        </div>
      ) : null}

      {error ? (
        <div className="project-error" role="alert">
          <strong>{error.title}</strong>
          <p>{error.message}</p>
          <p>{error.recovery}</p>
        </div>
      ) : null}
    </form>
  );
}
