use std::path::{Path, PathBuf};

pub const PERSIST_OVER: usize = 30_000;
const PREVIEW: usize = 2_000;

pub fn results_dir(session_id: &str) -> PathBuf {
    std::env::temp_dir().join(concat!(brand::command!(), "-tool-results")).join(session_id)
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
    let written = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, content));
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
}
