use sdk::{CustomAgent, McpOrigin, McpServerInfo, McpState};

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
    let plugins = if plugins.is_empty() {
        concat!("none (put them in ~/", brand::dir!(), "/plugins/)").to_owned()
    } else {
        plugins.join(", ")
    };
    format!(
        "Reloaded ~/{brand_dir} and the project:\n  plugins  {plugins}\n  skills   {}\n  agents   {}\n  MCP      {mcp}",
        skills.len(),
        agents.len(),
        brand_dir = brand::DIR
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
            concat!(
                "Reloaded ~/",
                brand::dir!(),
                " and the project:\n  plugins  acme, beta, gamma\n  skills   3\n  agents   1\n  MCP      1 ready · 1 waiting for approval (/mcp)"
            )
        );
        assert!(summary(&[], &[], &[]).contains(concat!("plugins  none (put them in ~/", brand::dir!(), "/plugins/)")));
    }
}
