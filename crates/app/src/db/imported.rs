use std::io;
use std::path::{Path, PathBuf};

pub(crate) const SUFFIX: &str = "imported";

pub(crate) fn set_aside(legacy: &Path) -> io::Result<PathBuf> {
    let name = legacy.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let first = legacy.with_file_name(format!("{name}.{SUFFIX}"));
    let aside = std::iter::once(first)
        .chain((1..).map(|again| legacy.with_file_name(format!("{name}.{SUFFIX}-{again}"))))
        .find(|candidate| !candidate.exists())
        .ok_or_else(|| io::Error::other("no free name to keep the imported file under"))?;
    std::fs::rename(legacy, &aside)?;
    Ok(aside)
}

pub(crate) fn set_aside_or_warn(legacy: &Path) {
    match set_aside(legacy) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => tracing::warn!(%error, file = %legacy.display(), "imported, but the file could not be renamed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_imported_file_is_renamed_and_never_replaces_an_earlier_one() {
        let dir = tempfile::tempdir().expect("dir");
        let legacy = dir.path().join("sessions.json");

        std::fs::write(&legacy, "first").expect("write");
        assert_eq!(set_aside(&legacy).expect("aside"), dir.path().join("sessions.json.imported"));
        std::fs::write(&legacy, "second").expect("write");
        assert_eq!(set_aside(&legacy).expect("aside"), dir.path().join("sessions.json.imported-1"));

        assert!(!legacy.exists());
        assert_eq!(std::fs::read_to_string(dir.path().join("sessions.json.imported")).expect("read"), "first");
        set_aside_or_warn(&legacy);
    }
}
