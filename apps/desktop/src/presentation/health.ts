import { ZH_CN } from "../i18n/zh-CN";
import type { AppError, HealthStatus } from "../lib/tauri/health";

export type HealthViewState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "ready"; value: HealthStatus }
  | { kind: "error"; error: AppError };

export type PresentedHealthStatus = {
  title: string;
  detail: string;
  topLine: string;
};

export function presentHealthStatus(status: HealthStatus): PresentedHealthStatus {
  const contract = `${ZH_CN.health.contractLabel} v${status.contractVersion}`;

  return {
    title: ZH_CN.health.readyTitle,
    detail: `${status.application} v${status.version} · ${status.platform} · ${contract}`,
    topLine: `${ZH_CN.health.topReady} · v${status.version} · ${contract}`,
  };
}
