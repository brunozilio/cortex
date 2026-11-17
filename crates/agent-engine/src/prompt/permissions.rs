use domain::permission_rules::{PermissionRules, RuleKind};
use platform::store::Document;
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
    for kind in RuleKind::ALL {
        let key = kind.word();
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

pub fn read_judge(settings: &Value, rules: &mut PermissionRules) -> Result<(), String> {
    let Some(list) = settings.get("verification").and_then(|verification| verification.get("allow")) else {
        return Ok(());
    };
    let list = list.as_array().ok_or("verification.allow must be a list of rules")?;
    for rule in list {
        let rule = rule.as_str().ok_or("verification.allow must be a list of rules")?;
        rules.add(RuleKind::Allow, rule).map_err(|error| format!("verification.allow: {error}"))?;
    }
    Ok(())
}

pub fn keep_allow(project: &std::path::Path, rule: &str) -> Result<(), String> {
    keep_rule(project, RuleKind::Allow, rule)
}

pub fn keep_rule(project: &std::path::Path, kind: RuleKind, rule: &str) -> Result<(), String> {
    let list = kind.word();
    let path = project.join(brand::DIR).join("settings.local.json");
    let kept = Document::hand_edited(&path).try_update(|settings: &mut Value| {
        if settings.is_null() {
            *settings = Value::Object(Default::default());
        }
        let root = settings.as_object_mut().ok_or(format!("{} is not a JSON object", path.display()))?;
        let permissions = root.entry("permissions").or_insert_with(|| Value::Object(Default::default()));
        let rules = permissions
            .as_object_mut()
            .ok_or("permissions must be an object")?
            .entry(list)
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or(format!("permissions.{list} must be a list of rules"))?;
        if !rules.iter().any(|kept| kept.as_str() == Some(rule)) {
            rules.push(Value::String(rule.to_owned()));
        }
        Ok::<(), String>(())
    });
    kept.map_err(|error| error.to_string())?
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
    fn rules_kept_at_once_are_all_kept_and_the_file_never_reads_half_written() {
        let repo = tempfile::tempdir().expect("repo");
        let local = repo.path().join(concat!(brand::dir!(), "/settings.local.json"));
        std::thread::scope(|scope| {
            for writer in 0..4 {
                let repo = repo.path();
                scope.spawn(move || {
                    for rule in 0..10 {
                        super::keep_allow(repo, &format!("Bash(tool-{writer}-{rule}:*)")).expect("kept");
                    }
                });
            }
            scope.spawn(|| {
                for _ in 0..200 {
                    if let Ok(text) = std::fs::read_to_string(&local) {
                        serde_json::from_str::<serde_json::Value>(&text).expect("a reader sees a whole document");
                    }
                }
            });
        });
        let kept: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&local).expect("read")).expect("json");
        assert_eq!(kept["permissions"]["allow"].as_array().map(Vec::len), Some(40));
    }

    #[test]
    fn settings_that_do_not_parse_are_reported_and_left_for_their_author() {
        let repo = tempfile::tempdir().expect("repo");
        let local = repo.path().join(concat!(brand::dir!(), "/settings.local.json"));
        std::fs::create_dir_all(local.parent().expect("folder")).expect("dir");
        std::fs::write(&local, "{\"permissions\": {\"deny\": [").expect("write");

        let error = super::keep_allow(repo.path(), "Bash(ls:*)").expect_err("refused");

        assert!(error.contains("settings.local.json"), "{error}");
        assert_eq!(std::fs::read_to_string(&local).expect("read"), "{\"permissions\": {\"deny\": [");
        let aside = std::fs::read_dir(local.parent().expect("folder")).expect("list").flatten();
        assert_eq!(aside.filter(|entry| entry.file_name().to_string_lossy().contains("corrupt")).count(), 0);
    }

    #[test]
    fn a_kept_rule_starts_the_file_when_there_is_none() {
        let repo = tempfile::tempdir().expect("repo");
        super::keep_allow(repo.path(), "Read(//opt/lib/**)").expect("kept");
        let rules = crate::prompt::settings::load_trusted(repo.path(), None, &[]).expect("settings").permissions;
        let cwd = repo.path().to_string_lossy().into_owned();
        let target = domain::permission_rules::Target::Path("/opt/lib/a.h");
        assert!(rules.decide("Read", &target, &cwd, None).is_some(), "loaded back as an allow");
    }
}

#[cfg(test)]
mod loading {
    use domain::permission_rules::{RuleKind, Target};

    use crate::prompt::settings::load_trusted as load;

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
        let skipped = load(repo.path(), None, &[]).expect("a project's malformed file is skipped").notices;
        assert!(skipped.iter().any(|notice| notice.contains("permissions.deny")), "{skipped:?}");

        let home = tempfile::tempdir().expect("home");
        std::fs::create_dir_all(home.path().join(brand::DIR)).expect("dir");
        std::fs::write(
            home.path().join(concat!(brand::dir!(), "/settings.json")),
            r#"{"permissions": {"deny": ["Bash(rm"]}}"#,
        )
        .expect("write");
        let error = load(repo.path(), Some(home.path()), &[]).expect_err("the owner's malformed file stops the run");
        assert!(error.contains("permissions.deny"), "{error}");
    }
}
