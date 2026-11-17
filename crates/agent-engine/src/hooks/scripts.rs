use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{Group, Handler, HandlerKind, HookEvent, Hooks};

pub const BUILTIN: &str = "builtin:";

fn scripts_in(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "js"))
        .collect();
    files.sort();
    files
}

pub fn source(command: &str) -> Result<String, String> {
    match command.strip_prefix(BUILTIN) {
        Some(path) => content::builtin_text(path).ok_or_else(|| format!("{path}: no such built-in hook")),
        None => std::fs::read_to_string(command).map_err(|error| format!("{command}: {error}")),
    }
}

impl Hooks {
    pub fn gathered(home: Option<&Path>, roots: &[PathBuf], runs: &dyn Fn(&Path) -> bool) -> Self {
        let user_hooks = home.map(|home| home.join(brand::DIR).join("hooks"));
        let mut hooks = Self::from_builtin(&content::builtin_files("hooks/"), user_hooks.as_deref());
        for root in roots {
            hooks.extend(&Self::from_scripts_that(&root.join("hooks"), runs));
        }
        hooks
    }

    pub fn from_scripts(folder: &Path) -> Self {
        Self::from_scripts_that(folder, &|_| true)
    }

    fn from_scripts_that(folder: &Path, runs: &dyn Fn(&Path) -> bool) -> Self {
        let files = scripts_in(folder).into_iter().filter(|file| runs(file)).filter_map(|file| {
            let source = std::fs::read_to_string(&file).ok()?;
            Some((file.display().to_string(), source))
        });
        Self::from_sources(files)
    }

    pub fn from_builtin(files: &[(&str, &[u8])], user_folder: Option<&Path>) -> Self {
        let replaced = |path: &str| {
            let name = Path::new(path).file_name();
            user_folder.is_some_and(|folder| name.is_some_and(|name| folder.join(name).is_file()))
        };
        let sources = files
            .iter()
            .filter(|(path, _)| path.ends_with(".js") && !replaced(path))
            .map(|(path, bytes)| (format!("{BUILTIN}{path}"), brand::text(&String::from_utf8_lossy(bytes))));
        Self::from_sources(sources)
    }

    fn from_sources(files: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut hooks = Self::default();
        for (shown, source) in files {
            let read = |name: &str| script::exported(&shown, &source, name);
            if !script::exports(&shown, &source, "default") || script::empty_function(&shown, &source, "default") {
                continue;
            }
            let events = match read("event") {
                Ok(Value::String(event)) => vec![event],
                Ok(Value::Array(events)) => events.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
                _ => {
                    tracing::warn!(file = shown, "a hook script names no event");
                    continue;
                }
            };
            let matcher = read("matcher").ok().and_then(|value| value.as_str().map(str::to_owned)).unwrap_or_default();
            let timeout = read("timeout")
                .ok()
                .and_then(|value| value.as_u64())
                .filter(|seconds| (1..=600).contains(seconds))
                .unwrap_or(60);
            for named in events {
                let Some(event) = HookEvent::named(&named) else {
                    tracing::warn!(file = shown, event = named, "a hook script names an unknown event");
                    continue;
                };
                let handler = Handler {
                    kind: HandlerKind::Script,
                    command: shown.clone(),
                    prompt: None,
                    model: None,
                    url: None,
                    headers: BTreeMap::new(),
                    allowed_env_vars: Vec::new(),
                    timeout,
                    status_message: (),
                };
                hooks.0.entry(event).or_default().push(Group { matcher: matcher.clone(), hooks: vec![handler] });
            }
        }
        hooks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Workspace;

    #[test]
    fn a_hook_script_names_its_event_and_blocks_by_returning_a_decision() {
        let dir = tempfile::tempdir().expect("dir");
        let folder = dir.path().join("hooks");
        std::fs::create_dir_all(&folder).expect("hooks");
        std::fs::write(
            folder.join("no-rm.js"),
            r#"export const event = "PreToolUse";
export const matcher = "Bash";
export default function (input) {
  if (input.tool_input.command.includes("rm -rf")) return { decision: "block", reason: "no rm -rf" };
}"#,
        )
        .expect("hook");
        std::fs::write(folder.join("nameless.js"), "export default () => null;").expect("hook");
        let hooks = Hooks::from_scripts(&folder);
        assert_eq!(hooks.0.keys().copied().collect::<Vec<_>>(), [HookEvent::PreToolUse]);
        let workspace = Workspace::new(dir.path());
        let input = |command: &str| serde_json::json!({ "tool_name": "Bash", "tool_input": { "command": command } });
        let blocked = hooks.run(HookEvent::PreToolUse, "Bash", input("rm -rf /"), &workspace).expect("run");
        assert_eq!(blocked.blocked.as_deref(), Some("no rm -rf"));
        let allowed = hooks.run(HookEvent::PreToolUse, "Bash", input("ls"), &workspace).expect("run");
        assert!(allowed.blocked.is_none());
        let other = hooks.run(HookEvent::PreToolUse, "Read", input("rm -rf /"), &workspace).expect("run");
        assert!(other.blocked.is_none(), "the matcher picks the tools");
    }

    #[test]
    fn a_built_in_hook_runs_from_content_unless_the_user_has_one_of_the_same_name() {
        let hook: &[u8] = br#"export const event = "PreToolUse";
export const matcher = "Bash";
export default () => ({ decision: "block", reason: "built in" });"#;
        let files: [(&str, &[u8]); 2] = [("hooks/guard.js", hook), ("hooks/notes.txt", b"not a hook")];
        let hooks = Hooks::from_builtin(&files, None);
        let handlers: Vec<&str> =
            hooks.0[&HookEvent::PreToolUse].iter().map(|group| group.hooks[0].command.as_str()).collect();
        assert_eq!(handlers, ["builtin:hooks/guard.js"]);

        let user = tempfile::tempdir().expect("user");
        std::fs::write(user.path().join("guard.js"), "export const event = \"Stop\";").expect("mine");
        assert!(Hooks::from_builtin(&files, Some(user.path())).0.is_empty(), "the user's copy replaces it");

        let shipped = content::builtin_files("hooks/");
        let events: Vec<String> = shipped
            .iter()
            .filter_map(|(path, source)| script::exported(path, &String::from_utf8_lossy(source), "event").ok())
            .filter_map(|event| event.as_str().map(str::to_owned))
            .collect();
        assert_eq!(events.len(), 19, "one file per event: {events:?}");
        for event in &events {
            assert!(HookEvent::named(event).is_some(), "{event}");
        }
        assert!(Hooks::from_builtin(&shipped, None).0.is_empty(), "an empty default registers nothing");

        assert_eq!(source("builtin:hooks/missing.js"), Err("hooks/missing.js: no such built-in hook".to_owned()));
        assert!(content::builtin_files("hooks/").iter().all(|(path, _)| path.ends_with(".js")));
    }
}
