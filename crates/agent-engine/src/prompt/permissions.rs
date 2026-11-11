//! The `permissions` of a settings file: `allow`, `ask` and `deny` rules
//! and `additionalDirectories`. Only the owner's files (the user's settings
//! and the project's `settings.local.json`) can allow or add folders; a
//! checked-in file only restricts, so a cloned project never approves its
//! own commands.

use domain::permission_rules::{PermissionRules, RuleKind};
use serde_json::Value;

pub fn read(settings: &Value, owners: bool, rules: &mut PermissionRules, dirs: &mut Vec<String>) -> Result<(), String> {
    let Some(permissions) = settings.get("permissions") else { return Ok(()) };
    if let Some(list) = permissions.get("additionalDirectories").filter(|_| owners) {
        let list = list.as_array().ok_or("permissions.additionalDirectories must be a list of folders")?;
        for dir in list {
            let dir = dir.as_str().ok_or("permissions.additionalDirectories must be a list of folders")?;
            dirs.push(dir.to_owned());
        }
    }
    for (key, kind) in [("allow", RuleKind::Allow), ("ask", RuleKind::Ask), ("deny", RuleKind::Deny)] {
        if kind == RuleKind::Allow && !owners {
            continue;
        }
        let Some(list) = permissions.get(key) else { continue };
        let list = list.as_array().ok_or(format!("permissions.{key} must be a list of rules"))?;
        for rule in list {
            let rule = rule.as_str().ok_or(format!("permissions.{key} must be a list of rules"))?;
            rules.add(kind, rule).map_err(|error| format!("permissions.{key}: {error}"))?;
        }
    }
    Ok(())
}

/// Keeps allow rule `rule` in the project's `.{command}/settings.local.json`
/// (the owner's, never checked in), next to what the file already holds.
pub fn keep_allow(project: &std::path::Path, rule: &str) -> Result<(), String> {
    let path = project.join(brand::DIR).join("settings.local.json");
    let mut settings: Value = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Object(Default::default()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let root = settings.as_object_mut().ok_or(format!("{} is not a JSON object", path.display()))?;
    let permissions = root.entry("permissions").or_insert_with(|| Value::Object(Default::default()));
    let allow = permissions
        .as_object_mut()
        .ok_or("permissions must be an object")?
        .entry("allow")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or("permissions.allow must be a list of rules")?;
    if !allow.iter().any(|kept| kept.as_str() == Some(rule)) {
        allow.push(Value::String(rule.to_owned()));
    }
    let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    let folder = path.parent().ok_or("no settings folder")?;
    std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    std::fs::write(&path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_kept_rule_joins_the_local_settings_once_and_keeps_the_rest() {
        let repo = tempfile::tempdir().expect("repo");
        let local = repo.path().join(concat!(brand::dir!(), "/settings.local.json"));
        std::fs::create_dir_all(local.parent().expect("folder")).expect("dir");
        std::fs::write(&local, r#"{"enabledMcpServers": ["jira"], "permissions": {"deny": ["Bash(rm:*)"]}}"#)
            .expect("write");
        super::keep_allow(repo.path(), "Bash(cargo test:*)").expect("kept");
        super::keep_allow(repo.path(), "Bash(cargo test:*)").expect("kept again");
        let kept: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&local).expect("read")).expect("json");
        assert_eq!(kept["permissions"]["allow"], serde_json::json!(["Bash(cargo test:*)"]));
        assert_eq!(kept["permissions"]["deny"], serde_json::json!(["Bash(rm:*)"]));
        assert_eq!(kept["enabledMcpServers"], serde_json::json!(["jira"]));
    }

    #[test]
    fn a_kept_rule_starts_the_file_when_there_is_none() {
        let repo = tempfile::tempdir().expect("repo");
        super::keep_allow(repo.path(), "Read(//opt/lib/**)").expect("kept");
        let rules = crate::prompt::settings::load(repo.path(), None, &[]).expect("settings").permissions;
        let cwd = repo.path().to_string_lossy().into_owned();
        let target = domain::permission_rules::Target::Path("/opt/lib/a.h");
        assert!(rules.decide("Read", &target, &cwd, None).is_some(), "loaded back as an allow");
    }
}

#[cfg(test)]
mod loading {
    use domain::permission_rules::{RuleKind, Target};

    use crate::prompt::settings::load;

    #[test]
    fn rules_come_from_every_file_but_only_the_owner_s_allow() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let write = |path: std::path::PathBuf, text: &str| {
            std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
            std::fs::write(path, text).expect("write");
        };
        write(
            repo.path().join(concat!(brand::dir!(), "/settings.json")),
            r#"{"permissions": {"allow": ["Bash"], "deny": ["Bash(rm:*)"]}}"#,
        );
        write(
            repo.path().join(concat!(brand::dir!(), "/settings.local.json")),
            r#"{"permissions": {"allow": ["Bash(cargo test)"]}}"#,
        );
        write(
            home.path().join(concat!(brand::dir!(), "/settings.json")),
            r#"{"permissions": {"ask": ["Bash(git push:*)"]}}"#,
        );
        let rules = load(repo.path(), Some(home.path()), &[]).expect("settings").permissions;
        let cwd = repo.path().to_string_lossy().into_owned();
        let decide = |command| rules.decide("Bash", &Target::Command(command), &cwd, None);
        assert_eq!(decide("cargo test"), Some(RuleKind::Allow));
        assert_eq!(decide("ls"), None, "the checked-in allow is ignored");
        assert_eq!(decide("rm -rf x"), Some(RuleKind::Deny));
        assert_eq!(decide("git push"), Some(RuleKind::Ask));
    }

    #[test]
    fn only_the_owner_adds_folders() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let write = |path: std::path::PathBuf, text: &str| {
            std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
            std::fs::write(path, text).expect("write");
        };
        write(
            repo.path().join(concat!(brand::dir!(), "/settings.json")),
            r#"{"permissions": {"additionalDirectories": ["/"]}}"#,
        );
        write(
            repo.path().join(concat!(brand::dir!(), "/settings.local.json")),
            r#"{"permissions": {"additionalDirectories": ["../shared"]}}"#,
        );
        write(
            home.path().join(concat!(brand::dir!(), "/settings.json")),
            r#"{"permissions": {"additionalDirectories": ["~/notes"]}}"#,
        );
        let settings = load(repo.path(), Some(home.path()), &[]).expect("settings");
        assert_eq!(settings.added_dirs, ["~/notes", "../shared"], "the checked-in one is ignored");
    }

    #[test]
    fn a_malformed_rule_says_which_list_it_is_in() {
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir_all(repo.path().join(brand::DIR)).expect("dir");
        std::fs::write(
            repo.path().join(concat!(brand::dir!(), "/settings.json")),
            r#"{"permissions": {"deny": ["Bash(rm"]}}"#,
        )
        .expect("write");
        let error = load(repo.path(), None, &[]).expect_err("malformed");
        assert!(error.contains("permissions.deny"), "{error}");
    }
}
