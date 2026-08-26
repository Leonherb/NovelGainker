use crate::AppError;

#[derive(Debug)]
pub struct ProjectWriteLease {
    #[cfg(windows)]
    handle: isize,
    #[cfg(not(windows))]
    key: String,
}

pub fn try_acquire_project_write_lease(
    coordination_key: &str,
) -> Result<Option<ProjectWriteLease>, AppError> {
    if coordination_key.len() != 64
        || !coordination_key
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(AppError::project_lock_unavailable());
    }
    acquire_platform_lease(coordination_key)
}

#[cfg(windows)]
fn acquire_platform_lease(key: &str) -> Result<Option<ProjectWriteLease>, AppError> {
    use std::ptr::null;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    let name = format!("Local\\NovelGainker.Project.{key}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let handle = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return Err(AppError::project_lock_unavailable()
            .with_diagnostic("project-writer-mutex-create-failed"));
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            CloseHandle(handle);
        }
        Ok(None)
    } else {
        Ok(Some(ProjectWriteLease {
            handle: handle as isize,
        }))
    }
}

#[cfg(windows)]
impl Drop for ProjectWriteLease {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;

        unsafe {
            CloseHandle(self.handle as *mut core::ffi::c_void);
        }
    }
}

#[cfg(not(windows))]
fn lease_registry() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};

    static LEASES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    LEASES.get_or_init(|| Mutex::new(HashSet::new()))
}

#[cfg(not(windows))]
fn acquire_platform_lease(key: &str) -> Result<Option<ProjectWriteLease>, AppError> {
    let mut leases = lease_registry()
        .lock()
        .map_err(|_| AppError::project_lock_unavailable())?;
    if leases.insert(key.to_owned()) {
        Ok(Some(ProjectWriteLease {
            key: key.to_owned(),
        }))
    } else {
        Ok(None)
    }
}

#[cfg(not(windows))]
impl Drop for ProjectWriteLease {
    fn drop(&mut self) {
        if let Ok(mut leases) = lease_registry().lock() {
            leases.remove(&self.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::try_acquire_project_write_lease;
    use uuid::Uuid;

    fn unique_key() -> String {
        use sha2::{Digest, Sha256};

        let digest = Sha256::digest(Uuid::new_v4().as_bytes());
        format!("{digest:x}")
    }

    #[test]
    fn ng_104_allows_only_one_same_process_writer_and_releases_on_drop() {
        let key = unique_key();
        let first = try_acquire_project_write_lease(&key)
            .expect("writer lookup must succeed")
            .expect("first writer must acquire");
        assert!(
            try_acquire_project_write_lease(&key)
                .expect("second writer lookup must succeed")
                .is_none(),
            "a second writer must become read-only"
        );
        drop(first);
        assert!(
            try_acquire_project_write_lease(&key)
                .expect("writer lookup after close must succeed")
                .is_some(),
            "closing the writer must release the OS-owned lease"
        );
    }

    #[cfg(windows)]
    #[test]
    fn ng_104_windows_mutex_coordinates_processes_and_survives_process_termination() {
        use std::process::Command;

        let key = unique_key();
        let first = try_acquire_project_write_lease(&key)
            .expect("writer lookup must succeed")
            .expect("parent writer must acquire");
        let probe = Command::new(std::env::current_exe().expect("test executable must resolve"))
            .args([
                "--ignored",
                "--exact",
                "coordination::tests::ng_104_cross_process_probe",
            ])
            .env("NOVELGAINKER_TEST_LOCK_KEY", &key)
            .env("NOVELGAINKER_TEST_LOCK_MODE", "readonly")
            .output()
            .expect("read-only probe must run");
        assert!(
            probe.status.success(),
            "second process must observe read-only: {}",
            String::from_utf8_lossy(&probe.stderr)
        );
        drop(first);

        let marker =
            std::env::temp_dir().join(format!("novelgainker-ng104-lock-marker-{}", Uuid::new_v4()));
        let terminated =
            Command::new(std::env::current_exe().expect("test executable must resolve"))
                .args([
                    "--ignored",
                    "--exact",
                    "coordination::tests::ng_104_cross_process_probe",
                ])
                .env("NOVELGAINKER_TEST_LOCK_KEY", &key)
                .env("NOVELGAINKER_TEST_LOCK_MODE", "abort")
                .env("NOVELGAINKER_TEST_LOCK_MARKER", &marker)
                .output()
                .expect("termination probe must run");
        assert!(
            !terminated.status.success(),
            "abort fixture must terminate abnormally"
        );
        assert_eq!(
            std::fs::read(&marker).expect("child acquisition marker must exist"),
            b"lease-acquired",
            "child must acquire before termination"
        );
        std::fs::remove_file(&marker).expect("verified child marker must remove");
        assert!(
            try_acquire_project_write_lease(&key)
                .expect("post-termination writer lookup must succeed")
                .is_some(),
            "the OS must release the mutex when its owning process terminates"
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "subprocess fixture invoked by the cross-process test"]
    fn ng_104_cross_process_probe() {
        use std::io::Write as _;

        let key = std::env::var("NOVELGAINKER_TEST_LOCK_KEY").expect("fixture key must exist");
        match std::env::var("NOVELGAINKER_TEST_LOCK_MODE").as_deref() {
            Ok("readonly") => assert!(
                try_acquire_project_write_lease(&key)
                    .expect("child lookup must succeed")
                    .is_none(),
                "second process must not acquire the writer mutex"
            ),
            Ok("abort") => {
                let _lease = try_acquire_project_write_lease(&key)
                    .expect("child lookup must succeed")
                    .expect("child must acquire after parent releases");
                let marker = std::path::PathBuf::from(
                    std::env::var_os("NOVELGAINKER_TEST_LOCK_MARKER")
                        .expect("fixture marker must exist"),
                );
                let mut marker_file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(marker)
                    .expect("fixture marker must create");
                marker_file
                    .write_all(b"lease-acquired")
                    .expect("fixture marker must write");
                marker_file.sync_all().expect("fixture marker must sync");
                std::process::abort();
            }
            _ => panic!("unknown cross-process fixture mode"),
        }
    }
}
