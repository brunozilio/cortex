//! Files a member sends from their computer for their runs (ADR 0010: the
//! server runs everything, so what a prompt reads must be on the server).
//! Each is kept under the member's checkouts, where their commands may
//! point, in a folder of its own.

use std::path::{Path, PathBuf};

use base64::Engine;

pub const UPLOAD_AT_MOST: usize = 25 * 1024 * 1024;

pub fn keep(home: &Path, name: &str, data: &str) -> Result<PathBuf, String> {
    if data.len() / 4 * 3 > UPLOAD_AT_MOST {
        return Err(format!("files over {} MB cannot be sent", UPLOAD_AT_MOST / (1024 * 1024)));
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(data).map_err(|_| "that file arrived unreadable")?;
    let folder = home.join("uploads").join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let path = folder.join(file_name(name));
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    Ok(path)
}

fn file_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let kept: String = last.chars().filter(|c| !c.is_control()).collect();
    let kept = kept.trim_start_matches('.');
    if kept.is_empty() { "file".to_owned() } else { kept.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_kept_in_a_folder_of_its_own_under_home() {
        let home = tempfile::tempdir().expect("home");
        let data = base64::engine::general_purpose::STANDARD.encode(b"sky");
        let path = keep(home.path(), "sky.png", &data).expect("kept");
        assert!(path.starts_with(home.path().join("uploads")));
        assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("sky.png"));
        assert_eq!(std::fs::read(&path).expect("read"), b"sky");
    }

    #[test]
    fn a_name_never_climbs_out_or_hides() {
        assert_eq!(file_name("../../etc/passwd"), "passwd");
        assert_eq!(file_name("C:\\x\\notes.md"), "notes.md");
        assert_eq!(file_name(".bashrc"), "bashrc");
        assert_eq!(file_name(".."), "file");
        assert_eq!(file_name(""), "file");
    }

    #[test]
    fn too_large_or_unreadable_is_refused() {
        let home = tempfile::tempdir().expect("home");
        assert!(keep(home.path(), "x", "!!").is_err());
        let huge = "A".repeat(UPLOAD_AT_MOST / 3 * 4 + 8);
        assert!(keep(home.path(), "x", &huge).is_err_and(|why| why.contains("MB")));
    }
}
