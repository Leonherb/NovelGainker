pub mod agent_runtime;
pub mod agent_transaction;
pub mod agent_write;
pub mod app_state;
pub mod canon_apply;
pub mod coordination;
pub mod credential_vault;
pub mod project;
pub mod provider_core;
pub mod provider_state;
pub mod recovery;
pub mod search;
pub mod workspace;

use serde::{Deserialize, Serialize};

pub const HEALTH_CONTRACT_VERSION: u16 = novelgainker_domain::DOMAIN_CONTRACT_VERSION;
pub const APP_ERROR_RUNTIME_UNAVAILABLE_CODE: &str = "runtime.unavailable";
pub const APP_ERROR_INVALID_RESPONSE_CODE: &str = "contract.invalid_response";
pub const APP_ERROR_UNKNOWN_CODE: &str = "runtime.unknown";
pub const APP_ERROR_RUNTIME_UNAVAILABLE_KEY: &str = "errors.runtimeUnavailable";
pub const APP_ERROR_INVALID_RESPONSE_KEY: &str = "errors.invalidRuntimeResponse";
pub const APP_ERROR_UNKNOWN_KEY: &str = "errors.unknown";
pub const APP_ERROR_PROJECT_INVALID_ROOT_CODE: &str = "project.invalid_root";
pub const APP_ERROR_PROJECT_METADATA_INVALID_CODE: &str = "project.metadata_invalid";
pub const APP_ERROR_PROJECT_CONFLICT_CODE: &str = "project.conflict";
pub const APP_ERROR_PROJECT_CHANGED_CODE: &str = "project.changed";
pub const APP_ERROR_PROJECT_SCAFFOLD_CHANGED_CODE: &str = "project.scaffold_changed";
pub const APP_ERROR_PROJECT_INVALID_METADATA_CODE: &str = "project.invalid_metadata";
pub const APP_ERROR_PROJECT_UNSUPPORTED_SCHEMA_CODE: &str = "project.unsupported_schema";
pub const APP_ERROR_PROJECT_CONFIRMATION_REQUIRED_CODE: &str = "project.confirmation_required";
pub const APP_ERROR_PROJECT_IO_CODE: &str = "project.io";
pub const APP_ERROR_PROJECT_LOCK_UNAVAILABLE_CODE: &str = "project.lock_unavailable";
pub const APP_ERROR_PROJECT_WRITER_REQUIRED_CODE: &str = "project.writer_required";
pub const APP_ERROR_PROJECT_RECOVERY_REQUIRED_CODE: &str = "project.recovery_required";
pub const APP_ERROR_PROJECT_INVALID_ROOT_KEY: &str = "errors.projectInvalidRoot";
pub const APP_ERROR_PROJECT_METADATA_INVALID_KEY: &str = "errors.projectMetadataInvalid";
pub const APP_ERROR_PROJECT_CONFLICT_KEY: &str = "errors.projectConflict";
pub const APP_ERROR_PROJECT_CHANGED_KEY: &str = "errors.projectChanged";
pub const APP_ERROR_PROJECT_SCAFFOLD_CHANGED_KEY: &str = "errors.projectScaffoldChanged";
pub const APP_ERROR_PROJECT_INVALID_METADATA_KEY: &str = "errors.projectInvalidMetadata";
pub const APP_ERROR_PROJECT_UNSUPPORTED_SCHEMA_KEY: &str = "errors.projectUnsupportedSchema";
pub const APP_ERROR_PROJECT_CONFIRMATION_REQUIRED_KEY: &str = "errors.projectConfirmationRequired";
pub const APP_ERROR_PROJECT_IO_KEY: &str = "errors.projectIo";
pub const APP_ERROR_PROJECT_LOCK_UNAVAILABLE_KEY: &str = "errors.projectLockUnavailable";
pub const APP_ERROR_PROJECT_WRITER_REQUIRED_KEY: &str = "errors.projectWriterRequired";
pub const APP_ERROR_PROJECT_RECOVERY_REQUIRED_KEY: &str = "errors.projectRecoveryRequired";
pub const APP_ERROR_PROJECT_SESSION_NOT_FOUND_CODE: &str = "project.session_not_found";
pub const APP_ERROR_FILE_INVALID_PATH_CODE: &str = "file.invalid_path";
pub const APP_ERROR_FILE_NOT_FOUND_CODE: &str = "file.not_found";
pub const APP_ERROR_FILE_NOT_ALLOWED_CODE: &str = "file.not_allowed";
pub const APP_ERROR_FILE_TOO_LARGE_CODE: &str = "file.too_large";
pub const APP_ERROR_FILE_UNSUPPORTED_ENCODING_CODE: &str = "file.unsupported_encoding";
pub const APP_ERROR_FILE_INDEX_TOO_LARGE_CODE: &str = "file.index_too_large";
pub const APP_ERROR_FILE_IO_CODE: &str = "file.io";
pub const APP_ERROR_FILE_SOURCE_CHANGED_CODE: &str = "file.source_changed";
pub const APP_ERROR_FILE_INVALID_SOURCE_HASH_CODE: &str = "file.invalid_source_hash";
pub const APP_ERROR_FILE_SAVE_FAILED_CODE: &str = "file.save_failed";
pub const APP_ERROR_SEARCH_INVALID_QUERY_CODE: &str = "search.invalid_query";
pub const APP_ERROR_APP_STATE_UNAVAILABLE_CODE: &str = "app_state.unavailable";
pub const APP_ERROR_AGENT_CONTRACT_CODE: &str = "agent.contract";
pub const APP_ERROR_PROVIDER_INVALID_ID_CODE: &str = "provider.invalid_id";
pub const APP_ERROR_PROVIDER_KEY_INVALID_CODE: &str = "provider.key_invalid";
pub const APP_ERROR_PROVIDER_VAULT_UNAVAILABLE_CODE: &str = "provider.vault_unavailable";
pub const APP_ERROR_PROVIDER_CONFIRMATION_REQUIRED_CODE: &str = "provider.confirmation_required";
pub const APP_ERROR_PROVIDER_INVALID_REQUEST_CODE: &str = "provider.invalid_request";
pub const APP_ERROR_PROVIDER_AUTHENTICATION_FAILED_CODE: &str = "provider.authentication_failed";
pub const APP_ERROR_PROVIDER_RATE_LIMITED_CODE: &str = "provider.rate_limited";
pub const APP_ERROR_PROVIDER_TIMEOUT_CODE: &str = "provider.timeout";
pub const APP_ERROR_PROVIDER_UNREACHABLE_CODE: &str = "provider.unreachable";
pub const APP_ERROR_PROVIDER_MODEL_UNAVAILABLE_CODE: &str = "provider.model_unavailable";
pub const APP_ERROR_PROVIDER_PROTOCOL_CODE: &str = "provider.protocol";
pub const APP_ERROR_PROVIDER_RESPONSE_TOO_LARGE_CODE: &str = "provider.response_too_large";
pub const APP_ERROR_PROVIDER_CANCELLED_CODE: &str = "provider.cancelled";
pub const APP_ERROR_PROVIDER_NOT_READY_CODE: &str = "provider.not_ready";
pub const APP_ERROR_PROVIDER_RUN_ACTIVE_CODE: &str = "provider.run_active";
pub const APP_ERROR_PROVIDER_RUN_NOT_FOUND_CODE: &str = "provider.run_not_found";
pub const APP_ERROR_WORKFLOW_RUN_ACTIVE_CODE: &str = "workflow.run_active";
pub const APP_ERROR_WORKFLOW_INVALID_STATE_CODE: &str = "workflow.invalid_state";
pub const APP_ERROR_WORKFLOW_CONFIRMATION_REQUIRED_CODE: &str = "workflow.confirmation_required";
pub const APP_ERROR_WORKFLOW_UNAVAILABLE_CODE: &str = "workflow.unavailable";
pub const APP_ERROR_WORKFLOW_INPUT_INVALID_CODE: &str = "workflow.input_invalid";
pub const APP_ERROR_WORKFLOW_CONTEXT_CHANGED_CODE: &str = "workflow.context_changed";
pub const APP_ERROR_WORKFLOW_SOURCE_NOT_ALLOWED_CODE: &str = "workflow.source_not_allowed";
pub const APP_ERROR_WORKFLOW_SOURCE_LIMIT_EXCEEDED_CODE: &str = "workflow.source_limit_exceeded";
pub const APP_ERROR_WORKFLOW_OUTPUT_PENDING_CODE: &str = "workflow.output_pending";
pub const APP_ERROR_WORKFLOW_OUTPUT_INVALID_CODE: &str = "workflow.output_invalid";
pub const APP_ERROR_WORKFLOW_REPAIR_FAILED_CODE: &str = "workflow.repair_failed";
pub const APP_ERROR_WORKFLOW_PROPOSAL_EXPIRED_CODE: &str = "workflow.proposal_expired";
pub const APP_ERROR_WORKFLOW_TARGET_CONFLICT_CODE: &str = "workflow.target_conflict";
pub const APP_ERROR_WORKFLOW_WRITER_REQUIRED_CODE: &str = "workflow.writer_required";
pub const APP_ERROR_WORKFLOW_RECOVERY_REQUIRED_CODE: &str = "workflow.recovery_required";
pub const APP_ERROR_WORKFLOW_VALIDATION_FAILED_CODE: &str = "workflow.validation_failed";
pub const APP_ERROR_PROJECT_SESSION_NOT_FOUND_KEY: &str = "errors.projectSessionNotFound";
pub const APP_ERROR_FILE_INVALID_PATH_KEY: &str = "errors.fileInvalidPath";
pub const APP_ERROR_FILE_NOT_FOUND_KEY: &str = "errors.fileNotFound";
pub const APP_ERROR_FILE_NOT_ALLOWED_KEY: &str = "errors.fileNotAllowed";
pub const APP_ERROR_FILE_TOO_LARGE_KEY: &str = "errors.fileTooLarge";
pub const APP_ERROR_FILE_UNSUPPORTED_ENCODING_KEY: &str = "errors.fileUnsupportedEncoding";
pub const APP_ERROR_FILE_INDEX_TOO_LARGE_KEY: &str = "errors.fileIndexTooLarge";
pub const APP_ERROR_FILE_IO_KEY: &str = "errors.fileIo";
pub const APP_ERROR_FILE_SOURCE_CHANGED_KEY: &str = "errors.fileSourceChanged";
pub const APP_ERROR_FILE_INVALID_SOURCE_HASH_KEY: &str = "errors.fileInvalidSourceHash";
pub const APP_ERROR_FILE_SAVE_FAILED_KEY: &str = "errors.fileSaveFailed";
pub const APP_ERROR_SEARCH_INVALID_QUERY_KEY: &str = "errors.searchInvalidQuery";
pub const APP_ERROR_APP_STATE_UNAVAILABLE_KEY: &str = "errors.appStateUnavailable";
pub const APP_ERROR_AGENT_CONTRACT_KEY: &str = "errors.agentContract";
const REDACTED_DIAGNOSTIC: &str = "diagnostic-redacted";
pub const APP_ERROR_PROVIDER_INVALID_ID_KEY: &str = "errors.providerInvalidId";
pub const APP_ERROR_PROVIDER_KEY_INVALID_KEY: &str = "errors.providerKeyInvalid";
pub const APP_ERROR_PROVIDER_VAULT_UNAVAILABLE_KEY: &str = "errors.providerVaultUnavailable";
pub const APP_ERROR_PROVIDER_CONFIRMATION_REQUIRED_KEY: &str =
    "errors.providerConfirmationRequired";
pub const APP_ERROR_PROVIDER_INVALID_REQUEST_KEY: &str = "errors.providerInvalidRequest";
pub const APP_ERROR_PROVIDER_AUTHENTICATION_FAILED_KEY: &str =
    "errors.providerAuthenticationFailed";
pub const APP_ERROR_PROVIDER_RATE_LIMITED_KEY: &str = "errors.providerRateLimited";
pub const APP_ERROR_PROVIDER_TIMEOUT_KEY: &str = "errors.providerTimeout";
pub const APP_ERROR_PROVIDER_UNREACHABLE_KEY: &str = "errors.providerUnreachable";
pub const APP_ERROR_PROVIDER_MODEL_UNAVAILABLE_KEY: &str = "errors.providerModelUnavailable";
pub const APP_ERROR_PROVIDER_PROTOCOL_KEY: &str = "errors.providerProtocol";
pub const APP_ERROR_PROVIDER_RESPONSE_TOO_LARGE_KEY: &str = "errors.providerResponseTooLarge";
pub const APP_ERROR_PROVIDER_CANCELLED_KEY: &str = "errors.providerCancelled";
pub const APP_ERROR_PROVIDER_NOT_READY_KEY: &str = "errors.providerNotReady";
pub const APP_ERROR_PROVIDER_RUN_ACTIVE_KEY: &str = "errors.providerRunActive";
pub const APP_ERROR_PROVIDER_RUN_NOT_FOUND_KEY: &str = "errors.providerRunNotFound";
pub const APP_ERROR_WORKFLOW_RUN_ACTIVE_KEY: &str = "errors.workflowRunActive";
pub const APP_ERROR_WORKFLOW_INVALID_STATE_KEY: &str = "errors.workflowInvalidState";
pub const APP_ERROR_WORKFLOW_CONFIRMATION_REQUIRED_KEY: &str =
    "errors.workflowConfirmationRequired";
pub const APP_ERROR_WORKFLOW_UNAVAILABLE_KEY: &str = "errors.workflowUnavailable";
pub const APP_ERROR_WORKFLOW_INPUT_INVALID_KEY: &str = "errors.workflowInputInvalid";
pub const APP_ERROR_WORKFLOW_CONTEXT_CHANGED_KEY: &str = "errors.workflowContextChanged";
pub const APP_ERROR_WORKFLOW_OUTPUT_PENDING_KEY: &str = "errors.workflowOutputPending";
pub const APP_ERROR_WORKFLOW_SOURCE_NOT_ALLOWED_KEY: &str = "errors.workflowSourceNotAllowed";
pub const APP_ERROR_WORKFLOW_SOURCE_LIMIT_EXCEEDED_KEY: &str = "errors.workflowSourceLimitExceeded";
pub const APP_ERROR_WORKFLOW_OUTPUT_INVALID_KEY: &str = "errors.workflowOutputInvalid";
pub const APP_ERROR_WORKFLOW_REPAIR_FAILED_KEY: &str = "errors.workflowRepairFailed";

pub const APP_ERROR_WORKFLOW_PROPOSAL_EXPIRED_KEY: &str = "errors.workflowProposalExpired";
pub const APP_ERROR_WORKFLOW_TARGET_CONFLICT_KEY: &str = "errors.workflowTargetConflict";
pub const APP_ERROR_WORKFLOW_WRITER_REQUIRED_KEY: &str = "errors.workflowWriterRequired";
pub const APP_ERROR_WORKFLOW_RECOVERY_REQUIRED_KEY: &str = "errors.workflowRecoveryRequired";
pub const APP_ERROR_WORKFLOW_VALIDATION_FAILED_KEY: &str = "errors.workflowValidationFailed";
pub const fn application_name() -> &'static str {
    "NovelGainker"
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub application: String,
    pub version: String,
    pub platform: String,
    pub status: HealthState,
    pub contract_version: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthState {
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

impl AppError {
    fn new(code: &'static str, message_key: &'static str) -> Self {
        Self {
            code: code.to_owned(),
            message_key: message_key.to_owned(),
            diagnostic: None,
        }
    }

    pub fn runtime_unavailable() -> Self {
        Self::new(
            APP_ERROR_RUNTIME_UNAVAILABLE_CODE,
            APP_ERROR_RUNTIME_UNAVAILABLE_KEY,
        )
    }

    pub fn invalid_runtime_response() -> Self {
        Self::new(
            APP_ERROR_INVALID_RESPONSE_CODE,
            APP_ERROR_INVALID_RESPONSE_KEY,
        )
    }

    pub fn unknown() -> Self {
        Self::new(APP_ERROR_UNKNOWN_CODE, APP_ERROR_UNKNOWN_KEY)
    }

    pub fn project_metadata_invalid() -> Self {
        Self::new(
            APP_ERROR_PROJECT_METADATA_INVALID_CODE,
            APP_ERROR_PROJECT_METADATA_INVALID_KEY,
        )
    }

    pub fn project_invalid_root() -> Self {
        Self::new(
            APP_ERROR_PROJECT_INVALID_ROOT_CODE,
            APP_ERROR_PROJECT_INVALID_ROOT_KEY,
        )
    }

    pub fn project_conflict() -> Self {
        Self::new(
            APP_ERROR_PROJECT_CONFLICT_CODE,
            APP_ERROR_PROJECT_CONFLICT_KEY,
        )
    }

    pub fn project_candidate_changed() -> Self {
        Self::new(
            APP_ERROR_PROJECT_CHANGED_CODE,
            APP_ERROR_PROJECT_CHANGED_KEY,
        )
    }

    pub fn project_scaffold_changed() -> Self {
        Self::new(
            APP_ERROR_PROJECT_SCAFFOLD_CHANGED_CODE,
            APP_ERROR_PROJECT_SCAFFOLD_CHANGED_KEY,
        )
    }

    pub fn project_invalid_metadata() -> Self {
        Self::new(
            APP_ERROR_PROJECT_INVALID_METADATA_CODE,
            APP_ERROR_PROJECT_INVALID_METADATA_KEY,
        )
    }

    pub fn project_unsupported_schema() -> Self {
        Self::new(
            APP_ERROR_PROJECT_UNSUPPORTED_SCHEMA_CODE,
            APP_ERROR_PROJECT_UNSUPPORTED_SCHEMA_KEY,
        )
    }

    pub fn project_confirmation_required() -> Self {
        Self::new(
            APP_ERROR_PROJECT_CONFIRMATION_REQUIRED_CODE,
            APP_ERROR_PROJECT_CONFIRMATION_REQUIRED_KEY,
        )
    }

    pub fn project_io() -> Self {
        Self::new(APP_ERROR_PROJECT_IO_CODE, APP_ERROR_PROJECT_IO_KEY)
    }

    pub fn project_lock_unavailable() -> Self {
        Self::new(
            APP_ERROR_PROJECT_LOCK_UNAVAILABLE_CODE,
            APP_ERROR_PROJECT_LOCK_UNAVAILABLE_KEY,
        )
    }

    pub fn project_writer_required() -> Self {
        Self::new(
            APP_ERROR_PROJECT_WRITER_REQUIRED_CODE,
            APP_ERROR_PROJECT_WRITER_REQUIRED_KEY,
        )
    }

    pub fn project_recovery_required() -> Self {
        Self::new(
            APP_ERROR_PROJECT_RECOVERY_REQUIRED_CODE,
            APP_ERROR_PROJECT_RECOVERY_REQUIRED_KEY,
        )
    }

    pub fn project_session_not_found() -> Self {
        Self::new(
            APP_ERROR_PROJECT_SESSION_NOT_FOUND_CODE,
            APP_ERROR_PROJECT_SESSION_NOT_FOUND_KEY,
        )
    }

    pub fn file_invalid_path() -> Self {
        Self::new(
            APP_ERROR_FILE_INVALID_PATH_CODE,
            APP_ERROR_FILE_INVALID_PATH_KEY,
        )
    }

    pub fn file_not_found() -> Self {
        Self::new(APP_ERROR_FILE_NOT_FOUND_CODE, APP_ERROR_FILE_NOT_FOUND_KEY)
    }

    pub fn file_not_allowed() -> Self {
        Self::new(
            APP_ERROR_FILE_NOT_ALLOWED_CODE,
            APP_ERROR_FILE_NOT_ALLOWED_KEY,
        )
    }

    pub fn file_too_large() -> Self {
        Self::new(APP_ERROR_FILE_TOO_LARGE_CODE, APP_ERROR_FILE_TOO_LARGE_KEY)
    }

    pub fn file_unsupported_encoding() -> Self {
        Self::new(
            APP_ERROR_FILE_UNSUPPORTED_ENCODING_CODE,
            APP_ERROR_FILE_UNSUPPORTED_ENCODING_KEY,
        )
    }

    pub fn file_index_too_large() -> Self {
        Self::new(
            APP_ERROR_FILE_INDEX_TOO_LARGE_CODE,
            APP_ERROR_FILE_INDEX_TOO_LARGE_KEY,
        )
    }

    pub fn file_io() -> Self {
        Self::new(APP_ERROR_FILE_IO_CODE, APP_ERROR_FILE_IO_KEY)
    }

    pub fn file_source_changed() -> Self {
        Self::new(
            APP_ERROR_FILE_SOURCE_CHANGED_CODE,
            APP_ERROR_FILE_SOURCE_CHANGED_KEY,
        )
    }

    pub fn file_invalid_source_hash() -> Self {
        Self::new(
            APP_ERROR_FILE_INVALID_SOURCE_HASH_CODE,
            APP_ERROR_FILE_INVALID_SOURCE_HASH_KEY,
        )
    }

    pub fn file_save_failed() -> Self {
        Self::new(
            APP_ERROR_FILE_SAVE_FAILED_CODE,
            APP_ERROR_FILE_SAVE_FAILED_KEY,
        )
    }

    pub fn search_invalid_query() -> Self {
        Self::new(
            APP_ERROR_SEARCH_INVALID_QUERY_CODE,
            APP_ERROR_SEARCH_INVALID_QUERY_KEY,
        )
    }

    pub fn app_state_unavailable() -> Self {
        Self::new(
            APP_ERROR_APP_STATE_UNAVAILABLE_CODE,
            APP_ERROR_APP_STATE_UNAVAILABLE_KEY,
        )
    }

    pub fn agent_contract(code: &str) -> Self {
        let stable_code = if (code.starts_with("agent-") || code.starts_with("agent."))
            && !code.is_empty()
            && code.len() <= 96
            && code.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'-' | b'_')
            }) {
            code.to_owned()
        } else {
            APP_ERROR_AGENT_CONTRACT_CODE.to_owned()
        };
        Self {
            code: stable_code,
            message_key: APP_ERROR_AGENT_CONTRACT_KEY.to_owned(),
            diagnostic: None,
        }
    }

    pub fn provider_invalid_id() -> Self {
        Self::new(
            APP_ERROR_PROVIDER_INVALID_ID_CODE,
            APP_ERROR_PROVIDER_INVALID_ID_KEY,
        )
    }

    pub fn provider_key_invalid() -> Self {
        Self::new(
            APP_ERROR_PROVIDER_KEY_INVALID_CODE,
            APP_ERROR_PROVIDER_KEY_INVALID_KEY,
        )
    }

    pub fn provider_vault_unavailable() -> Self {
        Self::new(
            APP_ERROR_PROVIDER_VAULT_UNAVAILABLE_CODE,
            APP_ERROR_PROVIDER_VAULT_UNAVAILABLE_KEY,
        )
    }

    pub fn provider_confirmation_required() -> Self {
        Self::new(
            APP_ERROR_PROVIDER_CONFIRMATION_REQUIRED_CODE,
            APP_ERROR_PROVIDER_CONFIRMATION_REQUIRED_KEY,
        )
    }

    pub fn from_provider_error(error: provider_core::ProviderError) -> Self {
        use provider_core::ProviderErrorKind;
        match error.kind {
            ProviderErrorKind::InvalidRequest => Self::new(
                APP_ERROR_PROVIDER_INVALID_REQUEST_CODE,
                APP_ERROR_PROVIDER_INVALID_REQUEST_KEY,
            ),
            ProviderErrorKind::Authentication => Self::new(
                APP_ERROR_PROVIDER_AUTHENTICATION_FAILED_CODE,
                APP_ERROR_PROVIDER_AUTHENTICATION_FAILED_KEY,
            ),
            ProviderErrorKind::RateLimited => Self::new(
                APP_ERROR_PROVIDER_RATE_LIMITED_CODE,
                APP_ERROR_PROVIDER_RATE_LIMITED_KEY,
            ),
            ProviderErrorKind::Timeout => Self::new(
                APP_ERROR_PROVIDER_TIMEOUT_CODE,
                APP_ERROR_PROVIDER_TIMEOUT_KEY,
            ),
            ProviderErrorKind::Unreachable => Self::new(
                APP_ERROR_PROVIDER_UNREACHABLE_CODE,
                APP_ERROR_PROVIDER_UNREACHABLE_KEY,
            ),
            ProviderErrorKind::ModelUnavailable => Self::new(
                APP_ERROR_PROVIDER_MODEL_UNAVAILABLE_CODE,
                APP_ERROR_PROVIDER_MODEL_UNAVAILABLE_KEY,
            ),
            ProviderErrorKind::Protocol => Self::new(
                APP_ERROR_PROVIDER_PROTOCOL_CODE,
                APP_ERROR_PROVIDER_PROTOCOL_KEY,
            ),
            ProviderErrorKind::ResponseTooLarge => Self::new(
                APP_ERROR_PROVIDER_RESPONSE_TOO_LARGE_CODE,
                APP_ERROR_PROVIDER_RESPONSE_TOO_LARGE_KEY,
            ),
            ProviderErrorKind::Cancelled => Self::new(
                APP_ERROR_PROVIDER_CANCELLED_CODE,
                APP_ERROR_PROVIDER_CANCELLED_KEY,
            ),
        }
    }

    pub fn provider_not_ready() -> Self {
        Self::new(
            APP_ERROR_PROVIDER_NOT_READY_CODE,
            APP_ERROR_PROVIDER_NOT_READY_KEY,
        )
    }

    pub fn provider_run_active() -> Self {
        Self::new(
            APP_ERROR_PROVIDER_RUN_ACTIVE_CODE,
            APP_ERROR_PROVIDER_RUN_ACTIVE_KEY,
        )
    }

    pub fn provider_run_not_found() -> Self {
        Self::new(
            APP_ERROR_PROVIDER_RUN_NOT_FOUND_CODE,
            APP_ERROR_PROVIDER_RUN_NOT_FOUND_KEY,
        )
    }

    pub fn workflow_run_active() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_RUN_ACTIVE_CODE,
            APP_ERROR_WORKFLOW_RUN_ACTIVE_KEY,
        )
    }

    pub fn workflow_invalid_state() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_INVALID_STATE_CODE,
            APP_ERROR_WORKFLOW_INVALID_STATE_KEY,
        )
    }

    pub fn workflow_confirmation_required() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_CONFIRMATION_REQUIRED_CODE,
            APP_ERROR_WORKFLOW_CONFIRMATION_REQUIRED_KEY,
        )
    }

    pub fn workflow_unavailable() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_UNAVAILABLE_CODE,
            APP_ERROR_WORKFLOW_UNAVAILABLE_KEY,
        )
    }

    pub fn workflow_input_invalid() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_INPUT_INVALID_CODE,
            APP_ERROR_WORKFLOW_INPUT_INVALID_KEY,
        )
    }

    pub fn workflow_context_changed() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_CONTEXT_CHANGED_CODE,
            APP_ERROR_WORKFLOW_CONTEXT_CHANGED_KEY,
        )
    }

    pub fn workflow_source_not_allowed() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_SOURCE_NOT_ALLOWED_CODE,
            APP_ERROR_WORKFLOW_SOURCE_NOT_ALLOWED_KEY,
        )
    }
    pub fn workflow_source_limit_exceeded() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_SOURCE_LIMIT_EXCEEDED_CODE,
            APP_ERROR_WORKFLOW_SOURCE_LIMIT_EXCEEDED_KEY,
        )
    }

    pub fn workflow_output_pending() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_OUTPUT_PENDING_CODE,
            APP_ERROR_WORKFLOW_OUTPUT_PENDING_KEY,
        )
    }

    pub fn workflow_output_invalid() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_OUTPUT_INVALID_CODE,
            APP_ERROR_WORKFLOW_OUTPUT_INVALID_KEY,
        )
    }

    pub fn workflow_repair_failed() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_REPAIR_FAILED_CODE,
            APP_ERROR_WORKFLOW_REPAIR_FAILED_KEY,
        )
    }

    pub fn workflow_proposal_expired() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_PROPOSAL_EXPIRED_CODE,
            APP_ERROR_WORKFLOW_PROPOSAL_EXPIRED_KEY,
        )
    }

    pub fn workflow_target_conflict() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_TARGET_CONFLICT_CODE,
            APP_ERROR_WORKFLOW_TARGET_CONFLICT_KEY,
        )
    }

    pub fn workflow_writer_required() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_WRITER_REQUIRED_CODE,
            APP_ERROR_WORKFLOW_WRITER_REQUIRED_KEY,
        )
    }

    pub fn workflow_recovery_required() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_RECOVERY_REQUIRED_CODE,
            APP_ERROR_WORKFLOW_RECOVERY_REQUIRED_KEY,
        )
    }

    pub fn workflow_validation_failed() -> Self {
        Self::new(
            APP_ERROR_WORKFLOW_VALIDATION_FAILED_CODE,
            APP_ERROR_WORKFLOW_VALIDATION_FAILED_KEY,
        )
    }
    pub fn with_diagnostic(mut self, diagnostic: impl Into<String>) -> Self {
        let diagnostic = diagnostic.into();
        self.diagnostic = Some(if is_safe_diagnostic(&diagnostic) {
            diagnostic
        } else {
            REDACTED_DIAGNOSTIC.to_owned()
        });
        self
    }
}

fn is_safe_diagnostic(diagnostic: &str) -> bool {
    !diagnostic.is_empty()
        && diagnostic.len() <= 80
        && diagnostic
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub fn health_status(version: &str, platform: &str) -> HealthStatus {
    HealthStatus {
        application: application_name().to_owned(),
        version: version.to_owned(),
        platform: platform.to_owned(),
        status: HealthState::Ready,
        contract_version: HEALTH_CONTRACT_VERSION,
    }
}

#[cfg(test)]
mod tests {
    use super::{application_name, health_status, AppError, HealthState};
    use serde_json::{json, Value};

    #[test]
    fn exposes_application_name() {
        assert_eq!(application_name(), "NovelGainker");
    }

    #[test]
    fn health_contract_matches_the_shared_fixture() {
        let expected: Value =
            serde_json::from_str(include_str!("../../../contracts/ipc/health-status.v1.json"))
                .expect("health fixture must be valid JSON");
        let actual = serde_json::to_value(health_status("0.1.0", "windows"))
            .expect("health status must serialize");

        assert_eq!(actual, expected);
        assert_eq!(actual["status"], json!(HealthState::Ready));
    }

    #[test]
    fn health_contract_exposes_no_sensitive_or_machine_specific_fields() {
        let value = serde_json::to_value(health_status("0.1.0", "windows"))
            .expect("health status must serialize");
        let keys = value
            .as_object()
            .expect("health status must be an object")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();

        assert_eq!(
            keys,
            [
                "application",
                "contractVersion",
                "platform",
                "status",
                "version"
            ]
        );
    }

    #[test]
    fn app_error_contract_matches_the_shared_fixture() {
        let expected: Value =
            serde_json::from_str(include_str!("../../../contracts/ipc/app-error.v1.json"))
                .expect("error fixture must be valid JSON");
        let actual = serde_json::to_value(
            AppError::runtime_unavailable().with_diagnostic("tauri-runtime-unavailable"),
        )
        .expect("app error must serialize");

        assert_eq!(actual, expected);
    }

    #[test]
    fn app_error_omits_an_absent_diagnostic() {
        let actual = serde_json::to_value(AppError::invalid_runtime_response())
            .expect("app error must serialize");

        assert_eq!(
            actual,
            json!({
                "code": "contract.invalid_response",
                "messageKey": "errors.invalidRuntimeResponse"
            })
        );
    }

    #[test]
    fn app_error_redacts_paths_usernames_inputs_and_secrets() {
        let unsafe_diagnostic =
            r"C:\Users\Leon\secret-project\draft.md?prompt=private&api_key=sk-secret";
        let actual = serde_json::to_value(AppError::unknown().with_diagnostic(unsafe_diagnostic))
            .expect("app error must serialize");
        let serialized = actual.to_string();

        assert_eq!(actual["diagnostic"], json!("diagnostic-redacted"));
        for sensitive in ["Leon", "draft.md", "private", "sk-secret", "C:\\Users"] {
            assert!(!serialized.contains(sensitive));
        }
    }
}
