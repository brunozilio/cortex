//! The machine `install` changes: this one, as root.

use std::path::Path;

use super::Machine;

#[derive(Debug)]
pub struct System;

impl Machine for System {
    fn run(&mut self, program: &str, args: &[&str]) -> std::io::Result<bool> {
        Ok(std::process::Command::new(program).args(args).status()?.success())
    }

    fn has(&mut self, program: &str) -> bool {
        std::env::var_os("PATH")
            .is_some_and(|path| std::env::split_paths(&path).any(|folder| folder.join(program).is_file()))
    }

    fn read(&mut self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn write(&mut self, path: &Path, contents: &str, mode: u32) -> std::io::Result<()> {
        let pending = path.with_extension("superfik-new");
        let written = (|| {
            std::fs::write(&pending, contents)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&pending, std::fs::Permissions::from_mode(mode))?;
            }
            #[cfg(not(unix))]
            let _ = mode;
            std::fs::rename(&pending, path)
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&pending);
        }
        written
    }

    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::rename(from, to)
    }

    fn untrusted(&mut self, path: &Path) -> Option<String> {
        #[cfg(unix)]
        {
            use super::trusted;
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            match std::fs::symlink_metadata(path) {
                Err(error) => Some(format!("cannot be read ({error})")),
                Ok(meta) if meta.file_type().is_symlink() => Some("is a symbolic link".to_owned()),
                Ok(meta) if !trusted(meta.uid(), meta.permissions().mode()) => {
                    Some("is not root's alone (owned by root, writable by no one else)".to_owned())
                }
                Ok(_) => None,
            }
        }
        #[cfg(not(unix))]
        Some(format!("{} cannot be checked here", path.display()))
    }
}

pub fn effective_uid() -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata("/proc/self").ok().map(|meta| meta.uid())
    }
    #[cfg(not(target_os = "linux"))]
    None
}
