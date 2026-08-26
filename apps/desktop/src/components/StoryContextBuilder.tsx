import { useEffect, useMemo, useState } from "react";
import { presentAppError } from "../errors/present-app-error";
import { ZH_CN } from "../i18n/zh-CN";
import type { AppError } from "../lib/tauri/health";
import {
  requestStoryContextBuild,
  requestStoryContextCatalog,
  type StoryAction,
  type StoryContextCatalogEntry,
  type StoryContextCatalogSnapshot,
  type StoryContextManifest,
  type StoryContextManifestSource,
  type StoryContextSelectionReason,
  type StoryContextSourceKind,
} from "../lib/tauri/story-context";

type StoryContextBuilderProps = {
  sessionId: string;
  onClose: () => void;
};

const copy = ZH_CN.conversation.storyContext;

function shortHash(value: string): string {
  return `${value.slice(0, 10)}…${value.slice(-8)}`;
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
  const chapterValid =
    Number.isSafeInteger(chapterNumber) && chapterNumber >= 1 && chapterNumber <= 999_999;
  const canBuild = catalog !== null && chapterValid && chapterTitle.trim().length > 0 && !building;

  const toggleSource = (relativePath: string, checked: boolean) => {
    setManifest(null);
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
    try {
      setManifest(
        await requestStoryContextBuild(sessionId, {
          action,
          chapterNumber,
          chapterTitle,
          selectedSourcePaths: selectedPaths,
        }),
      );
    } catch (error: unknown) {
      setBuildError(error as AppError);
      setManifest(null);
    } finally {
      setBuilding(false);
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
                onChange={(event) => {
                  setAction(event.currentTarget.value as StoryAction);
                  setManifest(null);
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
                max={999_999}
                min={1}
                onChange={(event) => {
                  setChapterNumber(event.currentTarget.valueAsNumber);
                  setManifest(null);
                }}
                type="number"
                value={Number.isNaN(chapterNumber) ? "" : chapterNumber}
              />
            </label>
            <label>
              {copy.chapterTitleLabel}
              <input
                maxLength={120}
                onChange={(event) => {
                  setChapterTitle(event.currentTarget.value);
                  setManifest(null);
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
          </div>
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
