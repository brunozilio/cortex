//! Ctrl+G: the prompt edited in `$VISUAL` or `$EDITOR` (else `vi`).

/// The editor the user named, as a program and its arguments.
pub fn named() -> String {
    std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).unwrap_or_else(|_| "vi".to_owned())
}

/// `text` as the user left it after editing it with `editor` (a program
/// and arguments, the file added last).
pub fn edit(editor: &str, text: &str) -> Result<String, String> {
    let mut words = editor.split_whitespace();
    let program = words.next().ok_or("no editor set: export EDITOR")?;
    let path = std::env::temp_dir().join(format!(
        "{brand_command}-prompt-{}.md",
        std::process::id(),
        brand_command = brand::COMMAND
    ));
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    let status = std::process::Command::new(program).args(words).arg(&path).status();
    let edited = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()));
    let _ = std::fs::remove_file(&path);
    match status {
        Ok(status) if status.success() => edited.map(|text| text.trim_end().to_owned()),
        Ok(status) => Err(format!("{program} ended with {status}; the prompt is as it was")),
        Err(error) => Err(format!("could not start {program}: {error}")),
    }
}

/// Opens `path` in `editor` and waits for it to close.
pub fn open(editor: &str, path: &std::path::Path) -> Result<(), String> {
    let mut words = editor.split_whitespace();
    let program = words.next().ok_or("no editor set: export EDITOR")?;
    match std::process::Command::new(program).args(words).arg(path).status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("{program} ended with {status}")),
        Err(error) => Err(format!("could not start {program}: {error}")),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn the_prompt_comes_back_as_the_editor_left_it() {
        let dir = tempfile::tempdir().expect("dir");
        let script = dir.path().join("append.sh");
        std::fs::write(&script, "#!/bin/sh\nprintf ' and the docs\\n' >> \"$1\"\n").expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let edited = super::edit(&script.display().to_string(), "fix the tests").expect("edited");
        assert_eq!(edited, "fix the tests and the docs");
        assert!(super::edit("false", "x").is_err(), "a failing editor keeps the prompt");
    }
}
