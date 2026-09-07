//! Locate the existing project window without exporting a path or granting a
//! second writer. The property exists only while that window owns the project.
use super::ProjectWriteLease;
use crate::AppError;

pub struct ProjectWindowRegistration {
    #[cfg(windows)]
    hwnd: isize,
    #[cfg(windows)]
    property: Vec<u16>,
}

fn property_name(key: &str) -> Option<Vec<u16>> {
    (key.len() == 64
        && key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    .then(|| {
        format!("NovelGainker.ProjectWindow.{key}")
            .encode_utf16()
            .chain(Some(0))
            .collect()
    })
}

impl ProjectWindowRegistration {
    /// hwnd comes only from the desktop Host, never from IPC/model arguments.
    #[cfg(windows)]
    pub fn attach(lease: &ProjectWriteLease, hwnd: isize) -> Result<Self, AppError> {
        use windows_sys::Win32::System::Threading::GetCurrentProcessId;
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, SetPropW};
        let property = property_name(&lease.coordination_key)
            .ok_or_else(AppError::project_lock_unavailable)?;
        let mut owner = 0;
        // Validate ownership before changing the window's property list.
        unsafe {
            GetWindowThreadProcessId(hwnd as _, &mut owner);
            if owner != GetCurrentProcessId()
                || SetPropW(hwnd as _, property.as_ptr(), owner as usize as _) == 0
            {
                return Err(AppError::project_lock_unavailable());
            }
        }
        Ok(Self { hwnd, property })
    }

    #[cfg(not(windows))]
    pub fn attach(_lease: &ProjectWriteLease, _hwnd: isize) -> Result<Self, AppError> {
        Err(AppError::project_lock_unavailable())
    }
}

#[cfg(windows)]
impl Drop for ProjectWindowRegistration {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::RemovePropW(
                self.hwnd as _,
                self.property.as_ptr(),
            );
        }
    }
}

/// A false result means no registered window was found or Windows declined
/// foreground activation. It never transfers the writer lease.
#[cfg(windows)]
pub fn activate_project_window(key: &str) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, SetForegroundWindow, ShowWindowAsync, SW_RESTORE,
    };
    let Some(hwnd) = find_project_window(key) else {
        return false;
    };
    unsafe {
        if IsIconic(hwnd as _) != 0 {
            ShowWindowAsync(hwnd as _, SW_RESTORE);
        }
        SetForegroundWindow(hwnd as _) != 0
    }
}

#[cfg(windows)]
fn find_project_window(key: &str) -> Option<isize> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetPropW, GetWindowThreadProcessId,
    };
    let property = property_name(key)?;
    struct Search {
        property: Vec<u16>,
        hwnd: HWND,
    }
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> i32 {
        // EnumWindows calls synchronously with the live Search pointer below.
        let search = unsafe { &mut *(data as *mut Search) };
        let value = unsafe { GetPropW(hwnd, search.property.as_ptr()) };
        if !value.is_null() {
            let mut owner = 0;
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut owner);
            }
            if owner != 0 && value as usize == owner as usize {
                search.hwnd = hwnd;
                return 0;
            }
        }
        1
    }
    let mut search = Search {
        property,
        hwnd: std::ptr::null_mut(),
    };
    unsafe {
        EnumWindows(Some(visit), &mut search as *mut Search as LPARAM);
    }
    (!search.hwnd.is_null()).then_some(search.hwnd as isize)
}

#[cfg(not(windows))]
pub fn activate_project_window(key: &str) -> bool {
    let _ = property_name(key);
    false
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WS_OVERLAPPEDWINDOW,
    };

    #[test]
    fn project_window_registration_is_scoped_and_removed_without_releasing_writer() {
        use sha2::{Digest, Sha256};
        let key = format!("{:x}", Sha256::digest(uuid::Uuid::new_v4().as_bytes()));
        let lease = crate::coordination::try_acquire_project_write_lease(&key)
            .unwrap()
            .unwrap();
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // A hidden, test-owned window: never activates or operates a user app.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                10,
                10,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!hwnd.is_null());
        let registration = ProjectWindowRegistration::attach(&lease, hwnd as isize).unwrap();
        assert_eq!(find_project_window(&key), Some(hwnd as isize));
        assert_eq!(find_project_window("invalid"), None);
        assert_eq!(find_project_window(&"f".repeat(64)), None);
        assert!(ProjectWindowRegistration::attach(&lease, 0).is_err());
        drop(registration);
        assert_eq!(find_project_window(&key), None);
        assert!(crate::coordination::try_acquire_project_write_lease(&key)
            .unwrap()
            .is_none());
        unsafe {
            DestroyWindow(hwnd);
        }
    }
}
