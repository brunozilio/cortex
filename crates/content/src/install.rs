use std::collections::BTreeMap;
use std::path::Path;

pub const MANIFEST: &str = ".content.json";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Installed {
    pub written: Vec<String>,
    pub kept: Vec<String>,
    pub removed: Vec<String>,
}

fn fingerprint(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn read_manifest(folder: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(folder.join(MANIFEST))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let staging = path.with_file_name(format!(".{name}.{}", std::process::id()));
    std::fs::write(&staging, bytes)?;
    std::fs::rename(&staging, path)
}

pub fn install_files(folder: &Path, files: &[(&str, &[u8])]) -> std::io::Result<Installed> {
    let previous = read_manifest(folder);
    let mut manifest = BTreeMap::new();
    let mut installed = Installed::default();
    for (path, bytes) in files {
        let Some(target) = super::override_path(folder, path) else { continue };
        let shipped = fingerprint(bytes);
        let on_disk = std::fs::read(&target).ok().map(|bytes| fingerprint(&bytes));
        let ours = on_disk.is_none() || on_disk == previous.get(*path).cloned() || on_disk.as_ref() == Some(&shipped);
        if !ours {
            installed.kept.push((*path).to_owned());
            if let Some(known) = previous.get(*path) {
                manifest.insert((*path).to_owned(), known.clone());
            }
            continue;
        }
        if on_disk.as_ref() != Some(&shipped) {
            write_atomically(&target, bytes)?;
            installed.written.push((*path).to_owned());
        }
        manifest.insert((*path).to_owned(), shipped);
    }
    for (path, known) in &previous {
        if manifest.contains_key(path) || files.iter().any(|(shipped, _)| shipped == path) {
            continue;
        }
        let Some(target) = super::override_path(folder, path) else { continue };
        match std::fs::read(&target) {
            Ok(bytes) if &fingerprint(&bytes) == known => {
                std::fs::remove_file(&target)?;
                installed.removed.push(path.clone());
            }
            Ok(_) => {
                manifest.insert(path.clone(), known.clone());
            }
            Err(_) => {}
        }
    }
    let text = serde_json::to_string_pretty(&manifest).map_err(std::io::Error::other)?;
    write_atomically(&folder.join(MANIFEST), text.as_bytes())?;
    Ok(installed)
}

pub fn branded(bytes: &[u8]) -> Vec<u8> {
    match std::str::from_utf8(bytes) {
        Ok(text) => brand::text(text).into_bytes(),
        Err(_) => bytes.to_vec(),
    }
}

pub fn install_once() {
    static DONE: std::sync::Once = std::sync::Once::new();
    DONE.call_once(|| {
        let Some(folder) = super::folder() else { return };
        let branded: Vec<(&str, Vec<u8>)> =
            super::embedded::FILES.iter().map(|(path, bytes)| (*path, branded(bytes))).collect();
        let files: Vec<(&str, &[u8])> = branded.iter().map(|(path, bytes)| (*path, bytes.as_slice())).collect();
        match install_files(&folder, &files) {
            Ok(installed) if !installed.kept.is_empty() => {
                tracing::info!(kept = ?installed.kept, "edited copies in the user's folder are kept")
            }
            Ok(_) => {}
            Err(error) => tracing::warn!(%error, folder = %folder.display(), "the built-in content was not copied"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_are_written_updated_while_untouched_and_kept_once_edited() {
        let folder = tempfile::tempdir().expect("folder");
        let first: [(&str, &[u8]); 2] = [("prompts/a.js", b"a1"), ("tools/B.js", b"b1")];
        let installed = install_files(folder.path(), &first).expect("install");
        assert_eq!(installed.written, ["prompts/a.js", "tools/B.js"]);
        assert_eq!(std::fs::read(folder.path().join("tools/B.js")).expect("b"), b"b1");

        assert_eq!(install_files(folder.path(), &first).expect("again"), Installed::default(), "nothing to do");

        std::fs::write(folder.path().join("tools/B.js"), b"mine").expect("edit");
        let second: [(&str, &[u8]); 2] = [("prompts/a.js", b"a2"), ("tools/B.js", b"b2")];
        let installed = install_files(folder.path(), &second).expect("update");
        assert_eq!(installed.written, ["prompts/a.js"]);
        assert_eq!(installed.kept, ["tools/B.js"]);
        assert_eq!(std::fs::read(folder.path().join("prompts/a.js")).expect("a"), b"a2");
        assert_eq!(std::fs::read(folder.path().join("tools/B.js")).expect("b"), b"mine");
        assert_eq!(install_files(folder.path(), &second).expect("kept").kept, ["tools/B.js"], "still the user's");
    }

    #[test]
    fn a_file_no_longer_shipped_goes_unless_the_user_edited_it() {
        let folder = tempfile::tempdir().expect("folder");
        let first: [(&str, &[u8]); 2] = [("prompts/old.js", b"old"), ("prompts/edited.js", b"x")];
        install_files(folder.path(), &first).expect("install");
        std::fs::write(folder.path().join("prompts/edited.js"), b"mine").expect("edit");
        let installed = install_files(folder.path(), &[]).expect("update");
        assert_eq!(installed.removed, ["prompts/old.js"]);
        assert!(!folder.path().join("prompts/old.js").exists());
        assert!(folder.path().join("prompts/edited.js").exists());
    }

    #[test]
    fn text_is_written_with_the_product_s_name_and_other_bytes_as_they_are() {
        let named = branded(b"Ask {{NAME}} in {{DIR}}.");
        assert_eq!(String::from_utf8(named).expect("text"), format!("Ask {} in {}.", brand::NAME, brand::DIR));
        assert_eq!(branded(&[0xff, 0xfe]), [0xff, 0xfe]);
    }

    #[test]
    fn only_plain_paths_are_written() {
        let folder = tempfile::tempdir().expect("folder");
        let files: [(&str, &[u8]); 1] = [("../escape.js", b"no")];
        assert_eq!(install_files(folder.path(), &files).expect("install"), Installed::default());
    }
}
