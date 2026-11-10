//! `/reload`: the engine rereads `~/.superfik` and the project on every
//! message, so reloading means refreshing what the chat keeps (the skills
//! it completes, the settings and models) and saying what is there now.

use superfik_sdk::{CustomAgent, McpOrigin, McpServerInfo, McpState};

/// What a reload found: the plugins (named by their skills, agents and
/// servers), how many skills and agents, and the MCP servers' states.
pub fn summary(skills: &[String], agents: &[CustomAgent], servers: &[McpServerInfo]) -> String {
    let namespace = |name: &str| name.split_once(':').map(|(plugin, _)| plugin.to_owned());
    let mut plugins: Vec<String> = skills
        .iter()
        .map(String::as_str)
        .chain(agents.iter().map(|agent| agent.name.as_str()))
        .filter_map(namespace)
        .chain(servers.iter().filter_map(|server| match &server.origin {
            McpOrigin::Plugin(plugin) => Some(plugin.clone()),
            _ => None,
        }))
        .collect();
    plugins.sort();
    plugins.dedup();
    let count = |state: McpState| servers.iter().filter(|server| server.state == state).count();
    let mut mcp = format!("{} ready", count(McpState::Ready));
    let waiting = count(McpState::Pending);
    if waiting > 0 {
        mcp.push_str(&format!(" · {waiting} waiting for approval (/mcp)"));
    }
    if count(McpState::Off) > 0 {
        mcp.push_str(&format!(" · {} off", count(McpState::Off)));
    }
    let plugins =
        if plugins.is_empty() { "none (put them in ~/.superfik/plugins/)".to_owned() } else { plugins.join(", ") };
    format!(
        "Reloaded ~/.superfik and the project:\n  plugins  {plugins}\n  skills   {}\n  agents   {}\n  MCP      {mcp}",
        skills.len(),
        agents.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reload_names_the_plugins_and_counts_what_they_bring() {
        let skills = ["acme:triage".to_owned(), "acme:fix".to_owned(), "verify".to_owned()];
        let agents = [CustomAgent { name: "beta:reviewer".into(), description: String::new() }];
        let server = |name: &str, origin: McpOrigin, state: McpState| McpServerInfo {
            name: name.into(),
            origin,
            target: String::new(),
            state,
        };
        let servers = [
            server("plugin_gamma_dd", McpOrigin::Plugin("gamma".into()), McpState::Ready),
            server("jira", McpOrigin::Project, McpState::Pending),
        ];
        assert_eq!(
            summary(&skills, &agents, &servers),
            "Reloaded ~/.superfik and the project:\n  plugins  acme, beta, gamma\n  skills   3\n  agents   1\n  MCP      1 ready · 1 waiting for approval (/mcp)"
        );
        assert!(summary(&[], &[], &[]).contains("plugins  none (put them in ~/.superfik/plugins/)"));
    }
}
