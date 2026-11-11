//! `/add-dir`: folders the agent reads like the project's own.

use std::path::{Path, PathBuf};

use super::app::App;

pub fn run(app: &mut App, typed: Option<String>) {
    let Some(typed) = typed else {
        return app.transcript.notice(if app.added_dirs.is_empty() {
            "No folders added. /add-dir <path> lets the agent read one like the project.".to_owned()
        } else {
            let dirs: Vec<String> = app.added_dirs.iter().map(|dir| format!("  {}", dir.display())).collect();
            format!("Added folders:\n{}", dirs.join("\n"))
        });
    };
    match folder(&app.cwd, &typed) {
        Ok(dir) => {
            app.transcript.notice(format!("Added {}: the agent reads it like the project.", dir.display()));
            app.add_dir(dir);
        }
        Err(error) => app.transcript.error(error),
    }
}

/// How `/permissions` reads: each rule under what it does, and the added
/// folders.
pub fn permissions(info: &domain::permission_rules::PermissionsInfo) -> String {
    use domain::permission_rules::RuleKind;
    if info.rules.is_empty() && info.added_dirs.is_empty() {
        return concat!(
            "No permission rules. Add permissions.allow/ask/deny to ",
            brand::dir!(),
            "/settings.local.json, \
                or answer \"don't ask again\" when the agent asks."
        )
        .to_owned();
    }
    let mut text = String::from("Permission rules (deny wins over ask, ask over allow):");
    for (kind, rule) in &info.rules {
        let label = match kind {
            RuleKind::Deny => "deny ",
            RuleKind::Ask => "ask  ",
            RuleKind::Allow => "allow",
        };
        text.push_str(&format!("\n  {label}  {rule}"));
    }
    for dir in &info.added_dirs {
        text.push_str(&format!("\n  folder {dir}"));
    }
    text
}

/// How `/hooks` reads: each hook under its event.
pub fn hooks(hooks: &[domain::permission_rules::HookInfo]) -> String {
    if hooks.is_empty() {
        return concat!("No hooks. Add them under \"hooks\" in ", brand::dir!(), "/settings.json.").to_owned();
    }
    let mut text = String::from("Hooks:");
    for hook in hooks {
        let matcher = if hook.matcher.is_empty() { "*" } else { &hook.matcher };
        text.push_str(&format!("\n  {:<18} {matcher:<12} {}", hook.event, hook.runs));
    }
    text
}

/// The folder `typed` names, from home (`~/`) or the project, made real.
pub fn folder(cwd: &Path, typed: &str) -> Result<PathBuf, String> {
    let path = match typed.strip_prefix("~/") {
        Some(rest) => std::env::home_dir().ok_or("no home folder to start ~/ from")?.join(rest),
        None => cwd.join(typed),
    };
    let real = path.canonicalize().map_err(|_| format!("No folder at {}.", path.display()))?;
    if real.is_dir() { Ok(real) } else { Err(format!("{} is a file, not a folder.", real.display())) }
}
