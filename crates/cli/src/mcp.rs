//! `superfik mcp`: the MCP servers a session in this folder can use, and
//! the owner's say over them. Presentation only; the rules are
//! `domain::mcp_config`'s and the files the backend's.

use clap::Subcommand;
use sdk::{McpOrigin, McpServerInfo, McpState, Superfik};

#[derive(Debug, Subcommand)]
pub enum McpAction {
    /// List the servers, where they come from and whether they start.
    List,
    /// Add one of your own servers: a URL (HTTP) or a command and its arguments.
    Add {
        name: String,
        /// `https://…` for an HTTP server, else the command to start.
        target: String,
        /// The command's arguments.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
        /// An environment variable for a command: KEY=VALUE (`${VAR}` is read at start).
        #[arg(short, long = "env", value_parser = parse_pair('='))]
        env: Vec<(String, String)>,
        /// A header for an HTTP server: "Name: value".
        #[arg(short = 'H', long = "header", value_parser = parse_pair(':'))]
        headers: Vec<(String, String)>,
        /// `sse` for a server on the older HTTP+SSE transport.
        #[arg(long, default_value = "http")]
        transport: String,
    },
    /// Remove one of your own servers.
    Remove { name: String },
    /// Let a server of this project's .mcp.json start.
    Approve { name: String },
    /// Turn a server of this project's .mcp.json off.
    Deny { name: String },
    /// Sign in to a remote server in the browser (OAuth).
    Login { name: String },
    /// Forget the sign-in to a remote server.
    Logout { name: String },
}

pub fn run(superfik: &Superfik, cwd: &std::path::Path, action: McpAction) -> Result<(), String> {
    let servers = match action {
        McpAction::List => superfik.mcp_servers(cwd),
        McpAction::Add { name, target, args, env, headers, transport } => {
            let mut server = domain::mcp_config::server_from(&target, &args, &env, &headers);
            if transport == "sse" && server.get("url").is_some() {
                server["type"] = "sse".into();
            }
            superfik.add_mcp_server(cwd, &name, server)
        }
        McpAction::Remove { name } => superfik.remove_mcp_server(cwd, &name),
        McpAction::Approve { name } => superfik.approve_mcp_server(cwd, &name, true),
        McpAction::Deny { name } => superfik.approve_mcp_server(cwd, &name, false),
        McpAction::Login { name } => superfik.sign_in_mcp_server(cwd, &name, |url| {
            eprintln!("Opening the sign-in page: {url}");
            crate::login::open(url);
        }),
        McpAction::Logout { name } => superfik.sign_out_mcp_server(cwd, &name),
    }
    .map_err(|error| error.to_string())?;
    print!("{}", render(&servers));
    Ok(())
}

/// The servers as a table, and how to approve the ones waiting.
pub fn render(servers: &[McpServerInfo]) -> String {
    if servers.is_empty() {
        return "No MCP servers. Add one with `superfik mcp add <name> <url|command>`, or a project's .mcp.json.\n"
            .into();
    }
    let width = servers.iter().map(|server| server.name.len()).max().unwrap_or(0);
    let mut out = String::new();
    for server in servers {
        let origin = match &server.origin {
            McpOrigin::Host => "host".to_owned(),
            McpOrigin::Project => "project".to_owned(),
            McpOrigin::User => "user".to_owned(),
            McpOrigin::Plugin(plugin) => format!("plugin {plugin}"),
        };
        let state = match server.state {
            McpState::Ready => "ready",
            McpState::Pending => "waiting for approval",
            McpState::Off => "off",
        };
        out.push_str(&format!("{:<width$}  {origin:<14} {state:<21} {}\n", server.name, server.target));
    }
    let waiting: Vec<&str> =
        servers.iter().filter(|server| server.state == McpState::Pending).map(|server| server.name.as_str()).collect();
    if !waiting.is_empty() {
        out.push_str(&format!("\nApprove with `superfik mcp approve <name>`: {}\n", waiting.join(", ")));
    }
    out
}

fn parse_pair(separator: char) -> impl Fn(&str) -> Result<(String, String), String> + Clone {
    move |text: &str| {
        let (key, value) = text.split_once(separator).ok_or_else(|| format!("`{text}` needs a `{separator}`"))?;
        Ok((key.trim().to_owned(), value.trim().to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(name: &str, origin: McpOrigin, target: &str, state: McpState) -> McpServerInfo {
        McpServerInfo { name: name.into(), origin, target: target.into(), state }
    }

    #[test]
    fn the_list_shows_each_server_and_what_waits_for_approval() {
        let shown = render(&[
            server("jira", McpOrigin::Project, "jira-mcp", McpState::Pending),
            server("plugin_acme_dd", McpOrigin::Plugin("acme".into()), "dd-mcp", McpState::Ready),
        ]);
        assert_eq!(
            shown,
            "jira            project        waiting for approval  jira-mcp\n\
             plugin_acme_dd  plugin acme    ready                 dd-mcp\n\
             \nApprove with `superfik mcp approve <name>`: jira\n"
        );
        assert!(render(&[]).starts_with("No MCP servers."));
    }

    #[test]
    fn pairs_split_once_and_trim() {
        assert_eq!(parse_pair(':')("Authorization: Bearer a:b"), Ok(("Authorization".into(), "Bearer a:b".into())));
        assert!(parse_pair('=')("NOPE").is_err());
    }
}
