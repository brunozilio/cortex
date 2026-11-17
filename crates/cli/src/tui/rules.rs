use domain::permission_rules::{HookInfo, PermissionsInfo};

use super::app::{App, Msg};

pub fn show(app: &App, hooks_only: bool) {
    let cwd = app.cwd.clone();
    app.spawn(move |client| match client.permissions(&cwd) {
        Ok(info) if hooks_only => Msg::Notice(hooks(&info)),
        Ok(info) => Msg::Notice(permissions(&info)),
        Err(error) => Msg::Error(error.to_string()),
    });
}

pub fn permissions(info: &PermissionsInfo) -> String {
    noted(rules(info), &info.notices)
}

pub fn hooks(info: &PermissionsInfo) -> String {
    noted(hook_list(&info.hooks), &info.notices)
}

fn noted(mut text: String, notices: &[String]) -> String {
    for notice in notices {
        text.push_str("\n\n");
        text.push_str(notice);
    }
    text
}

fn rules(info: &PermissionsInfo) -> String {
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
        text.push_str(&format!("\n  {:<5}  {rule}", kind.word()));
    }
    for dir in &info.added_dirs {
        text.push_str(&format!("\n  folder {dir}"));
    }
    text
}

fn hook_list(hooks: &[HookInfo]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    const SKIPPED: &str = "Skipped: folder not trusted (/trust).\n  Hook scripts:\n    hooks/guard.js";

    #[test]
    fn the_permissions_listing_says_what_was_skipped_because_the_folder_is_not_trusted() {
        let mut info = PermissionsInfo { notices: vec![SKIPPED.to_owned()], ..PermissionsInfo::default() };
        assert!(permissions(&info).ends_with(SKIPPED), "even with no rules: {}", permissions(&info));
        info.rules.push((domain::permission_rules::RuleKind::Allow, "Bash(ls:*)".to_owned()));
        let listed = permissions(&info);
        assert!(listed.contains("allow  Bash(ls:*)") && listed.ends_with(SKIPPED), "{listed}");
        info.notices.clear();
        assert!(!permissions(&info).contains("not trusted"));
    }

    #[test]
    fn the_hooks_listing_says_what_was_skipped_because_the_folder_is_not_trusted() {
        let info = PermissionsInfo { notices: vec![SKIPPED.to_owned()], ..PermissionsInfo::default() };
        let listed = hooks(&info);
        assert!(listed.starts_with("No hooks.") && listed.ends_with(SKIPPED), "{listed}");
        assert_eq!(hooks(&PermissionsInfo::default()), hook_list(&[]));
    }
}
