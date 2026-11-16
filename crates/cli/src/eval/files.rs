use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub fn copy(from: &Path, to: &Path) -> std::io::Result<()> {
    if !from.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&target)?;
            copy(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn succeeds(dir: &Path, command: &str) -> bool {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn changed(before: &Path, after: &Path) -> Vec<String> {
    let mut paths = BTreeSet::new();
    listed(before, before, &mut paths);
    listed(after, after, &mut paths);
    paths
        .into_iter()
        .filter_map(|path| {
            let (old, new) = (std::fs::read(before.join(&path)).ok(), std::fs::read(after.join(&path)).ok());
            let mark = match (&old, &new) {
                (None, Some(_)) => "A",
                (Some(_), None) => "D",
                (Some(old), Some(new)) if old != new => "M",
                _ => return None,
            };
            Some(format!("{mark} {}", path.display()))
        })
        .collect()
}

fn listed(root: &Path, dir: &Path, into: &mut BTreeSet<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            listed(root, &path, into);
        } else if let Ok(relative) = path.strip_prefix(root) {
            into.insert(relative.to_path_buf());
        }
    }
}
