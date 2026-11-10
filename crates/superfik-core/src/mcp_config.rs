//! Rules for MCP servers a person or a project configures (`.mcp.json`):
//! which of a project's servers may start, and how `${VAR}` and
//! `${VAR:-default}` in a config are filled in. Claude Code's keys and
//! syntax, so the same files work in both.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Names the platform keeps for itself: the app's own servers and plugins'.
pub const RESERVED_PREFIXES: [&str; 2] = ["superfik_", "plugin_"];

/// Whether a name is free for a person's or a project's server.
pub fn is_free_name(name: &str) -> bool {
    !RESERVED_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

/// What the owner said about a project's `.mcp.json` servers
/// (`enableAllProjectMcpServers`, `enabledMcpjsonServers`,
/// `disabledMcpjsonServers`). A repository cannot say it for itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpApproval {
    pub all: bool,
    pub enabled: Vec<String>,
    pub disabled: Vec<String>,
}

impl McpApproval {
    /// Reads the approval keys of one settings file over what is known.
    pub fn read(&mut self, settings: &Value) {
        if let Some(all) = settings["enableAllProjectMcpServers"].as_bool() {
            self.all = all;
        }
        let names = |key: &str| -> Vec<String> {
            settings[key].as_array().into_iter().flatten().filter_map(|n| n.as_str().map(str::to_owned)).collect()
        };
        self.enabled.extend(names("enabledMcpjsonServers"));
        self.disabled.extend(names("disabledMcpjsonServers"));
    }

    /// A project's server may start when approved and not turned off.
    pub fn allows(&self, name: &str) -> bool {
        let named = |list: &[String]| list.iter().any(|known| known == name);
        !named(&self.disabled) && (self.all || named(&self.enabled))
    }
}

/// Fills `${VAR}` and `${VAR:-default}` in every string of a config. An
/// unknown variable without a default stays as written.
pub fn expand(value: Value, lookup: &dyn Fn(&str) -> Option<String>) -> Value {
    match value {
        Value::String(text) => Value::String(expand_text(&text, lookup)),
        Value::Array(items) => Value::Array(items.into_iter().map(|item| expand(item, lookup)).collect()),
        Value::Object(map) => Value::Object(map.into_iter().map(|(key, item)| (key, expand(item, lookup))).collect()),
        other => other,
    }
}

fn expand_text(text: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let inner = &after[..end];
        let (name, default) = match inner.split_once(":-") {
            Some((name, default)) => (name, Some(default)),
            None => (inner, None),
        };
        match lookup(name).or_else(|| default.map(str::to_owned)) {
            Some(found) => out.push_str(&found),
            None => out.push_str(&rest[start..start + 2 + end + 1]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_project_server_starts_only_when_the_owner_approved_it() {
        let mut approval = McpApproval::default();
        assert!(!approval.allows("jira"), "nothing approved yet");
        approval.read(&json!({"enabledMcpjsonServers": ["jira"]}));
        assert!(approval.allows("jira") && !approval.allows("sonar"));
        approval.read(&json!({"enableAllProjectMcpServers": true, "disabledMcpjsonServers": ["jira"]}));
        assert!(approval.allows("sonar"), "all approved");
        assert!(!approval.allows("jira"), "turned off wins");
    }

    #[test]
    fn variables_are_filled_in_with_defaults() {
        let lookup = |name: &str| (name == "TOKEN").then(|| "s3cret".to_owned());
        let config = json!({"headers": {"Authorization": "Bearer ${TOKEN}"},
            "args": ["${HOST:-sonar.local}", "${MISSING}", "${broken"], "port": 9});
        assert_eq!(
            expand(config, &lookup),
            json!({"headers": {"Authorization": "Bearer s3cret"},
                "args": ["sonar.local", "${MISSING}", "${broken"], "port": 9})
        );
    }

    #[test]
    fn the_platform_s_prefixes_are_not_free() {
        assert!(is_free_name("jira"));
        assert!(!is_free_name("superfik_memory") && !is_free_name("plugin_acme_jira"));
    }
}
