//! Rules for MCP servers a person or a project configures
//! (`.{command}/mcp.json`): which of a project's servers may start, and how
//! `${VAR}` and `${VAR:-default}` in a config are filled in.

mod edit;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use self::edit::{McpOrigin, McpServerInfo, McpState, add_server, remove_server, server_from, set_approval};

/// Names the platform keeps for itself: the app's own servers and plugins'.
pub const RESERVED_PREFIXES: [&str; 2] = [concat!(brand::command!(), "_"), "plugin_"];

/// Whether a name is free for a person's or a project's server.
pub fn is_free_name(name: &str) -> bool {
    !RESERVED_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

/// What the owner said about a project's `.{command}/mcp.json` servers
/// (`enableAllProjectMcpServers`, `enabledMcpServers`,
/// `disabledMcpServers`). A repository cannot say it for itself.
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
        self.enabled.extend(names("enabledMcpServers"));
        self.disabled.extend(names("disabledMcpServers"));
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
        approval.read(&json!({"enabledMcpServers": ["jira"]}));
        assert!(approval.allows("jira") && !approval.allows("sonar"));
        approval.read(&json!({"enableAllProjectMcpServers": true, "disabledMcpServers": ["jira"]}));
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
    fn approving_and_turning_off_rewrite_only_the_owner_s_lists() {
        let settings = json!({"model": "opus", "enabledMcpServers": ["sonar"], "disabledMcpServers": ["jira"]});
        let approved = set_approval(settings, "jira", true);
        assert_eq!(
            approved,
            json!({"model": "opus", "enabledMcpServers": ["sonar", "jira"], "disabledMcpServers": []})
        );
        let off = set_approval(approved, "sonar", false);
        assert_eq!(off["enabledMcpServers"], json!(["jira"]));
        assert_eq!(off["disabledMcpServers"], json!(["sonar"]));
        assert_eq!(set_approval(json!(null), "x", true), json!({"enabledMcpServers": ["x"], "disabledMcpServers": []}));
    }

    #[test]
    fn a_person_adds_and_removes_their_own_servers() {
        let server = json!({"type": "http", "url": "https://mcp.atlassian.com/v1/mcp"});
        let config = add_server(json!(null), "atlassian", server.clone()).expect("added");
        assert_eq!(config, json!({"mcpServers": {"atlassian": server}}));
        assert!(add_server(config.clone(), "plugin_x", json!({"command": "x"})).is_err(), "reserved");
        assert!(add_server(config.clone(), "bad name", json!({"command": "x"})).is_err(), "not a tool-safe name");
        assert!(add_server(config.clone(), "x", json!({"type": "websocket"})).is_err(), "neither stdio nor http");
        let (left, removed) = remove_server(config, "atlassian");
        assert!(removed);
        assert_eq!(left, json!({"mcpServers": {}}));
        assert!(!remove_server(left, "atlassian").1);
    }

    #[test]
    fn a_server_s_state_says_why_it_does_or_does_not_start() {
        let mut approval = McpApproval::default();
        approval.read(&json!({"enabledMcpServers": ["jira"], "disabledMcpServers": ["sonar"]}));
        assert_eq!(McpState::of(&McpOrigin::Project, "jira", &approval), McpState::Ready);
        assert_eq!(McpState::of(&McpOrigin::Project, "sonar", &approval), McpState::Off);
        assert_eq!(McpState::of(&McpOrigin::Project, "new", &approval), McpState::Pending);
        assert_eq!(McpState::of(&McpOrigin::User, "new", &approval), McpState::Ready);
    }

    #[test]
    fn a_url_is_an_http_server_and_anything_else_a_command() {
        let pair = |k: &str, v: &str| (k.to_owned(), v.to_owned());
        assert_eq!(
            server_from("https://mcp.atlassian.com/v1/mcp", &[], &[], &[pair("Authorization", "Bearer ${TOKEN}")]),
            json!({"type": "http", "url": "https://mcp.atlassian.com/v1/mcp", "headers": {"Authorization": "Bearer ${TOKEN}"}})
        );
        assert_eq!(
            server_from("npx", &["-y".into(), "sonar-mcp".into()], &[pair("SONAR_TOKEN", "${SONAR_TOKEN}")], &[]),
            json!({"command": "npx", "args": ["-y", "sonar-mcp"], "env": {"SONAR_TOKEN": "${SONAR_TOKEN}"}})
        );
        assert_eq!(server_from("jira-mcp", &[], &[], &[]), json!({"command": "jira-mcp"}));
    }

    #[test]
    fn the_platform_s_prefixes_are_not_free() {
        assert!(is_free_name("jira"));
        assert!(!is_free_name(concat!(brand::command!(), "_memory")) && !is_free_name("plugin_acme_jira"));
    }
}
