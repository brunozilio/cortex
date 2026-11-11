//! Files only the user running the app can read: secrets, tokens and
//! MCP configurations (ADR 0013).
//!
//! - Unix: mode `0600`, also on a file that already existed.
//! - Windows: a protected DACL granting the current user alone, so the
//!   file inherits nothing from its folder.
//!
//! The file is restricted before it is handed back, so nothing is written
//! to it while others could read it.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

pub fn open(options: &mut OpenOptions, path: &Path) -> io::Result<File> {
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(options, 0o600);
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(windows)]
    windows::restrict(path)?;
    Ok(file)
}

pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    io::Write::write_all(&mut open(OpenOptions::new().write(true).create(true).truncate(true), path)?, bytes)
}

pub fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = temporary_beside(path);
    let written = (|| {
        let mut file = open(OpenOptions::new().write(true).create_new(true), &temporary)?;
        io::Write::write_all(&mut file, bytes)?;
        #[cfg(unix)]
        keep_owner(&file, path)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

fn temporary_beside(path: &Path) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let unique = format!(".{name}.{}.{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
    path.with_file_name(unique)
}

#[cfg(unix)]
fn keep_owner(file: &File, path: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let Ok(old) = std::fs::metadata(path) else { return Ok(()) };
    let new = file.metadata()?;
    if (old.uid(), old.gid()) == (new.uid(), new.gid()) {
        return Ok(());
    }
    match std::os::unix::fs::fchown(file, Some(old.uid()), Some(old.gid())) {
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => Ok(()),
        done => done,
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, GENERIC_ALL, HANDLE, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        EXPLICIT_ACCESS_W, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT, SET_ACCESS, SetEntriesInAclW, SetNamedSecurityInfoW,
        TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        ACL, DACL_SECURITY_INFORMATION, GetTokenInformation, NO_INHERITANCE, PROTECTED_DACL_SECURITY_INFORMATION,
        TOKEN_QUERY, TOKEN_USER, TokenUser,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    pub(super) fn restrict(path: &Path) -> io::Result<()> {
        let user = current_user()?;
        // SAFETY: `user` holds a TOKEN_USER filled by GetTokenInformation;
        let sid = unsafe { (*user.as_ptr().cast::<TOKEN_USER>()).User.Sid };
        let access = EXPLICIT_ACCESS_W {
            grfAccessPermissions: GENERIC_ALL,
            grfAccessMode: SET_ACCESS,
            grfInheritance: NO_INHERITANCE,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: std::ptr::null_mut(),
                MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_USER,
                ptstrName: sid.cast(),
            },
        };
        let mut acl: *mut ACL = std::ptr::null_mut();
        // SAFETY: one valid entry, no old ACL; `acl` receives a LocalAlloc'd ACL.
        let made = unsafe { SetEntriesInAclW(1, &access, std::ptr::null(), &mut acl) };
        if made != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(made as i32));
        }
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        // SAFETY: `wide` is a NUL-terminated path; `acl` was just built.
        let set = unsafe {
            SetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                acl,
                std::ptr::null(),
            )
        };
        // SAFETY: `acl` came from SetEntriesInAclW and is freed once.
        unsafe { LocalFree(acl.cast()) };
        if set != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(set as i32));
        }
        Ok(())
    }

    fn current_user() -> io::Result<Vec<u64>> {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: the pseudo-handle of this process needs no closing.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut needed = 0u32;
        // SAFETY: a null buffer of size 0 only asks for the size needed.
        unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed) };
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8).max(1)];
        let size = u32::try_from(buffer.len() * 8).unwrap_or(u32::MAX);
        // SAFETY: `buffer` holds `size` bytes, aligned for TOKEN_USER.
        let read = unsafe { GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), size, &mut needed) };
        let error = io::Error::last_os_error();
        // SAFETY: `token` was opened above and is closed once.
        unsafe { CloseHandle(token) };
        if read == 0 {
            return Err(error);
        }
        Ok(buffer)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn mode(path: &Path) -> io::Result<u32> {
        Ok(std::fs::metadata(path)?.permissions().mode() & 0o777)
    }

    #[test]
    fn a_new_file_is_the_users_alone() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("secret.json");
        write(&path, b"{}")?;
        assert_eq!(mode(&path)?, 0o600);
        assert_eq!(std::fs::read(&path)?, b"{}");
        Ok(())
    }

    #[test]
    fn a_replaced_file_is_whole_and_the_users_alone() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("keys").join("sealed.json");
        replace(&path, b"one")?;
        replace(&path, b"two")?;
        assert_eq!(std::fs::read(&path)?, b"two");
        assert_eq!(mode(&path)?, 0o600);
        assert!(!path.with_extension("tmp").exists());
        Ok(())
    }

    #[test]
    fn writers_at_once_never_share_a_temporary_file() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("accounts.json");
        let writers: Vec<_> = (0..8u8)
            .map(|writer| {
                let path = path.clone();
                std::thread::spawn(move || (0..50).try_for_each(|_| replace(&path, &[writer; 4096])))
            })
            .collect();
        for writer in writers {
            writer.join().map_err(|_| io::Error::other("writer panicked"))??;
        }
        let bytes = std::fs::read(&path)?;
        assert!(bytes.len() == 4096 && bytes.iter().all(|byte| *byte == bytes[0]), "one writer's file, whole");
        assert_eq!(std::fs::read_dir(dir.path())?.count(), 1, "no temporary file left behind");
        Ok(())
    }

    #[test]
    fn a_replaced_file_keeps_its_owner_and_group() -> io::Result<()> {
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("accounts.json");
        replace(&path, b"one")?;
        let made = std::fs::metadata(&path)?;
        let groups = std::process::Command::new("id").arg("-G").output()?.stdout;
        let other = String::from_utf8_lossy(&groups)
            .split_whitespace()
            .filter_map(|gid| gid.parse::<u32>().ok())
            .find(|gid| *gid != made.gid());
        let Some(other) = other else { return Ok(()) };
        std::os::unix::fs::chown(&path, None, Some(other))?;
        replace(&path, b"two")?;
        let replaced = std::fs::metadata(&path)?;
        assert_eq!((replaced.uid(), replaced.gid()), (made.uid(), other));
        assert_eq!(mode(&path)?, 0o600);
        Ok(())
    }

    #[test]
    fn a_file_others_could_read_is_closed_to_them() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("audit.log");
        std::fs::write(&path, "before\n")?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))?;
        io::Write::write_all(&mut open(OpenOptions::new().create(true).append(true), &path)?, b"after\n")?;
        assert_eq!(mode(&path)?, 0o600);
        assert_eq!(std::fs::read_to_string(&path)?, "before\nafter\n");
        Ok(())
    }
}
