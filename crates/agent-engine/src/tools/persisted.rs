use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, SystemTime};

pub const PERSIST_OVER: usize = 30_000;
const PREVIEW: usize = 2_000;

pub const RESULTS_KEPT_FOR: Duration = Duration::from_secs(7 * 24 * 60 * 60);

fn results_root() -> PathBuf {
    std::env::temp_dir().join(concat!(brand::command!(), "-tool-results"))
}

pub fn results_dir(session_id: &str) -> PathBuf {
    results_root().join(session_id)
}

pub fn forget_old_results_once() {
    static SWEPT: Once = Once::new();
    SWEPT.call_once(|| {
        let sweep = || {
            forget_results_older_than(&results_root(), SystemTime::now(), RESULTS_KEPT_FOR);
        };
        if let Err(error) = std::thread::Builder::new().name("old-tool-results".into()).spawn(sweep) {
            tracing::warn!(%error, "old tool results were not cleaned up");
        }
    });
}

pub fn forget_results_older_than(root: &Path, now: SystemTime, kept_for: Duration) -> usize {
    let is_folder = |path: &Path| std::fs::symlink_metadata(path).ok().filter(|found| found.is_dir());
    if is_folder(root).is_none() {
        return 0;
    }
    let mut removed = 0;
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let path = entry.path();
        let old = is_folder(&path)
            .and_then(|found| found.modified().ok())
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > kept_for);
        if old && std::fs::remove_dir_all(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

pub fn write_private(dir: &Path, path: &Path, content: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    platform::make_private_dir(dir)?;
    platform::private_file::write(path, content.as_bytes())
}

fn cut_at(text: &str, at: usize) -> usize {
    (0..=at.min(text.len())).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0)
}

pub fn preview(content: &str, path: &Path) -> String {
    let head_end = cut_at(content, PREVIEW);
    let tail_start = cut_at(content, content.len().saturating_sub(PREVIEW)).max(head_end);
    let omitted = content[head_end..tail_start].chars().count();
    format!(
        "<persisted-output>\nOutput too large ({} characters). Full output saved to: {}\nRead or grep that file for \
         the parts you need.\n\nPreview (start and end):\n{}\n… [{omitted} characters omitted] …\n{}\n</persisted-output>",
        content.chars().count(),
        path.display(),
        &content[..head_end],
        &content[tail_start..],
    )
}

pub fn persist(content: &str, dir: &Path, id: &str) -> Option<String> {
    if content.len() <= PERSIST_OVER {
        return None;
    }
    let name: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    let path = dir.join(format!("{name}.txt"));
    let written = write_private(dir, &path, content);
    match written {
        Ok(()) => Some(preview(content, &path)),
        Err(error) => {
            tracing::warn!(%error, "could not save a large tool output");
            Some(super::clip(content, PERSIST_OVER))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn only_result_folders_untouched_for_longer_than_they_are_kept_are_removed() {
        let root = tempfile::tempdir().expect("dir");
        let (old, fresh) = (root.path().join("old-session"), root.path().join("fresh-session"));
        for folder in [&old, &fresh] {
            persist(&"x".repeat(PERSIST_OVER + 1), folder, "call_1").expect("persisted");
        }
        std::fs::write(root.path().join("stray.txt"), "not a session").expect("file");
        let now = SystemTime::now();
        let week_ago = now - RESULTS_KEPT_FOR - Duration::from_secs(60);
        std::fs::File::open(&old).expect("open").set_modified(week_ago).expect("aged");

        assert_eq!(forget_results_older_than(root.path(), now, RESULTS_KEPT_FOR), 1);
        assert!(!old.exists() && fresh.join("call_1.txt").exists());
        assert!(root.path().join("stray.txt").exists(), "only folders are sessions");
        assert_eq!(forget_results_older_than(&root.path().join("never-made"), now, RESULTS_KEPT_FOR), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_among_the_results_is_never_followed() {
        let root = tempfile::tempdir().expect("dir");
        let elsewhere = tempfile::tempdir().expect("elsewhere");
        std::fs::write(elsewhere.path().join("precious.txt"), "keep").expect("file");
        let results = root.path().join("results");
        std::fs::create_dir_all(results.join("old-session")).expect("folder");
        std::os::unix::fs::symlink(elsewhere.path(), results.join("old-session").join("inside")).expect("link");
        std::os::unix::fs::symlink(elsewhere.path(), results.join("linked-session")).expect("link");
        let long_after = SystemTime::now() + RESULTS_KEPT_FOR + RESULTS_KEPT_FOR;

        assert_eq!(forget_results_older_than(&results, long_after, RESULTS_KEPT_FOR), 1);
        assert!(!results.join("old-session").exists());
        assert!(results.join("linked-session").exists(), "a linked folder is not a session's");
        assert!(elsewhere.path().join("precious.txt").exists(), "nothing behind a link is touched");

        let linked_root = root.path().join("linked-root");
        std::os::unix::fs::symlink(elsewhere.path(), &linked_root).expect("link");
        assert_eq!(forget_results_older_than(&linked_root, long_after, RESULTS_KEPT_FOR), 0);
    }

    #[test]
    fn small_output_stays_in_the_conversation() {
        let dir = tempfile::tempdir().expect("dir");
        assert_eq!(persist("short", dir.path(), "call_1"), None);
        assert!(std::fs::read_dir(dir.path()).expect("dir").next().is_none());
    }

    #[test]
    fn large_output_is_saved_whole_and_previewed_by_its_ends() {
        let dir = tempfile::tempdir().expect("dir");
        let content = format!("START{}END", "é".repeat(PERSIST_OVER));
        let shown = persist(&content, &dir.path().join("session"), "call/1").expect("persisted");
        let saved = dir.path().join("session").join("call_1.txt");
        assert_eq!(std::fs::read_to_string(&saved).expect("saved"), content);
        assert!(shown.contains(&saved.display().to_string()) && shown.contains("Output too large"));
        assert!(shown.contains("START") && shown.contains("END") && shown.contains("characters omitted"));
        assert!(shown.len() < 3 * PREVIEW + 500);
    }

    #[cfg(unix)]
    #[test]
    fn a_saved_output_and_its_folder_are_readable_by_the_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("dir");
        let folder = dir.path().join("session");
        persist(&"x".repeat(PERSIST_OVER + 1), &folder, "call_1").expect("persisted");
        let mode = |path: &Path| std::fs::metadata(path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!((mode(&folder), mode(&folder.join("call_1.txt"))), (0o700, 0o600));
    }
}
