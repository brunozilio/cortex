use std::path::{Path, PathBuf};

use super::app::App;

pub fn run(app: &mut App, typed: Option<String>) {
    let Some(typed) = typed else {
        let added = app.session.added_dirs();
        return app.transcript.notice(if added.is_empty() {
            "No folders added. /add-dir <path> lets the agent read one like the project.".to_owned()
        } else {
            let dirs: Vec<String> = added.iter().map(|dir| format!("  {}", dir.display())).collect();
            format!("Added folders:\n{}", dirs.join("\n"))
        });
    };
    match folder(&app.cwd, &typed) {
        Ok(dir) => {
            app.transcript.notice(format!("Added {}: the agent reads it like the project.", dir.display()));
            app.session.add_dir(dir);
        }
        Err(error) => app.transcript.error(error),
    }
}

pub fn folder(cwd: &Path, typed: &str) -> Result<PathBuf, String> {
    let path = match typed.strip_prefix("~/") {
        Some(rest) => std::env::home_dir().ok_or("no home folder to start ~/ from")?.join(rest),
        None => cwd.join(typed),
    };
    let real = path.canonicalize().map_err(|_| format!("No folder at {}.", path.display()))?;
    if real.is_dir() { Ok(real) } else { Err(format!("{} is a file, not a folder.", real.display())) }
}
