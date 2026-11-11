use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

pub fn restrict_open(options: &mut OpenOptions) {
    std::os::unix::fs::OpenOptionsExt::mode(options, 0o600);
}

pub fn restrict(file: &File, _path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
}

pub fn keep_owner(file: &File, path: &Path) -> io::Result<()> {
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

#[cfg(test)]
mod tests {
    use crate::private_file::open;
    use crate::private_file::{replace, write};
    use std::fs::OpenOptions;
    use std::io;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

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
