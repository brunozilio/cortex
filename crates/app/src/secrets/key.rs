use std::path::Path;

use super::sealed::{hex, unhex};

pub fn at(path: &Path) -> Result<[u8; 32], String> {
    if let Some(key) = kept(path)? {
        return Ok(key);
    }
    let made = platform::store::Document::at(path).try_update_text(|text| {
        if !text.is_empty() {
            return read(path, text);
        }
        let mut key = [0u8; 32];
        getrandom::fill(&mut key).map_err(|error| error.to_string())?;
        *text = hex(&key);
        Ok(key)
    });
    made.map_err(|error| unusable(&error))?
}

fn kept(path: &Path) -> Result<Option<[u8; 32]>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => read(path, &text).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(unusable(&format!("{}: {error}", path.display()))),
    }
}

fn read(path: &Path, text: &str) -> Result<[u8; 32], String> {
    unhex(text.trim())
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| format!("the secrets key {} is not 64 hex digits", path.display()))
}

fn unusable(error: &dyn std::fmt::Display) -> String {
    format!("the secrets key cannot be used: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_key_that_cannot_be_read_is_an_error_and_is_left_as_it_was() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("secrets.key");
        let key = at(&path).expect("made");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("lock out");
        if std::fs::read(&path).is_ok() {
            return;
        }

        let error = at(&path).expect_err("an unreadable key is not replaced");

        assert!(error.contains("secrets.key"), "{error}");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("let in");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), hex(&key), "the vault's key is still the one made");
    }

    #[test]
    fn a_folder_where_the_key_should_be_is_an_error_not_a_new_key() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("secrets.key");
        std::fs::create_dir(&path).expect("folder");
        assert!(at(&path).is_err());
        assert!(path.is_dir());
    }

    #[test]
    fn first_runs_at_once_end_with_one_key() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("secrets.key");
        let start = std::sync::Barrier::new(16);
        let made: Vec<[u8; 32]> = std::thread::scope(|scope| {
            let runs: Vec<_> = (0..16)
                .map(|_| {
                    scope.spawn(|| {
                        start.wait();
                        at(&path).expect("key")
                    })
                })
                .collect();
            runs.into_iter().map(|run| run.join().expect("join")).collect()
        });
        let kept = at(&path).expect("kept");
        assert!(made.iter().all(|key| *key == kept), "every first run seals with the key that was kept");
    }
}
