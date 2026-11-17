pub fn named() -> String {
    std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).unwrap_or_else(|_| "vi".to_owned())
}

pub fn edit(editor: &str, text: &str) -> Result<String, String> {
    let mut words = editor.split_whitespace();
    let program = words.next().ok_or("no editor set: export EDITOR")?;
    let path = private_file(text).map_err(|e| format!("could not make a file for the prompt: {e}"))?;
    let status = std::process::Command::new(program).args(words).arg(&*path).status();
    let edited = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()));
    drop(path);
    match status {
        Ok(status) if status.success() => edited.map(|text| text.trim_end().to_owned()),
        Ok(status) => Err(format!("{program} ended with {status}; the prompt is as it was")),
        Err(error) => Err(format!("could not start {program}: {error}")),
    }
}

fn private_file(text: &str) -> std::io::Result<tempfile::TempPath> {
    use std::io::Write as _;
    let prefix = format!("{}-prompt-", brand::COMMAND);
    let mut file = tempfile::Builder::new().prefix(&prefix).suffix(".md").tempfile()?;
    file.write_all(text.as_bytes())?;
    file.flush()?;
    Ok(file.into_temp_path())
}

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

    #[test]
    fn the_prompt_is_edited_in_a_private_file_with_a_name_nobody_can_guess() {
        let dir = tempfile::tempdir().expect("dir");
        let (script, seen) = (dir.path().join("look.sh"), dir.path().join("seen"));
        let look = format!("#!/bin/sh\nls -l \"$1\" | cut -c1-10 > '{0}'\necho \"$1\" >> '{0}'\n", seen.display());
        std::fs::write(&script, look).expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("mode");
        super::edit(&script.display().to_string(), "a secret").expect("edited");
        let seen = std::fs::read_to_string(&seen).expect("seen");
        let (mode, path) = seen.trim_end().split_once('\n').expect("mode and path");
        assert_eq!(mode, "-rw-------", "only this user reads the prompt");
        let guessable = format!("{}-prompt-{}.md", brand::COMMAND, std::process::id());
        assert!(!path.ends_with(&guessable) && path.ends_with(".md"), "{path}");
        assert!(!std::path::Path::new(path).exists(), "the file is removed afterwards");
    }
}
