import { useCallback, useEffect, useState, type FormEvent } from "react";
import { presentAppError, type PresentedAppError } from "../errors/present-app-error";
import { ZH_CN } from "../i18n/zh-CN";
import type { AppError } from "../lib/tauri/health";
import {
  requestDeleteProviderKey,
  requestProviderConnectionTest,
  requestProviderStatus,
  requestSaveProviderKey,
  requestSelectProvider,
  type ProviderId,
  type ProviderStateSnapshot,
  type ProviderStatus,
} from "../lib/tauri/provider";
import {
  DEFAULT_PROVIDER_STATE,
  getProviderConnectionLabel,
  PROVIDER_PRESENTATION,
} from "./provider-settings-model";

const PROVIDER_ORDER: readonly ProviderId[] = ["deepseek", "openai"];

type ProviderSettingsProps = {
  desktopRuntime: boolean;
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
  onStateChange?: (snapshot: ProviderStateSnapshot) => void;
  runActive?: boolean;
};

type BusyAction = {
  providerId: ProviderId;
  action: "save" | "delete" | "select" | "test";
};

function toPresentedError(error: unknown): PresentedAppError {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string" &&
    "messageKey" in error &&
    typeof error.messageKey === "string"
  ) {
    return presentAppError(error as AppError);
  }
  return presentAppError({
    code: "runtime.unknown",
    messageKey: "errors.unknown",
  });
}

function mergeProviderStatus(
  snapshot: ProviderStateSnapshot,
  status: ProviderStatus,
): ProviderStateSnapshot {
  const providers = snapshot.providers.map((provider) => {
    if (provider.providerId === status.providerId) {
      return status;
    }
    return status.selected ? { ...provider, selected: false } : provider;
  });
  return {
    schemaVersion: 2,
    providers,
    selectedProviderId: status.selected ? status.providerId : snapshot.selectedProviderId,
  };
}

export function ProviderSettings({
  desktopRuntime,
  expanded,
  onExpandedChange,
  onStateChange,
  runActive = false,
}: ProviderSettingsProps) {
  const [snapshot, setSnapshot] = useState(DEFAULT_PROVIDER_STATE);
  const [ready, setReady] = useState(false);
  const [keyValues, setKeyValues] = useState<Record<ProviderId, string>>({
    deepseek: "",
    openai: "",
  });
  const [busy, setBusy] = useState<BusyAction | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<PresentedAppError | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ProviderId | null>(null);
  const [connectionTarget, setConnectionTarget] = useState<ProviderId | null>(null);

  useEffect(() => {
    onStateChange?.(snapshot);
  }, [onStateChange, snapshot]);

  const loadStatus = useCallback(async () => {
    if (!desktopRuntime) {
      return;
    }
    try {
      const next = await requestProviderStatus();
      setError(null);
      setSnapshot(next);
      setReady(true);
    } catch (loadError: unknown) {
      setReady(false);
      setError(toPresentedError(loadError));
    }
  }, [desktopRuntime]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void loadStatus();
    }, 0);
    return () => window.clearTimeout(timer);
  }, [loadStatus]);

  const updateStatus = (status: ProviderStatus) => {
    setSnapshot((current) => mergeProviderStatus(current, status));
  };

  const saveKey = async (providerId: ProviderId) => {
    const submittedKey = keyValues[providerId];
    if (!desktopRuntime || !ready || submittedKey.length === 0 || busy !== null) {
      return;
    }
    setKeyValues((current) => ({ ...current, [providerId]: "" }));
    setBusy({ providerId, action: "save" });
    setNotice(null);
    setError(null);
    try {
      const status = await requestSaveProviderKey(providerId, submittedKey);
      updateStatus(status);
      setNotice(ZH_CN.api.keySaved(PROVIDER_PRESENTATION[providerId].name));
    } catch (saveError: unknown) {
      setError(toPresentedError(saveError));
    } finally {
      setKeyValues((current) => ({ ...current, [providerId]: "" }));
      setBusy(null);
    }
  };

  const deleteKey = async (providerId: ProviderId) => {
    if (!desktopRuntime || !ready || busy !== null) {
      return;
    }
    setDeleteTarget(null);
    setBusy({ providerId, action: "delete" });
    setNotice(null);
    setError(null);
    try {
      const status = await requestDeleteProviderKey(providerId, true);
      updateStatus(status);
      setNotice(ZH_CN.api.keyDeleted(PROVIDER_PRESENTATION[providerId].name));
    } catch (deleteError: unknown) {
      setError(toPresentedError(deleteError));
    } finally {
      setBusy(null);
    }
  };

  const selectProvider = async (providerId: ProviderId) => {
    if (!desktopRuntime || !ready || busy !== null || runActive) {
      return;
    }
    setKeyValues({
      deepseek: "",
      openai: "",
    });
    setBusy({ providerId, action: "select" });
    setNotice(null);
    setError(null);
    try {
      const status = await requestSelectProvider(providerId);
      updateStatus(status);
      setNotice(ZH_CN.providers.selectedNotice(PROVIDER_PRESENTATION[providerId].name));
    } catch (selectError: unknown) {
      setError(toPresentedError(selectError));
    } finally {
      setBusy(null);
    }
  };

  const testConnection = async (providerId: ProviderId) => {
    if (!desktopRuntime || !ready || busy !== null) {
      return;
    }
    setConnectionTarget(null);
    setBusy({ providerId, action: "test" });
    setNotice(null);
    setError(null);
    try {
      const status = await requestProviderConnectionTest(providerId);
      updateStatus(status);
      setNotice(ZH_CN.connection.succeeded(PROVIDER_PRESENTATION[providerId].name));
    } catch (testError: unknown) {
      setError(toPresentedError(testError));
      try {
        const next = await requestProviderStatus();
        setSnapshot(next);
      } catch {
        // Preserve the original, stable Provider error.
      }
    } finally {
      setBusy(null);
    }
  };

  const submitKey = (event: FormEvent<HTMLFormElement>, providerId: ProviderId) => {
    event.preventDefault();
    void saveKey(providerId);
  };

  const selectedProviderId = snapshot.selectedProviderId;
  const selectedStatus =
    snapshot.providers.find((provider) => provider.providerId === selectedProviderId) ??
    DEFAULT_PROVIDER_STATE.providers.find(
      (provider) => provider.providerId === selectedProviderId,
    )!;
  const selectedPresentation = PROVIDER_PRESENTATION[selectedProviderId];
  const selectedProviderBusy = busy?.providerId === selectedProviderId;
  const selectedInputDisabled = !desktopRuntime || !ready || selectedProviderBusy;
  const selectedStatusId = `${selectedProviderId}-provider-status`;
  const selectedInputId = `${selectedProviderId}-key`;

  return (
    <>
      <section aria-labelledby="provider-title" className="panel-section">
        <div className="section-title-row">
          <h2 id="provider-title">{ZH_CN.providers.available}</h2>
          <span className="quiet-pill">{ZH_CN.providers.fixedCount}</span>
        </div>
        {PROVIDER_ORDER.map((providerId) => {
          const status = snapshot.providers.find((provider) => provider.providerId === providerId);
          const presentation = PROVIDER_PRESENTATION[providerId];
          const visibleStatus = status?.selected
            ? ZH_CN.providers.current
            : ready && status
              ? getProviderConnectionLabel(status.connectionStatus)
              : ZH_CN.providers.loading;
          return (
            <button
              aria-label={ZH_CN.providers.cardLabel(presentation.name, visibleStatus)}
              aria-pressed={status?.selected ?? false}
              className="provider-card"
              data-selected={status?.selected ?? false}
              disabled={!desktopRuntime || !ready || busy !== null || runActive}
              key={providerId}
              onClick={() => {
                if (!status?.selected) {
                  void selectProvider(providerId);
                }
              }}
              type="button"
            >
              <span aria-hidden="true" className={`provider-logo ${presentation.tone}-logo`}>
                {presentation.initial}
              </span>
              <div className="provider-copy">
                <strong>{presentation.name}</strong>
              </div>
              <span
                className="provider-state"
                data-status={status?.connectionStatus ?? "unconfigured"}
                id={`${providerId}-provider-status`}
              >
                {visibleStatus}
              </span>
            </button>
          );
        })}
      </section>

      <section aria-labelledby="api-card-title" className="api-card">
        <button
          aria-controls="api-key-fields"
          aria-expanded={expanded}
          aria-label={
            expanded
              ? ZH_CN.api.collapse(selectedPresentation.name)
              : ZH_CN.api.expand(selectedPresentation.name)
          }
          className="api-card-toggle"
          onClick={() => onExpandedChange(!expanded)}
          type="button"
        >
          <span>
            <small>{ZH_CN.api.heading}</small>
            <strong id="api-card-title">{selectedPresentation.name}</strong>
          </span>
          <span aria-hidden="true" className="chevron">
            {expanded ? "⌃" : "⌄"}
          </span>
        </button>

        {expanded ? (
          <div className="provider-settings-list" id="api-key-fields">
            <article className="provider-settings-card">
              <form onSubmit={(event) => submitKey(event, selectedProviderId)}>
                <label htmlFor={selectedInputId}>{selectedPresentation.inputLabel}</label>
                <input
                  aria-describedby={selectedStatusId}
                  autoCapitalize="none"
                  autoComplete="new-password"
                  disabled={selectedInputDisabled}
                  id={selectedInputId}
                  maxLength={2_560}
                  onChange={(event) =>
                    setKeyValues((current) => ({
                      ...current,
                      [selectedProviderId]: event.target.value,
                    }))
                  }
                  placeholder={ZH_CN.api.placeholder}
                  spellCheck={false}
                  type="password"
                  value={keyValues[selectedProviderId]}
                />
                <div className="provider-actions">
                  <button
                    className="provider-action-primary"
                    disabled={
                      selectedInputDisabled ||
                      busy !== null ||
                      keyValues[selectedProviderId].length === 0
                    }
                    type="submit"
                  >
                    {selectedProviderBusy && busy?.action === "save"
                      ? ZH_CN.api.saving
                      : selectedStatus.configured
                        ? ZH_CN.api.replace
                        : ZH_CN.api.save}
                  </button>
                  <button
                    disabled={
                      !desktopRuntime || !ready || busy !== null || !selectedStatus.configured
                    }
                    onClick={() => setDeleteTarget(selectedProviderId)}
                    type="button"
                  >
                    {selectedProviderBusy && busy?.action === "delete"
                      ? ZH_CN.api.deleting
                      : ZH_CN.api.delete}
                  </button>
                  <button
                    className="provider-test-button"
                    disabled={
                      !desktopRuntime || !ready || busy !== null || !selectedStatus.configured
                    }
                    onClick={() => setConnectionTarget(selectedProviderId)}
                    type="button"
                  >
                    {selectedProviderBusy && busy?.action === "test"
                      ? ZH_CN.connection.testing
                      : ZH_CN.api.connectionTest}
                  </button>
                </div>
              </form>
            </article>

            <p className="safety-note">{ZH_CN.api.safety}</p>
            {runActive ? (
              <p aria-live="polite" className="provider-runtime-note" role="status">
                {ZH_CN.providers.switchLocked}
              </p>
            ) : null}
            {!desktopRuntime ? (
              <p className="provider-runtime-note">{ZH_CN.api.desktopOnly}</p>
            ) : null}
            {notice ? (
              <p aria-live="polite" className="provider-notice" role="status">
                {notice}
              </p>
            ) : null}
            {error ? (
              <div className="provider-error" role="alert">
                <strong>{error.title}</strong>
                <p>{error.message}</p>
                <p>{error.recovery}</p>
                <code>
                  {ZH_CN.api.errorCode}：{error.code}
                </code>
                <button onClick={() => void loadStatus()} type="button">
                  {ZH_CN.api.retryStatus}
                </button>
              </div>
            ) : null}
          </div>
        ) : null}
      </section>

      <section aria-labelledby="connection-title" className="connection-card">
        <span aria-hidden="true" className="connection-icon">
          ⌁
        </span>
        <div>
          <h2 id="connection-title">{ZH_CN.connection.heading}</h2>
          <p>{ZH_CN.connection.body}</p>
        </div>
      </section>

      {deleteTarget ? (
        <div className="provider-dialog-backdrop">
          <section
            aria-labelledby="provider-delete-title"
            aria-modal="true"
            className="provider-dialog"
            role="dialog"
          >
            <h2 id="provider-delete-title">{ZH_CN.api.deleteDialogTitle}</h2>
            <p>{ZH_CN.api.deleteDialogBody(PROVIDER_PRESENTATION[deleteTarget].name)}</p>
            <div className="provider-dialog-actions">
              <button autoFocus onClick={() => setDeleteTarget(null)} type="button">
                {ZH_CN.api.cancel}
              </button>
              <button
                className="provider-delete-confirm"
                onClick={() => void deleteKey(deleteTarget)}
                type="button"
              >
                {ZH_CN.api.confirmDelete}
              </button>
            </div>
          </section>
        </div>
      ) : null}

      {connectionTarget ? (
        <div className="provider-dialog-backdrop">
          <section
            aria-labelledby="provider-test-title"
            aria-modal="true"
            className="provider-dialog"
            role="dialog"
          >
            <h2 id="provider-test-title">{ZH_CN.connection.dialogTitle}</h2>
            <dl>
              <div>
                <dt>{ZH_CN.connection.providerLabel}</dt>
                <dd>{PROVIDER_PRESENTATION[connectionTarget].name}</dd>
              </div>
              <div>
                <dt>{ZH_CN.connection.modelLabel}</dt>
                <dd>{PROVIDER_PRESENTATION[connectionTarget].model}</dd>
              </div>
            </dl>
            <p>{ZH_CN.connection.usage}</p>
            <p className="provider-dialog-boundary">{ZH_CN.connection.boundary}</p>
            <div className="provider-dialog-actions">
              <button autoFocus onClick={() => setConnectionTarget(null)} type="button">
                {ZH_CN.connection.close}
              </button>
              <button
                className="provider-action-primary"
                onClick={() => void testConnection(connectionTarget)}
                type="button"
              >
                {ZH_CN.connection.run}
              </button>
            </div>
          </section>
        </div>
      ) : null}
    </>
  );
}
