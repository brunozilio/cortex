use std::path::{Path, PathBuf};

use super::app::{App, Msg};

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

pub fn show_rules(app: &App, hooks_only: bool) {
    let cwd = app.cwd.clone();
    app.spawn(move |sdk| match sdk.permissions(&cwd) {
        Ok(info) if hooks_only => Msg::Notice(hooks(&info)),
        Ok(info) => Msg::Notice(permissions(&info)),
        Err(error) => Msg::Error(error.to_string()),
    });
}

pub fn permissions(info: &domain::permission_rules::PermissionsInfo) -> String {
    noted(rules(info), &info.notices)
}

pub fn hooks(info: &domain::permission_rules::PermissionsInfo) -> String {
    noted(hook_list(&info.hooks), &info.notices)
}

fn noted(mut text: String, notices: &[String]) -> String {
    for notice in notices {
        text.push_str("\n\n");
        text.push_str(notice);
    }
    text
}

fn rules(info: &domain::permission_rules::PermissionsInfo) -> String {
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

fn hook_list(hooks: &[domain::permission_rules::HookInfo]) -> String {
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

pub fn folder(cwd: &Path, typed: &str) -> Result<PathBuf, String> {
    let path = match typed.strip_prefix("~/") {
        Some(rest) => std::env::home_dir().ok_or("no home folder to start ~/ from")?.join(rest),
        None => cwd.join(typed),
    };
    let real = path.canonicalize().map_err(|_| format!("No folder at {}.", path.display()))?;
    if real.is_dir() { Ok(real) } else { Err(format!("{} is a file, not a folder.", real.display())) }
}

#[cfg(test)]
mod tests {
    use domain::permission_rules::{PermissionsInfo, RuleKind};

    use super::*;

    const SKIPPED: &str = "This project's hooks were skipped: /repo/hooks/guard.js is versioned in git.";

    #[test]
    fn the_permissions_listing_says_which_project_hooks_were_skipped() {
        let mut info = PermissionsInfo { notices: vec![SKIPPED.to_owned()], ..PermissionsInfo::default() };
        assert!(permissions(&info).ends_with(SKIPPED), "even with no rules: {}", permissions(&info));
        info.rules.push((RuleKind::Allow, "Bash(ls:*)".to_owned()));
        let listed = permissions(&info);
        assert!(listed.contains("allow  Bash(ls:*)") && listed.ends_with(SKIPPED), "{listed}");
        info.notices.clear();
        assert!(!permissions(&info).contains("skipped"));
    }

    #[test]
    fn the_hooks_listing_says_which_project_hooks_were_skipped() {
        let info = PermissionsInfo { notices: vec![SKIPPED.to_owned()], ..PermissionsInfo::default() };
        let listed = hooks(&info);
        assert!(listed.starts_with("No hooks.") && listed.ends_with(SKIPPED), "{listed}");
        assert_eq!(hooks(&PermissionsInfo::default()), hook_list(&[]));
    }
}
