//! `/export` writes the conversation as Markdown; `/copy` puts the agent's
//! last answer on the clipboard.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::transcript::{Entry, ToolState};

/// The conversation as Markdown: prompts as quotes, answers as they are,
/// tool calls as one line each.
pub fn markdown(title: Option<&str>, entries: &[Entry]) -> String {
    let mut out = format!("# {}\n", title.unwrap_or("Superfik conversation"));
    for entry in entries {
        let block = match entry {
            Entry::User(text) => text.lines().map(|line| format!("> {line}")).collect::<Vec<_>>().join("\n"),
            Entry::Assistant(text) => text.trim().to_owned(),
            Entry::Tool { name, summary, state, .. } => {
                let failed = if *state == ToolState::Failed { " (failed)" } else { "" };
                format!("- `{name}` {summary}{failed}")
            }
            Entry::Agent { kind, summary, .. } => format!("- agent `{kind}`: {summary}"),
            Entry::Plan { plan, .. } => format!("**Plan**\n\n{}", plan.trim()),
            Entry::Diff { diff, .. } => format!("```diff\n{}\n```", diff.trim_end()),
            Entry::Thinking { .. } | Entry::Notice(_) | Entry::Error(_) => continue,
        };
        out.push('\n');
        out.push_str(&block);
        out.push('\n');
    }
    out
}

/// Where `/export` writes when no path is given: a file named after the
/// title in the project.
pub fn default_path(cwd: &Path, title: Option<&str>) -> PathBuf {
    let name: String = title
        .unwrap_or("conversation")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let name = name.split('-').filter(|part| !part.is_empty()).collect::<Vec<_>>().join("-");
    cwd.join(format!("{}.md", if name.is_empty() { "conversation" } else { &name }))
}

/// The agent's last answer.
pub fn last_answer(entries: &[Entry]) -> Option<&str> {
    entries.iter().rev().find_map(|entry| match entry {
        Entry::Assistant(text) => Some(text.as_str()),
        _ => None,
    })
}

/// Puts `text` on the system clipboard (`pbcopy`, `wl-copy`, `xclip`).
pub fn copy(text: &str) -> Result<(), String> {
    let tries: [(&str, &[&str]); 3] = [("pbcopy", &[]), ("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])];
    for (program, args) in tries {
        let Ok(mut child) = Command::new(program).args(args).stdin(Stdio::piped()).spawn() else { continue };
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        }
        return match child.wait() {
            Ok(status) if status.success() => Ok(()),
            _ => Err(format!("{program} could not take the text")),
        };
    }
    Err("no clipboard tool found (pbcopy, wl-copy or xclip)".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_conversation_reads_as_markdown_without_notices() {
        let entries = vec![
            Entry::User("fix it\nplease".into()),
            Entry::Tool {
                id: "t".into(),
                name: "Edit".into(),
                summary: "src/a.rs".into(),
                state: ToolState::Done,
                output: None,
            },
            Entry::Notice("Mode: plan".into()),
            Entry::Assistant("Done.\n".into()),
        ];
        assert_eq!(
            markdown(Some("Login page"), &entries),
            "# Login page\n\n> fix it\n> please\n\n- `Edit` src/a.rs\n\nDone.\n"
        );
        assert_eq!(last_answer(&entries), Some("Done.\n"));
        assert_eq!(
            default_path(Path::new("/p"), Some("Fix the Login page!")),
            PathBuf::from("/p/fix-the-login-page.md")
        );
    }
}
