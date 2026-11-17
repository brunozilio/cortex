use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use domain::output::{PLAIN_ENV, Saving, clean, filter, program};

use crate::tools::Workspace;

pub(super) fn plain(shell: &mut std::process::Command, workspace: &Workspace) {
    if workspace.policy.output.cleanup {
        shell.envs(PLAIN_ENV);
    }
}

pub(super) fn shaped(command: &str, output: &str, workspace: &Workspace) -> String {
    let shaping = workspace.policy.output;
    let mut text = if shaping.cleanup { clean(output) } else { output.to_owned() };
    if shaping.filters
        && let Some(filtered) = filter(command, &text)
        && let Some(whole) = kept_whole(&text, workspace)
    {
        text = format!("{}\n{}", filtered.text, filtered.footer(&whole.display().to_string()));
    }
    if shaping.savings {
        let saving = Saving { program: program(command), before: output.chars().count(), after: text.chars().count() };
        super::savings::record(workspace.credentials.client, saving);
    }
    text
}

pub(super) fn kept_whole(text: &str, workspace: &Workspace) -> Option<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let dir = crate::tools::persisted::results_dir(&workspace.session_id);
    let name = format!("bash-{}-{}.txt", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
    let path = dir.join(name);
    match crate::tools::persisted::write_private(&dir, &path, &workspace.policy.redacted(text)) {
        Ok(()) => Some(path),
        Err(error) => {
            tracing::warn!(%error, "could not keep a command's whole output");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::output::Shaping;

    use super::*;

    fn cargo_output() -> String {
        let passing: String = (0..40).map(|i| format!("test case_{i} ... ok\n")).collect();
        format!("\u{1b}[32m   Compiling\u{1b}[0m x v0.1.0\n{passing}test broken ... FAILED\ntest result: FAILED")
    }

    fn workspace(output: Shaping, client: u64) -> Workspace {
        let mut workspace = Workspace::new(std::env::temp_dir());
        workspace.session_id = format!("shaped-{client}");
        workspace.credentials.client = client;
        workspace.policy.output = output;
        workspace
    }

    #[test]
    fn with_every_flag_off_the_output_is_left_as_it_came() {
        let workspace = workspace(Shaping::default(), 92_001);
        assert_eq!(shaped("cargo test", &cargo_output(), &workspace), cargo_output());
        assert!(super::super::savings::list(92_001).is_empty());
    }

    #[test]
    fn cleanup_alone_drops_escape_codes_but_keeps_every_line() {
        let workspace = workspace(Shaping { cleanup: true, ..Shaping::default() }, 92_002);
        let shown = shaped("cargo test", &cargo_output(), &workspace);
        assert!(shown.starts_with("   Compiling x v0.1.0\ntest case_0 ... ok"), "{shown}");
        let mut shell = std::process::Command::new("true");
        plain(&mut shell, &workspace);
        assert!(shell.get_envs().any(|(name, value)| name == "NO_COLOR" && value == Some("1".as_ref())));
    }

    #[cfg(unix)]
    #[test]
    fn a_command_s_whole_output_is_kept_for_the_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let workspace = workspace(Shaping::default(), 92_004);
        let path = kept_whole("secret output", &workspace).expect("kept");
        let mode = |path: &std::path::Path| std::fs::metadata(path).expect("metadata").permissions().mode() & 0o777;
        let modes = (mode(path.parent().expect("folder")), mode(&path));
        let _ = std::fs::remove_dir_all(path.parent().expect("folder"));
        assert_eq!(modes, (0o700, 0o600));
    }

    #[test]
    fn filters_hide_the_noise_keep_the_whole_output_and_count_the_saving() {
        let workspace = workspace(Shaping { cleanup: true, filters: true, savings: true }, 92_003);
        let shown = shaped("cargo test", &cargo_output(), &workspace);
        let (kept, footer) = shown.split_once("\n[").expect("footer");
        assert_eq!(kept, "test broken ... FAILED\ntest result: FAILED");
        let whole = footer.rsplit_once("the whole output is in ").and_then(|(_, path)| path.strip_suffix(']'));
        let saved = std::fs::read_to_string(whole.expect("path")).expect("saved");
        assert!(saved.contains("test case_39 ... ok") && !saved.contains('\u{1b}'), "{saved}");

        let savings = super::super::savings::list(92_003);
        assert_eq!(savings.len(), 1);
        assert_eq!(savings[0].program, "cargo");
        assert!(savings[0].after < savings[0].before / 2, "{savings:?}");
        super::super::savings::forget(92_003);
    }
}
