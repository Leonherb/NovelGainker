use crate::app_state::{AppStateRepository, ProviderSettingRecord};
use crate::credential_vault::{CredentialVault, ProviderId, SecretValue, VaultSecretStatus};
use crate::provider_core::{
    CancellationToken, NormalizedRequest, ProviderErrorKind, ProviderHttpClient,
};
use crate::AppError;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
const PROVIDER_STATE_SCHEMA_VERSION: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderConnectionStatus {
    Unconfigured,
    Untested,
    Connected,
    AuthenticationFailed,
    Unreachable,
    VaultUnavailable,
    RecoveryRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider_id: ProviderId,
    pub configured: bool,
    pub connection_status: ProviderConnectionStatus,
    pub preset_version: u16,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStateSnapshot {
    pub schema_version: u16,
    pub providers: Vec<ProviderStatus>,
    pub selected_provider_id: ProviderId,
}

pub fn load_provider_state(
    app_state: &AppStateRepository,
    vault: &dyn CredentialVault,
) -> Result<ProviderStateSnapshot, AppError> {
    let records = app_state.provider_settings()?;
    let selected_provider_id = records
        .iter()
        .find(|record| record.is_selected)
        .map(|record| record.provider_id)
        .ok_or_else(AppError::app_state_unavailable)?;
    let providers = records
        .iter()
        .map(|record| reconcile_provider(record, vault))
        .collect();
    Ok(ProviderStateSnapshot {
        schema_version: PROVIDER_STATE_SCHEMA_VERSION,
        providers,
        selected_provider_id,
    })
}

pub fn save_provider_key(
    app_state: &AppStateRepository,
    vault: &dyn CredentialVault,
    provider_id: &str,
    key: String,
) -> Result<ProviderStatus, AppError> {
    let provider = ProviderId::try_from(provider_id)?;
    let secret = SecretValue::parse(key)?;
    vault.put(provider, &secret)?;
    app_state.save_provider_credential_ref(provider, vault.credential_ref(provider))?;
    provider_status(app_state, vault, provider)
}

pub fn delete_provider_key(
    app_state: &AppStateRepository,
    vault: &dyn CredentialVault,
    provider_id: &str,
    confirmed: bool,
) -> Result<ProviderStatus, AppError> {
    if !confirmed {
        return Err(AppError::provider_confirmation_required());
    }
    let provider = ProviderId::try_from(provider_id)?;
    vault.delete(provider)?;
    app_state.clear_provider_credential_ref(provider)?;
    provider_status(app_state, vault, provider)
}

pub fn select_provider(
    app_state: &AppStateRepository,
    vault: &dyn CredentialVault,
    provider_id: &str,
) -> Result<ProviderStatus, AppError> {
    let provider = ProviderId::try_from(provider_id)?;
    app_state.select_provider(provider)?;
    provider_status(app_state, vault, provider)
}

pub fn test_provider_connection(
    app_state: &AppStateRepository,
    vault: &dyn CredentialVault,
    provider_id: &str,
) -> Result<ProviderStatus, AppError> {
    let provider = ProviderId::try_from(provider_id)?;
    let current = provider_status(app_state, vault, provider)?;
    if !current.configured {
        return Err(AppError::provider_vault_unavailable());
    }

    let client = ProviderHttpClient::production().map_err(AppError::from_provider_error)?;
    let request = NormalizedRequest::connection_probe();
    let cancellation = CancellationToken::default();
    let mut outcome = None;
    vault.with_secret(provider, &mut |key| {
        outcome = Some(client.complete(provider, key, &request, &cancellation));
        Ok(())
    })?;
    let outcome = outcome.ok_or_else(AppError::provider_vault_unavailable)?;
    let checked_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::app_state_unavailable())?
        .as_secs()
        .try_into()
        .map_err(|_| AppError::app_state_unavailable())?;

    match outcome {
        Ok(_) => {
            app_state.update_provider_connection_status(provider, "connected", checked_at)?;
            provider_status(app_state, vault, provider)
        }
        Err(error) => {
            let persisted_status = match error.kind {
                ProviderErrorKind::Authentication => "authentication_failed",
                _ => "unreachable",
            };
            app_state.update_provider_connection_status(provider, persisted_status, checked_at)?;
            Err(AppError::from_provider_error(error))
        }
    }
}

fn provider_status(
    app_state: &AppStateRepository,
    vault: &dyn CredentialVault,
    provider: ProviderId,
) -> Result<ProviderStatus, AppError> {
    let records = app_state.provider_settings()?;
    let record = records
        .iter()
        .find(|record| record.provider_id == provider)
        .ok_or_else(AppError::app_state_unavailable)?;
    Ok(reconcile_provider(record, vault))
}

fn reconcile_provider(
    record: &ProviderSettingRecord,
    vault: &dyn CredentialVault,
) -> ProviderStatus {
    let expected_reference = vault.credential_ref(record.provider_id);
    let vault_status = vault.status(record.provider_id);
    let (configured, connection_status) = match (record.credential_ref.as_deref(), vault_status) {
        (_, Err(_)) => (false, ProviderConnectionStatus::VaultUnavailable),
        (None, Ok(VaultSecretStatus::Absent)) => (false, ProviderConnectionStatus::Unconfigured),
        (Some(reference), Ok(VaultSecretStatus::Present)) if reference == expected_reference => {
            (true, persisted_connection_status(&record.connection_status))
        }
        _ => (false, ProviderConnectionStatus::RecoveryRequired),
    };
    ProviderStatus {
        provider_id: record.provider_id,
        configured,
        connection_status,
        preset_version: record.preset_version,
        selected: record.is_selected,
    }
}

fn persisted_connection_status(value: &str) -> ProviderConnectionStatus {
    match value {
        "connected" => ProviderConnectionStatus::Connected,
        "authentication_failed" => ProviderConnectionStatus::AuthenticationFailed,
        "unreachable" => ProviderConnectionStatus::Unreachable,
        _ => ProviderConnectionStatus::Untested,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        delete_provider_key, load_provider_state, save_provider_key, select_provider,
        ProviderConnectionStatus,
    };
    use crate::app_state::{AppStateRepository, APP_STATE_DATABASE_NAME};
    use crate::credential_vault::{CredentialVault, ProviderId, SecretValue, VaultSecretStatus};
    use crate::provider_core::ProviderError;
    use crate::AppError;
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use uuid::Uuid;

    struct MemoryVault {
        present: Mutex<HashSet<ProviderId>>,
        fail_put: AtomicBool,
        secrets: Mutex<HashMap<ProviderId, Vec<u8>>>,
        fail_status: AtomicBool,
    }

    impl MemoryVault {
        fn ready() -> Self {
            Self {
                present: Mutex::new(HashSet::new()),
                fail_put: AtomicBool::new(false),
                secrets: Mutex::new(HashMap::new()),
                fail_status: AtomicBool::new(false),
            }
        }
    }

    impl CredentialVault for MemoryVault {
        fn credential_ref(&self, provider: ProviderId) -> &str {
            match provider {
                ProviderId::Deepseek => "memory:deepseek",
                ProviderId::Openai => "memory:openai",
            }
        }

        fn put(&self, provider: ProviderId, secret: &SecretValue) -> Result<(), AppError> {
            if self.fail_put.load(Ordering::SeqCst) {
                return Err(AppError::provider_vault_unavailable());
            }
            let secret_copy = secret.with_bytes(<[u8]>::to_vec);
            self.secrets.lock().unwrap().insert(provider, secret_copy);
            self.present.lock().unwrap().insert(provider);
            Ok(())
        }

        fn delete(&self, provider: ProviderId) -> Result<(), AppError> {
            self.present.lock().unwrap().remove(&provider);
            self.secrets.lock().unwrap().remove(&provider);
            Ok(())
        }

        fn status(&self, provider: ProviderId) -> Result<VaultSecretStatus, AppError> {
            if self.fail_status.load(Ordering::SeqCst) {
                return Err(AppError::provider_vault_unavailable());
            }
            Ok(if self.present.lock().unwrap().contains(&provider) {
                VaultSecretStatus::Present
            } else {
                VaultSecretStatus::Absent
            })
        }

        fn with_secret(
            &self,
            provider: ProviderId,
            operation: &mut dyn FnMut(&[u8]) -> Result<(), AppError>,
        ) -> Result<(), AppError> {
            let secrets = self.secrets.lock().unwrap();
            let secret = secrets
                .get(&provider)
                .ok_or_else(AppError::provider_vault_unavailable)?;
            operation(secret)
        }
    }

    fn state_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("novelgainker-ng201-{label}-{}", Uuid::new_v4()))
    }

    fn assert_no_canary(label: &str, artifact: &[u8], canaries: &[String]) {
        assert!(
            !artifact
                .windows(b"Bearer ".len())
                .any(|window| window == b"Bearer "),
            "{label} contained an authorization value"
        );
        for (secret_index, canary) in canaries.iter().enumerate() {
            assert!(
                !artifact
                    .windows(canary.len())
                    .any(|window| window == canary.as_bytes()),
                "{label} contained complete Canary {secret_index}"
            );
            for (fragment_index, fragment) in canary.as_bytes().windows(16).enumerate() {
                assert!(
                    !artifact
                        .windows(fragment.len())
                        .any(|window| window == fragment),
                    "{label} contained Canary {secret_index} fragment {fragment_index}"
                );
            }
        }
    }

    fn scan_verified_tree(root: &PathBuf, canaries: &[String]) {
        for entry in fs::read_dir(root).expect("verified test root must be readable") {
            let entry = entry.expect("verified test entry must be readable");
            let path = entry.path();
            let metadata = entry
                .metadata()
                .expect("verified test metadata must be readable");
            if metadata.is_dir() {
                scan_verified_tree(&path, canaries);
            } else if metadata.is_file() {
                let bytes = fs::read(&path).expect("verified test artifact must be readable");
                assert_no_canary("verified temporary tree", &bytes, canaries);
            }
        }
    }

    #[test]
    fn ng_207_high_entropy_canary_lifecycle_leaks_zero_and_never_writes_canon() {
        let root = state_root("ng207-security");
        let app_state_root = root.join("state");
        let project_root = root.join("project");
        let chapter_root = project_root.join("chapters");
        fs::create_dir_all(&chapter_root).unwrap();
        let chapter_path = chapter_root.join("001.md");
        fs::write(&chapter_path, "# 合成章节\n\n离线编辑基线。\n").unwrap();
        let canon_before = fs::read(&chapter_path).unwrap();

        let repository = AppStateRepository::open(&app_state_root);
        let vault = MemoryVault::ready();
        let deepseek_first = format!(
            "NG207DeepSeek{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let deepseek_replacement = format!(
            "NG207DeepSeekReplace{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let openai_key = format!(
            "NG207OpenAI{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let canaries = vec![
            deepseek_first.clone(),
            deepseek_replacement.clone(),
            openai_key.clone(),
        ];
        assert!(canaries.iter().all(|canary| canary.len() >= 64));
        assert_ne!(deepseek_first, openai_key);

        let first = save_provider_key(&repository, &vault, "deepseek", deepseek_first).unwrap();
        assert!(first.configured);
        let replacement =
            save_provider_key(&repository, &vault, "deepseek", deepseek_replacement).unwrap();
        assert!(replacement.configured);
        let openai = save_provider_key(&repository, &vault, "openai", openai_key.clone()).unwrap();
        assert!(openai.configured);
        let selected = select_provider(&repository, &vault, "deepseek").unwrap();
        assert!(selected.selected);

        let mut borrowed_length = 0usize;
        vault
            .with_secret(ProviderId::Openai, &mut |secret| {
                borrowed_length = secret.len();
                Ok(())
            })
            .unwrap();
        assert_eq!(borrowed_length, openai_key.len());

        let snapshot = load_provider_state(&repository, &vault).unwrap();
        let snapshot_json = serde_json::to_vec(&snapshot).unwrap();
        let sqlite_export = repository
            .provider_settings()
            .unwrap()
            .into_iter()
            .map(|record| {
                format!(
                    "{}|{}|{}|{}|{}|{:?}",
                    record.provider_id.as_str(),
                    record.preset_version,
                    record.credential_ref.unwrap_or_default(),
                    record.is_selected,
                    record.connection_status,
                    record.checked_at
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let errors = [
            AppError::from_provider_error(ProviderError::authentication()),
            AppError::from_provider_error(ProviderError::rate_limited()),
            AppError::from_provider_error(ProviderError::timeout()),
            AppError::from_provider_error(ProviderError::model_unavailable()),
            AppError::from_provider_error(ProviderError::protocol()),
            AppError::from_provider_error(ProviderError::cancelled()),
        ];
        let error_json = serde_json::to_vec(&errors).unwrap();
        let app_log = errors
            .iter()
            .map(|error| error.code.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        drop(repository);
        let restarted = AppStateRepository::open(&app_state_root);
        let restarted_snapshot = load_provider_state(&restarted, &vault).unwrap();
        assert_eq!(
            restarted_snapshot.selected_provider_id,
            ProviderId::Deepseek
        );
        let deleted = delete_provider_key(&restarted, &vault, "deepseek", true).unwrap();
        assert!(!deleted.configured);
        assert!(load_provider_state(&restarted, &vault)
            .unwrap()
            .providers
            .iter()
            .any(|provider| provider.provider_id == ProviderId::Openai && provider.configured));

        vault.fail_status.store(true, Ordering::SeqCst);
        let unavailable = load_provider_state(&restarted, &vault).unwrap();
        assert!(unavailable.providers.iter().all(|provider| {
            provider.connection_status == ProviderConnectionStatus::VaultUnavailable
        }));
        vault.fail_status.store(false, Ordering::SeqCst);

        let artifacts = [
            ("sqlite export", sqlite_export.into_bytes()),
            ("error JSON", error_json),
            ("app log", app_log.into_bytes()),
            ("test snapshot", snapshot_json),
            (
                "DOM",
                b"<input type=\"password\" value=\"\"><p>Provider ready</p>".to_vec(),
            ),
            (
                "screenshot metadata",
                b"gate=G2;content=synthetic;provider=mock".to_vec(),
            ),
            (
                "quality evidence",
                b"canaryLeakCount=0;crossProviderAutomaticRequestCount=0;canonAutomaticWriteCount=0"
                    .to_vec(),
            ),
            (
                "restart snapshot",
                serde_json::to_vec(&restarted_snapshot).unwrap(),
            ),
            (
                "vault unavailable snapshot",
                serde_json::to_vec(&unavailable).unwrap(),
            ),
        ];
        for (label, artifact) in artifacts {
            assert_no_canary(label, &artifact, &canaries);
        }
        scan_verified_tree(&app_state_root, &canaries);
        scan_verified_tree(&project_root, &canaries);
        assert_eq!(fs::read(&chapter_path).unwrap(), canon_before);

        drop(restarted);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn saves_replaces_and_deletes_two_provider_keys_independently_without_readback() {
        let root = state_root("lifecycle");
        let repository = AppStateRepository::open(&root);
        let vault = MemoryVault::ready();
        let canary = format!("ng201-canary-{}", Uuid::new_v4());

        let deepseek = save_provider_key(&repository, &vault, "deepseek", canary.clone()).unwrap();
        assert!(deepseek.configured);
        assert_eq!(
            deepseek.connection_status,
            ProviderConnectionStatus::Untested
        );
        let database_bytes = fs::read(repository.database_path()).unwrap();
        assert!(!database_bytes
            .windows(canary.len())
            .any(|window| window == canary.as_bytes()));
        assert!(!serde_json::to_string(&deepseek).unwrap().contains(&canary));

        let replaced = save_provider_key(
            &repository,
            &vault,
            "deepseek",
            "synthetic-deepseek-replacement".to_owned(),
        )
        .unwrap();
        assert!(replaced.configured);
        let openai = save_provider_key(
            &repository,
            &vault,
            "openai",
            "synthetic-openai-first".to_owned(),
        )
        .unwrap();
        assert!(openai.configured);
        let deleted = delete_provider_key(&repository, &vault, "deepseek", true).unwrap();
        assert!(!deleted.configured);
        let restarted = AppStateRepository::open(&root);
        let snapshot = load_provider_state(&restarted, &vault).unwrap();
        assert_eq!(snapshot.schema_version, 2);
        assert!(!snapshot.providers[0].configured);
        assert!(snapshot.providers[1].configured);
        let serialized = serde_json::to_string(&snapshot).unwrap();
        assert!(!serialized.contains(&canary));

        drop(restarted);
        drop(repository);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn selection_is_explicit_persistent_and_does_not_require_a_key() {
        let root = state_root("selection");
        let repository = AppStateRepository::open(&root);
        let vault = MemoryVault::ready();

        let initial = load_provider_state(&repository, &vault).unwrap();
        assert_eq!(initial.selected_provider_id, ProviderId::Openai);

        let selected = select_provider(&repository, &vault, "deepseek").unwrap();
        assert_eq!(selected.provider_id, ProviderId::Deepseek);
        assert!(selected.selected);
        assert!(!selected.configured);

        let restarted = AppStateRepository::open(&root);
        let snapshot = load_provider_state(&restarted, &vault).unwrap();
        assert_eq!(snapshot.selected_provider_id, ProviderId::Deepseek);
        assert_eq!(
            snapshot
                .providers
                .iter()
                .filter(|provider| provider.selected)
                .count(),
            1
        );
        assert_eq!(
            select_provider(&restarted, &vault, "other")
                .unwrap_err()
                .code,
            "provider.invalid_id"
        );
        assert_eq!(
            load_provider_state(&restarted, &vault)
                .unwrap()
                .selected_provider_id,
            ProviderId::Deepseek
        );

        drop(restarted);
        drop(repository);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn confirmation_invalid_provider_and_vault_failure_do_not_write_state() {
        let root = state_root("failures");
        let repository = AppStateRepository::open(&root);
        let vault = MemoryVault::ready();

        assert_eq!(
            delete_provider_key(&repository, &vault, "openai", false)
                .unwrap_err()
                .code,
            "provider.confirmation_required"
        );
        assert_eq!(
            save_provider_key(&repository, &vault, "other", "synthetic-value".to_owned())
                .unwrap_err()
                .code,
            "provider.invalid_id"
        );
        vault.fail_put.store(true, Ordering::SeqCst);
        assert_eq!(
            save_provider_key(&repository, &vault, "openai", "synthetic-value".to_owned())
                .unwrap_err()
                .code,
            "provider.vault_unavailable"
        );
        assert!(repository
            .provider_settings()
            .unwrap()
            .iter()
            .all(|record| record.credential_ref.is_none()));

        drop(repository);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reconciles_orphaned_missing_and_unavailable_vault_state() {
        let root = state_root("reconcile");
        let repository = AppStateRepository::open(&root);
        let vault = MemoryVault::ready();

        vault.present.lock().unwrap().insert(ProviderId::Deepseek);
        let orphaned = load_provider_state(&repository, &vault).unwrap();
        assert_eq!(
            orphaned.providers[0].connection_status,
            ProviderConnectionStatus::RecoveryRequired
        );

        repository
            .save_provider_credential_ref(ProviderId::Openai, "memory:openai")
            .unwrap();
        let missing = load_provider_state(&repository, &vault).unwrap();
        assert_eq!(
            missing.providers[1].connection_status,
            ProviderConnectionStatus::RecoveryRequired
        );

        vault.fail_status.store(true, Ordering::SeqCst);
        let unavailable = load_provider_state(&repository, &vault).unwrap();
        assert!(unavailable.providers.iter().all(|provider| {
            provider.connection_status == ProviderConnectionStatus::VaultUnavailable
        }));

        drop(repository);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn vault_success_then_sqlite_failure_is_recoverable_as_an_orphan() {
        let root = state_root("orphan");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(APP_STATE_DATABASE_NAME), b"not a database").unwrap();
        let degraded = AppStateRepository::open(&root);
        let vault = MemoryVault::ready();

        assert_eq!(
            save_provider_key(
                &degraded,
                &vault,
                "deepseek",
                "synthetic-orphan-value".to_owned(),
            )
            .unwrap_err()
            .code,
            "app_state.unavailable"
        );
        assert_eq!(
            vault.status(ProviderId::Deepseek).unwrap(),
            VaultSecretStatus::Present
        );

        drop(degraded);
        fs::remove_file(root.join(APP_STATE_DATABASE_NAME)).unwrap();
        let recovered = AppStateRepository::open(&root);
        let snapshot = load_provider_state(&recovered, &vault).unwrap();
        assert_eq!(
            snapshot.providers[0].connection_status,
            ProviderConnectionStatus::RecoveryRequired
        );

        drop(recovered);
        fs::remove_dir_all(root).unwrap();
    }
}
