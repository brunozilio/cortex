use std::collections::BTreeMap;

use serde_json::Value;

use super::{Group, Handler, Hooks};

impl Hooks {
    pub fn from_scripts(folder: &std::path::Path) -> Self {
        let mut hooks = Self::default();
        let Ok(entries) = std::fs::read_dir(folder) else { return hooks };
        let mut files: Vec<_> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "js"))
            .collect();
        files.sort();
        for file in files {
            let shown = file.display().to_string();
            let Ok(source) = std::fs::read_to_string(&file) else { continue };
            let read = |name: &str| script::exported(&shown, &source, name);
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
            for event in events {
                let handler = Handler {
                    kind: "script".into(),
                    command: shown.clone(),
                    prompt: None,
                    model: None,
                    url: None,
                    headers: BTreeMap::new(),
                    allowed_env_vars: Vec::new(),
                    timeout,
                    status_message: None,
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
        assert_eq!(hooks.0.keys().collect::<Vec<_>>(), ["PreToolUse"]);
        let workspace = Workspace::new(dir.path());
        let input = |command: &str| serde_json::json!({ "tool_name": "Bash", "tool_input": { "command": command } });
        let blocked = hooks.run("PreToolUse", "Bash", input("rm -rf /"), &workspace).expect("run");
        assert_eq!(blocked.blocked.as_deref(), Some("no rm -rf"));
        let allowed = hooks.run("PreToolUse", "Bash", input("ls"), &workspace).expect("run");
        assert!(allowed.blocked.is_none());
        let other = hooks.run("PreToolUse", "Read", input("rm -rf /"), &workspace).expect("run");
        assert!(other.blocked.is_none(), "the matcher picks the tools");
    }
}
