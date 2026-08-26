import { ZH_CN } from "../i18n/zh-CN";
import type {
  ProviderConnectionStatus,
  ProviderId,
  ProviderStateSnapshot,
} from "../lib/tauri/provider";

type ProviderPresentation = {
  name: string;
  initial: string;
  model: string;
  tone: ProviderId;
  inputLabel: string;
};

export const PROVIDER_PRESENTATION: Record<ProviderId, ProviderPresentation> = {
  deepseek: {
    name: ZH_CN.providers.deepSeek,
    initial: "D",
    model: "deepseek-v4-pro",
    tone: "deepseek",
    inputLabel: ZH_CN.api.deepSeekLabel,
  },
  openai: {
    name: ZH_CN.providers.openAi,
    initial: "O",
    model: "gpt-5.6-terra",
    tone: "openai",
    inputLabel: ZH_CN.api.openAiLabel,
  },
};

export const DEFAULT_PROVIDER_STATE: ProviderStateSnapshot = {
  schemaVersion: 2,
  providers: [
    {
      providerId: "deepseek",
      configured: false,
      connectionStatus: "unconfigured",
      presetVersion: 1,
      selected: false,
    },
    {
      providerId: "openai",
      configured: false,
      connectionStatus: "unconfigured",
      presetVersion: 1,
      selected: true,
    },
  ],
  selectedProviderId: "openai",
};

const STATUS_COPY: Record<ProviderConnectionStatus, string> = {
  unconfigured: ZH_CN.providers.unconfigured,
  untested: ZH_CN.providers.untested,
  connected: ZH_CN.providers.connected,
  authenticationFailed: ZH_CN.providers.authenticationFailed,
  unreachable: ZH_CN.providers.unreachable,
  vaultUnavailable: ZH_CN.providers.vaultUnavailable,
  recoveryRequired: ZH_CN.providers.recoveryRequired,
};

export function getProviderConnectionLabel(status: ProviderConnectionStatus): string {
  return STATUS_COPY[status];
}
