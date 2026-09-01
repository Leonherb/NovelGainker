import { ZH_CN } from "../i18n/zh-CN";
import type { AppError } from "../lib/tauri/health";

export const APP_ERROR_CODES = {
  runtimeUnavailable: "runtime.unavailable",
  invalidRuntimeResponse: "contract.invalid_response",
  projectInvalidRoot: "project.invalid_root",
  projectMetadataInvalid: "project.metadata_invalid",
  projectConflict: "project.conflict",
  projectChanged: "project.changed",
  projectScaffoldChanged: "project.scaffold_changed",
  projectInvalidMetadata: "project.invalid_metadata",
  projectUnsupportedSchema: "project.unsupported_schema",
  projectConfirmationRequired: "project.confirmation_required",
  projectIo: "project.io",
  projectSessionNotFound: "project.session_not_found",
  projectLockUnavailable: "project.lock_unavailable",
  projectWriterRequired: "project.writer_required",
  projectRecoveryRequired: "project.recovery_required",
  fileInvalidPath: "file.invalid_path",
  fileNotFound: "file.not_found",
  fileNotAllowed: "file.not_allowed",
  fileTooLarge: "file.too_large",
  fileUnsupportedEncoding: "file.unsupported_encoding",
  fileIndexTooLarge: "file.index_too_large",
  fileIo: "file.io",
  fileSourceChanged: "file.source_changed",
  fileInvalidSourceHash: "file.invalid_source_hash",
  fileSaveFailed: "file.save_failed",
  searchInvalidQuery: "search.invalid_query",
  appStateUnavailable: "app_state.unavailable",
  providerInvalidId: "provider.invalid_id",
  providerKeyInvalid: "provider.key_invalid",
  providerVaultUnavailable: "provider.vault_unavailable",
  providerConfirmationRequired: "provider.confirmation_required",
  providerInvalidRequest: "provider.invalid_request",
  providerAuthenticationFailed: "provider.authentication_failed",
  providerRateLimited: "provider.rate_limited",
  providerTimeout: "provider.timeout",
  providerUnreachable: "provider.unreachable",
  providerModelUnavailable: "provider.model_unavailable",
  providerProtocol: "provider.protocol",
  providerResponseTooLarge: "provider.response_too_large",
  providerCancelled: "provider.cancelled",
  providerNotReady: "provider.not_ready",
  providerRunActive: "provider.run_active",
  providerRunNotFound: "provider.run_not_found",
  unknown: "runtime.unknown",
} as const;

export const APP_ERROR_MESSAGE_KEYS = {
  runtimeUnavailable: "errors.runtimeUnavailable",
  invalidRuntimeResponse: "errors.invalidRuntimeResponse",
  projectInvalidRoot: "errors.projectInvalidRoot",
  projectMetadataInvalid: "errors.projectMetadataInvalid",
  projectConflict: "errors.projectConflict",
  projectChanged: "errors.projectChanged",
  projectScaffoldChanged: "errors.projectScaffoldChanged",
  projectInvalidMetadata: "errors.projectInvalidMetadata",
  projectUnsupportedSchema: "errors.projectUnsupportedSchema",
  projectConfirmationRequired: "errors.projectConfirmationRequired",
  projectIo: "errors.projectIo",
  projectSessionNotFound: "errors.projectSessionNotFound",
  projectLockUnavailable: "errors.projectLockUnavailable",
  projectWriterRequired: "errors.projectWriterRequired",
  projectRecoveryRequired: "errors.projectRecoveryRequired",
  fileInvalidPath: "errors.fileInvalidPath",
  fileNotFound: "errors.fileNotFound",
  fileNotAllowed: "errors.fileNotAllowed",
  fileTooLarge: "errors.fileTooLarge",
  fileUnsupportedEncoding: "errors.fileUnsupportedEncoding",
  fileIndexTooLarge: "errors.fileIndexTooLarge",
  fileIo: "errors.fileIo",
  fileSourceChanged: "errors.fileSourceChanged",
  fileInvalidSourceHash: "errors.fileInvalidSourceHash",
  fileSaveFailed: "errors.fileSaveFailed",
  searchInvalidQuery: "errors.searchInvalidQuery",
  appStateUnavailable: "errors.appStateUnavailable",
  providerInvalidId: "errors.providerInvalidId",
  providerKeyInvalid: "errors.providerKeyInvalid",
  providerVaultUnavailable: "errors.providerVaultUnavailable",
  providerConfirmationRequired: "errors.providerConfirmationRequired",
  providerInvalidRequest: "errors.providerInvalidRequest",
  providerAuthenticationFailed: "errors.providerAuthenticationFailed",
  providerRateLimited: "errors.providerRateLimited",
  providerTimeout: "errors.providerTimeout",
  providerUnreachable: "errors.providerUnreachable",
  providerModelUnavailable: "errors.providerModelUnavailable",
  providerProtocol: "errors.providerProtocol",
  providerResponseTooLarge: "errors.providerResponseTooLarge",
  providerCancelled: "errors.providerCancelled",
  providerNotReady: "errors.providerNotReady",
  providerRunActive: "errors.providerRunActive",
  providerRunNotFound: "errors.providerRunNotFound",
  unknown: "errors.unknown",
} as const;

type ErrorCopy = (typeof ZH_CN.errors)[keyof typeof ZH_CN.errors];

export type PresentedAppError = ErrorCopy & {
  code: string;
  diagnostic?: string;
};

const SAFE_DIAGNOSTIC = /^[a-zA-Z0-9._-]{1,80}$/u;
const KNOWN_ERRORS: Record<string, { code: string; copy: ErrorCopy }> = {
  [APP_ERROR_MESSAGE_KEYS.runtimeUnavailable]: {
    code: APP_ERROR_CODES.runtimeUnavailable,
    copy: ZH_CN.errors.runtimeUnavailable,
  },
  [APP_ERROR_MESSAGE_KEYS.invalidRuntimeResponse]: {
    code: APP_ERROR_CODES.invalidRuntimeResponse,
    copy: ZH_CN.errors.invalidRuntimeResponse,
  },
  [APP_ERROR_MESSAGE_KEYS.projectInvalidRoot]: {
    code: APP_ERROR_CODES.projectInvalidRoot,
    copy: ZH_CN.errors.projectInvalidRoot,
  },
  [APP_ERROR_MESSAGE_KEYS.projectMetadataInvalid]: {
    code: APP_ERROR_CODES.projectMetadataInvalid,
    copy: ZH_CN.errors.projectMetadataInvalid,
  },
  [APP_ERROR_MESSAGE_KEYS.projectConflict]: {
    code: APP_ERROR_CODES.projectConflict,
    copy: ZH_CN.errors.projectConflict,
  },
  [APP_ERROR_MESSAGE_KEYS.projectChanged]: {
    code: APP_ERROR_CODES.projectChanged,
    copy: ZH_CN.errors.projectChanged,
  },
  [APP_ERROR_MESSAGE_KEYS.projectScaffoldChanged]: {
    code: APP_ERROR_CODES.projectScaffoldChanged,
    copy: ZH_CN.errors.projectScaffoldChanged,
  },
  [APP_ERROR_MESSAGE_KEYS.projectInvalidMetadata]: {
    code: APP_ERROR_CODES.projectInvalidMetadata,
    copy: ZH_CN.errors.projectInvalidMetadata,
  },
  [APP_ERROR_MESSAGE_KEYS.projectUnsupportedSchema]: {
    code: APP_ERROR_CODES.projectUnsupportedSchema,
    copy: ZH_CN.errors.projectUnsupportedSchema,
  },
  [APP_ERROR_MESSAGE_KEYS.projectConfirmationRequired]: {
    code: APP_ERROR_CODES.projectConfirmationRequired,
    copy: ZH_CN.errors.projectConfirmationRequired,
  },
  [APP_ERROR_MESSAGE_KEYS.projectIo]: {
    code: APP_ERROR_CODES.projectIo,
    copy: ZH_CN.errors.projectIo,
  },
  [APP_ERROR_MESSAGE_KEYS.projectSessionNotFound]: {
    code: APP_ERROR_CODES.projectSessionNotFound,
    copy: ZH_CN.errors.projectSessionNotFound,
  },
  [APP_ERROR_MESSAGE_KEYS.projectLockUnavailable]: {
    code: APP_ERROR_CODES.projectLockUnavailable,
    copy: ZH_CN.errors.projectLockUnavailable,
  },
  [APP_ERROR_MESSAGE_KEYS.projectWriterRequired]: {
    code: APP_ERROR_CODES.projectWriterRequired,
    copy: ZH_CN.errors.projectWriterRequired,
  },
  [APP_ERROR_MESSAGE_KEYS.projectRecoveryRequired]: {
    code: APP_ERROR_CODES.projectRecoveryRequired,
    copy: ZH_CN.errors.projectRecoveryRequired,
  },
  [APP_ERROR_MESSAGE_KEYS.fileInvalidPath]: {
    code: APP_ERROR_CODES.fileInvalidPath,
    copy: ZH_CN.errors.fileInvalidPath,
  },
  [APP_ERROR_MESSAGE_KEYS.fileNotFound]: {
    code: APP_ERROR_CODES.fileNotFound,
    copy: ZH_CN.errors.fileNotFound,
  },
  [APP_ERROR_MESSAGE_KEYS.fileNotAllowed]: {
    code: APP_ERROR_CODES.fileNotAllowed,
    copy: ZH_CN.errors.fileNotAllowed,
  },
  [APP_ERROR_MESSAGE_KEYS.fileTooLarge]: {
    code: APP_ERROR_CODES.fileTooLarge,
    copy: ZH_CN.errors.fileTooLarge,
  },
  [APP_ERROR_MESSAGE_KEYS.fileUnsupportedEncoding]: {
    code: APP_ERROR_CODES.fileUnsupportedEncoding,
    copy: ZH_CN.errors.fileUnsupportedEncoding,
  },
  [APP_ERROR_MESSAGE_KEYS.fileIndexTooLarge]: {
    code: APP_ERROR_CODES.fileIndexTooLarge,
    copy: ZH_CN.errors.fileIndexTooLarge,
  },
  [APP_ERROR_MESSAGE_KEYS.fileIo]: {
    code: APP_ERROR_CODES.fileIo,
    copy: ZH_CN.errors.fileIo,
  },
  [APP_ERROR_MESSAGE_KEYS.fileSourceChanged]: {
    code: APP_ERROR_CODES.fileSourceChanged,
    copy: ZH_CN.errors.fileSourceChanged,
  },
  [APP_ERROR_MESSAGE_KEYS.fileInvalidSourceHash]: {
    code: APP_ERROR_CODES.fileInvalidSourceHash,
    copy: ZH_CN.errors.fileInvalidSourceHash,
  },
  [APP_ERROR_MESSAGE_KEYS.fileSaveFailed]: {
    code: APP_ERROR_CODES.fileSaveFailed,
    copy: ZH_CN.errors.fileSaveFailed,
  },
  [APP_ERROR_MESSAGE_KEYS.searchInvalidQuery]: {
    code: APP_ERROR_CODES.searchInvalidQuery,
    copy: ZH_CN.errors.searchInvalidQuery,
  },
  [APP_ERROR_MESSAGE_KEYS.appStateUnavailable]: {
    code: APP_ERROR_CODES.appStateUnavailable,
    copy: ZH_CN.errors.appStateUnavailable,
  },
  [APP_ERROR_MESSAGE_KEYS.providerInvalidId]: {
    code: APP_ERROR_CODES.providerInvalidId,
    copy: ZH_CN.errors.providerInvalidId,
  },
  [APP_ERROR_MESSAGE_KEYS.providerKeyInvalid]: {
    code: APP_ERROR_CODES.providerKeyInvalid,
    copy: ZH_CN.errors.providerKeyInvalid,
  },
  [APP_ERROR_MESSAGE_KEYS.providerVaultUnavailable]: {
    code: APP_ERROR_CODES.providerVaultUnavailable,
    copy: ZH_CN.errors.providerVaultUnavailable,
  },
  [APP_ERROR_MESSAGE_KEYS.providerConfirmationRequired]: {
    code: APP_ERROR_CODES.providerConfirmationRequired,
    copy: ZH_CN.errors.providerConfirmationRequired,
  },
  [APP_ERROR_MESSAGE_KEYS.providerInvalidRequest]: {
    code: APP_ERROR_CODES.providerInvalidRequest,
    copy: ZH_CN.errors.providerInvalidRequest,
  },
  [APP_ERROR_MESSAGE_KEYS.providerAuthenticationFailed]: {
    code: APP_ERROR_CODES.providerAuthenticationFailed,
    copy: ZH_CN.errors.providerAuthenticationFailed,
  },
  [APP_ERROR_MESSAGE_KEYS.providerRateLimited]: {
    code: APP_ERROR_CODES.providerRateLimited,
    copy: ZH_CN.errors.providerRateLimited,
  },
  [APP_ERROR_MESSAGE_KEYS.providerTimeout]: {
    code: APP_ERROR_CODES.providerTimeout,
    copy: ZH_CN.errors.providerTimeout,
  },
  [APP_ERROR_MESSAGE_KEYS.providerUnreachable]: {
    code: APP_ERROR_CODES.providerUnreachable,
    copy: ZH_CN.errors.providerUnreachable,
  },
  [APP_ERROR_MESSAGE_KEYS.providerModelUnavailable]: {
    code: APP_ERROR_CODES.providerModelUnavailable,
    copy: ZH_CN.errors.providerModelUnavailable,
  },
  [APP_ERROR_MESSAGE_KEYS.providerProtocol]: {
    code: APP_ERROR_CODES.providerProtocol,
    copy: ZH_CN.errors.providerProtocol,
  },
  [APP_ERROR_MESSAGE_KEYS.providerResponseTooLarge]: {
    code: APP_ERROR_CODES.providerResponseTooLarge,
    copy: ZH_CN.errors.providerResponseTooLarge,
  },
  [APP_ERROR_MESSAGE_KEYS.providerCancelled]: {
    code: APP_ERROR_CODES.providerCancelled,
    copy: ZH_CN.errors.providerCancelled,
  },
  [APP_ERROR_MESSAGE_KEYS.providerNotReady]: {
    code: APP_ERROR_CODES.providerNotReady,
    copy: ZH_CN.errors.providerNotReady,
  },
  [APP_ERROR_MESSAGE_KEYS.providerRunActive]: {
    code: APP_ERROR_CODES.providerRunActive,
    copy: ZH_CN.errors.providerRunActive,
  },
  [APP_ERROR_MESSAGE_KEYS.providerRunNotFound]: {
    code: APP_ERROR_CODES.providerRunNotFound,
    copy: ZH_CN.errors.providerRunNotFound,
  },
  [APP_ERROR_MESSAGE_KEYS.unknown]: {
    code: APP_ERROR_CODES.unknown,
    copy: ZH_CN.errors.unknown,
  },
};

export function presentAppError(error: AppError): PresentedAppError {
  const known = KNOWN_ERRORS[error.messageKey];

  if (!known || known.code !== error.code) {
    return {
      code: APP_ERROR_CODES.unknown,
      ...ZH_CN.errors.unknown,
    };
  }

  return {
    code: known.code,
    ...known.copy,
    ...(error.diagnostic && SAFE_DIAGNOSTIC.test(error.diagnostic)
      ? { diagnostic: error.diagnostic }
      : {}),
  };
}
