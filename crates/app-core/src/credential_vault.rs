use crate::AppError;
#[cfg(windows)]
use crate::APP_ERROR_PROVIDER_VAULT_UNAVAILABLE_CODE;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{compiler_fence, Ordering};

pub const MAX_CREDENTIAL_BLOB_BYTES: usize = 2_560;
#[cfg(windows)]
const DEEPSEEK_TARGET: &str = "NovelGainker/provider/deepseek/v1";
#[cfg(windows)]
const OPENAI_TARGET: &str = "NovelGainker/provider/openai/v1";
const DEEPSEEK_REFERENCE: &str = "wincred:NovelGainker/provider/deepseek/v1";
const OPENAI_REFERENCE: &str = "wincred:NovelGainker/provider/openai/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Deepseek,
    Openai,
}

impl ProviderId {
    pub const ALL: [Self; 2] = [Self::Deepseek, Self::Openai];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deepseek => "deepseek",
            Self::Openai => "openai",
        }
    }

    #[cfg(windows)]
    const fn index(self) -> usize {
        match self {
            Self::Deepseek => 0,
            Self::Openai => 1,
        }
    }
}

impl TryFrom<&str> for ProviderId {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "deepseek" => Ok(Self::Deepseek),
            "openai" => Ok(Self::Openai),
            _ => Err(AppError::provider_invalid_id()),
        }
    }
}

pub struct SecretValue {
    bytes: Vec<u8>,
}

impl SecretValue {
    pub fn parse(value: String) -> Result<Self, AppError> {
        let mut bytes = value.into_bytes();
        let valid = !bytes.is_empty()
            && bytes.len() <= MAX_CREDENTIAL_BLOB_BYTES
            && bytes.is_ascii()
            && std::str::from_utf8(&bytes).is_ok_and(|text| {
                text.chars()
                    .all(|character| !character.is_control() && !character.is_whitespace())
            });
        if !valid {
            zero_bytes(&mut bytes);
            return Err(AppError::provider_key_invalid());
        }
        Ok(Self { bytes })
    }

    pub fn with_bytes<T>(&self, operation: impl FnOnce(&[u8]) -> T) -> T {
        operation(&self.bytes)
    }
}

impl Drop for SecretValue {
    fn drop(&mut self) {
        zero_bytes(&mut self.bytes);
    }
}

fn zero_bytes(bytes: &mut [u8]) {
    for byte in bytes {
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultSecretStatus {
    Present,
    Absent,
}

pub trait CredentialVault: Send + Sync {
    fn credential_ref(&self, provider: ProviderId) -> &str;
    fn put(&self, provider: ProviderId, secret: &SecretValue) -> Result<(), AppError>;
    fn delete(&self, provider: ProviderId) -> Result<(), AppError>;
    fn status(&self, provider: ProviderId) -> Result<VaultSecretStatus, AppError>;
    fn with_secret(
        &self,
        provider: ProviderId,
        operation: &mut dyn FnMut(&[u8]) -> Result<(), AppError>,
    ) -> Result<(), AppError>;
}

#[cfg(windows)]
pub struct WindowsCredentialVault {
    targets: [String; 2],
    references: [String; 2],
}

#[cfg(windows)]
impl WindowsCredentialVault {
    pub fn production() -> Self {
        Self {
            targets: [DEEPSEEK_TARGET.to_owned(), OPENAI_TARGET.to_owned()],
            references: [DEEPSEEK_REFERENCE.to_owned(), OPENAI_REFERENCE.to_owned()],
        }
    }

    fn target(&self, provider: ProviderId) -> &str {
        &self.targets[provider.index()]
    }

    fn account(provider: ProviderId) -> &'static str {
        match provider {
            ProviderId::Deepseek => "NovelGainker.DeepSeek",
            ProviderId::Openai => "NovelGainker.OpenAI",
        }
    }

    fn read(&self, provider: ProviderId) -> Result<Option<CredentialAllocation>, AppError> {
        use std::ptr::null_mut;
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
        use windows_sys::Win32::Security::Credentials::{
            CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
        };

        let target = wide_null(self.target(provider));
        let mut raw: *mut CREDENTIALW = null_mut();
        let read = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut raw) };
        if read == 0 {
            let code = unsafe { GetLastError() };
            if code == ERROR_NOT_FOUND {
                return Ok(None);
            }
            return Err(vault_error("read", code));
        }
        if raw.is_null() {
            return Err(AppError::provider_vault_unavailable().with_diagnostic("wincred-read-null"));
        }
        Ok(Some(CredentialAllocation { raw }))
    }
}

#[cfg(windows)]
impl Default for WindowsCredentialVault {
    fn default() -> Self {
        Self::production()
    }
}

#[cfg(windows)]
impl CredentialVault for WindowsCredentialVault {
    fn credential_ref(&self, provider: ProviderId) -> &str {
        &self.references[provider.index()]
    }

    fn put(&self, provider: ProviderId, secret: &SecretValue) -> Result<(), AppError> {
        use windows_sys::Win32::Foundation::GetLastError;
        use windows_sys::Win32::Security::Credentials::{
            CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
        };

        let mut target = wide_null(self.target(provider));
        let mut account = wide_null(Self::account(provider));
        secret.with_bytes(|bytes| {
            let credential = CREDENTIALW {
                Type: CRED_TYPE_GENERIC,
                TargetName: target.as_mut_ptr(),
                CredentialBlobSize: bytes.len() as u32,
                CredentialBlob: bytes.as_ptr().cast_mut(),
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                UserName: account.as_mut_ptr(),
                ..Default::default()
            };
            let written = unsafe { CredWriteW(&credential, 0) };
            if written == 0 {
                return Err(vault_error("write", unsafe { GetLastError() }));
            }
            Ok(())
        })
    }

    fn delete(&self, provider: ProviderId) -> Result<(), AppError> {
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
        use windows_sys::Win32::Security::Credentials::{CredDeleteW, CRED_TYPE_GENERIC};

        let target = wide_null(self.target(provider));
        let deleted = unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if deleted != 0 {
            return Ok(());
        }
        let code = unsafe { GetLastError() };
        if code == ERROR_NOT_FOUND {
            return Ok(());
        }
        Err(vault_error("delete", code))
    }

    fn status(&self, provider: ProviderId) -> Result<VaultSecretStatus, AppError> {
        Ok(if self.read(provider)?.is_some() {
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
        let allocation = self.read(provider)?.ok_or_else(|| {
            AppError::provider_vault_unavailable().with_diagnostic("wincred-secret-absent")
        })?;
        operation(allocation.secret())
    }
}

#[cfg(windows)]
struct CredentialAllocation {
    raw: *mut windows_sys::Win32::Security::Credentials::CREDENTIALW,
}

#[cfg(windows)]
impl CredentialAllocation {
    fn secret(&self) -> &[u8] {
        let credential = unsafe { &*self.raw };
        if credential.CredentialBlob.is_null() || credential.CredentialBlobSize == 0 {
            return &[];
        }
        unsafe {
            std::slice::from_raw_parts(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            )
        }
    }
}

#[cfg(windows)]
impl Drop for CredentialAllocation {
    fn drop(&mut self) {
        use windows_sys::Win32::Security::Credentials::CredFree;
        let credential = unsafe { &mut *self.raw };
        if !credential.CredentialBlob.is_null() && credential.CredentialBlobSize > 0 {
            let secret = unsafe {
                std::slice::from_raw_parts_mut(
                    credential.CredentialBlob,
                    credential.CredentialBlobSize as usize,
                )
            };
            zero_bytes(secret);
        }
        unsafe { CredFree(self.raw.cast()) };
    }
}

#[cfg(windows)]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn vault_error(operation: &str, code: u32) -> AppError {
    debug_assert_eq!(
        AppError::provider_vault_unavailable().code,
        APP_ERROR_PROVIDER_VAULT_UNAVAILABLE_CODE
    );
    AppError::provider_vault_unavailable().with_diagnostic(format!("wincred-{operation}-{code}"))
}

#[cfg(not(windows))]
pub struct WindowsCredentialVault;

#[cfg(not(windows))]
impl WindowsCredentialVault {
    pub fn production() -> Self {
        Self
    }
}

#[cfg(not(windows))]
impl CredentialVault for WindowsCredentialVault {
    fn credential_ref(&self, provider: ProviderId) -> &str {
        match provider {
            ProviderId::Deepseek => DEEPSEEK_REFERENCE,
            ProviderId::Openai => OPENAI_REFERENCE,
        }
    }

    fn put(&self, _: ProviderId, _: &SecretValue) -> Result<(), AppError> {
        Err(AppError::provider_vault_unavailable())
    }

    fn delete(&self, _: ProviderId) -> Result<(), AppError> {
        Err(AppError::provider_vault_unavailable())
    }

    fn status(&self, _: ProviderId) -> Result<VaultSecretStatus, AppError> {
        Err(AppError::provider_vault_unavailable())
    }

    fn with_secret(
        &self,
        _: ProviderId,
        _: &mut dyn FnMut(&[u8]) -> Result<(), AppError>,
    ) -> Result<(), AppError> {
        Err(AppError::provider_vault_unavailable())
    }
}

#[cfg(all(test, windows))]
impl WindowsCredentialVault {
    fn isolated(namespace: &str) -> Self {
        assert!(
            !namespace.is_empty()
                && namespace
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        );
        let deepseek = format!("NovelGainker.test/{namespace}/deepseek/v1");
        let openai = format!("NovelGainker.test/{namespace}/openai/v1");
        Self {
            references: [format!("wincred:{deepseek}"), format!("wincred:{openai}")],
            targets: [deepseek, openai],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CredentialVault, ProviderId, SecretValue, VaultSecretStatus};

    #[test]
    fn secret_validation_rejects_empty_whitespace_control_and_oversize_values() {
        for invalid in [
            String::new(),
            "has space".to_owned(),
            "line\nbreak".to_owned(),
        ] {
            assert_eq!(
                SecretValue::parse(invalid).err().unwrap().code,
                "provider.key_invalid"
            );
        }
        assert_eq!(
            SecretValue::parse("x".repeat(super::MAX_CREDENTIAL_BLOB_BYTES + 1))
                .err()
                .unwrap()
                .code,
            "provider.key_invalid"
        );
        assert!(SecretValue::parse("synthetic-ng201-value".to_owned()).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn windows_credential_manager_isolated_round_trip_replace_and_delete() {
        use super::WindowsCredentialVault;
        use uuid::Uuid;

        let namespace = format!("ng201-{}", Uuid::new_v4());
        let vault = WindowsCredentialVault::isolated(&namespace);
        let provider = ProviderId::Deepseek;
        vault.delete(provider).expect("isolated target must clean");
        assert_eq!(vault.status(provider).unwrap(), VaultSecretStatus::Absent);

        let first = SecretValue::parse(format!("synthetic-first-{namespace}")).unwrap();
        vault.put(provider, &first).expect("credential must save");
        let mut observed = Vec::new();
        vault
            .with_secret(provider, &mut |bytes| {
                observed.extend_from_slice(bytes);
                Ok(())
            })
            .expect("credential must read inside Rust scope");
        first.with_bytes(|bytes| assert_eq!(observed, bytes));

        let replacement = SecretValue::parse(format!("synthetic-second-{namespace}")).unwrap();
        vault
            .put(provider, &replacement)
            .expect("credential must replace");
        observed.clear();
        vault
            .with_secret(provider, &mut |bytes| {
                observed.extend_from_slice(bytes);
                Ok(())
            })
            .expect("replacement must read inside Rust scope");
        replacement.with_bytes(|bytes| assert_eq!(observed, bytes));

        vault.delete(provider).expect("credential must delete");
        assert_eq!(vault.status(provider).unwrap(), VaultSecretStatus::Absent);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires explicit G2 product-owner approval and both fixed Vault credentials"]
    fn ng_207_live_provider_smoke_reads_only_fixed_windows_vault_targets() {
        use super::WindowsCredentialVault;
        use crate::provider_core::{
            preset, CancellationToken, NormalizedRequest, ProviderHttpClient,
        };
        use std::time::{SystemTime, UNIX_EPOCH};

        let vault = WindowsCredentialVault::production();
        let client = ProviderHttpClient::production()
            .unwrap_or_else(|error| panic!("G2 live client failed: {:?}", error.kind));
        let request = NormalizedRequest::connection_probe();

        for provider in ProviderId::ALL {
            let cancellation = CancellationToken::default();
            let mut outcome = None;
            let borrowed = vault.with_secret(provider, &mut |secret| {
                outcome = Some(client.complete(provider, secret, &request, &cancellation));
                Ok(())
            });
            if let Err(error) = borrowed {
                panic!(
                    "G2 live Vault borrow failed for {} with stable code {}",
                    provider.as_str(),
                    error.code
                );
            }
            let response = match outcome.expect("Vault operation must set a redacted outcome") {
                Ok(response) => response,
                Err(error) => {
                    panic!(
                        "G2 live Provider failed for {} with stable kind {:?}",
                        provider.as_str(),
                        error.kind
                    )
                }
            };
            assert_eq!(response.provider_id, provider);
            assert_eq!(response.model, preset(provider).model);
            let checked_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock must follow Unix epoch")
                .as_secs();
            println!(
                "[g2-live] provider={} presetVersion={} model={} result=passed checkedAt={}",
                provider.as_str(),
                preset(provider).preset_version,
                preset(provider).model,
                checked_at
            );
        }
    }
}
