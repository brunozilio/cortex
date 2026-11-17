use app::{Client, McpEntry, McpListing, McpOrigin, McpServerInfo, McpState};
use clap::Subcommand;

const ADD_EXAMPLES: &str = concat!(
    "The options may come anywhere before `--`. Write `--` before the server's own arguments \
     when any of them starts with `-`:\n\n  ",
    brand::command!(),
    " mcp add -e GITHUB_TOKEN='${GITHUB_TOKEN}' github -- npx -y @modelcontextprotocol/server-github\n  ",
    brand::command!(),
    " mcp add jira jira-mcp -- --stdio"
);

#[derive(Debug, Subcommand)]
pub enum McpAction {
    #[command(about = "List the servers, where they come from and whether they start")]
    List,
    #[command(about = "Show one server's entry, with its secrets hidden")]
    Get { name: String },
    #[command(about = "Add one of your own servers: a URL (HTTP) or a command and its arguments")]
    #[command(after_help = ADD_EXAMPLES)]
    Add {
        name: String,
        #[arg(help = "`https://…` for an HTTP server, else the command to start")]
        target: String,
        #[arg(help = "The command's arguments; write `--` before them when one starts with `-`")]
        args: Vec<String>,
        #[arg(help = "Replace a server of yours that has this name")]
        #[arg(long)]
        force: bool,
        #[arg(help = "An environment variable for a command: KEY=VALUE (`${VAR}` is read at start)")]
        #[arg(short, long = "env", value_parser = parse_pair('='))]
        env: Vec<(String, String)>,
        #[arg(help = "A header for an HTTP server: \"Name: value\"")]
        #[arg(short = 'H', long = "header", value_parser = parse_pair(':'))]
        headers: Vec<(String, String)>,
        #[arg(help = "`sse` for a server on the older HTTP+SSE transport")]
        #[arg(long, default_value = "http")]
        transport: String,
    },
    #[command(about = "Remove one of your own servers")]
    Remove { name: String },
    #[command(about = concat!("Let a server of this project's ", brand::dir!(), "/mcp.json start"))]
    Approve { name: String },
    #[command(about = concat!("Turn a server of this project's ", brand::dir!(), "/mcp.json off"))]
    Deny { name: String },
    #[command(about = "Sign in to a remote server in the browser (OAuth)")]
    Login { name: String },
    #[command(about = "Forget the sign-in to a remote server")]
    Logout { name: String },
}

pub fn run(client: &Client, cwd: &std::path::Path, action: McpAction) -> Result<(), String> {
    let mut added_name = None;
    let servers = match action {
        McpAction::List => return list(&client.mcp_listing(cwd).map_err(|error| error.to_string())?),
        McpAction::Get { name } => {
            print!("{}", shown(&client.mcp_server(cwd, &name).map_err(|error| error.to_string())?));
            return Ok(());
        }
        McpAction::Add { name, target, args, force, env, headers, transport } => {
            let mut server = domain::mcp_config::server_from(&target, &args, &env, &headers);
            if transport == "sse" && server.get("url").is_some() {
                server["type"] = "sse".into();
            }
            let result = client.add_mcp_server(cwd, &name, server, force);
            added_name = Some(name);
            result
        }
        McpAction::Remove { name } => client.remove_mcp_server(cwd, &name),
        McpAction::Approve { name } => client.approve_mcp_server(cwd, &name, true),
        McpAction::Deny { name } => client.approve_mcp_server(cwd, &name, false),
        McpAction::Login { name } => client.sign_in_mcp_server(cwd, &name, |url| {
            eprintln!("Opening the sign-in page: {url}");
            crate::login::open(url);
        }),
        McpAction::Logout { name } => client.sign_out_mcp_server(cwd, &name),
    }
    .map_err(|error| error.to_string())?;
    match added_name {
        Some(name) => print!("{}", added(&name, &servers)),
        None => print!("{}", render(&servers)),
    }
    Ok(())
}

fn list(listing: &McpListing) -> Result<(), String> {
    for warning in listing.warnings() {
        eprintln!("warning: {}", warning.line());
    }
    if !listing.owner_s_file_is_broken() {
        print!("{}", render(&listing.servers));
        return Ok(());
    }
    if !listing.servers.is_empty() {
        print!("{}", render(&listing.servers));
    }
    Err(listing.errors().map(app::McpUnreadable::line).collect::<Vec<_>>().join("\n"))
}

fn shown(entry: &McpEntry) -> String {
    let info = &entry.info;
    let mut out = format!(
        "{}\n  origin: {}\n  state:  {}\n  target: {}\n",
        info.name,
        origin(&info.origin),
        state(info.state),
        info.target
    );
    if let Some(file) = &entry.file {
        out.push_str(&format!("  file:   {}\n", file.display()));
    }
    if let Some(stored) = &entry.entry {
        let pretty = serde_json::to_string_pretty(stored).unwrap_or_else(|_| stored.to_string());
        out.push_str(&format!("{pretty}\n"));
    }
    out
}

fn added(name: &str, servers: &[McpServerInfo]) -> String {
    match servers.iter().find(|server| server.name == name) {
        Some(server) => {
            format!("Added {name} ({}, {}): {}\n", origin(&server.origin), state(server.state), server.target)
        }
        None => render(servers),
    }
}

fn origin(origin: &McpOrigin) -> String {
    match origin {
        McpOrigin::Host => "host".to_owned(),
        McpOrigin::Project => "project".to_owned(),
        McpOrigin::User => "user".to_owned(),
        McpOrigin::Plugin(plugin) => format!("plugin {plugin}"),
    }
}

fn state(state: McpState) -> &'static str {
    match state {
        McpState::Ready => "ready",
        McpState::Pending => "waiting for approval",
        McpState::Off => "off",
        McpState::Untrusted => "skipped: folder not trusted",
    }
}

pub fn render(servers: &[McpServerInfo]) -> String {
    if servers.is_empty() {
        return concat!(
            "No MCP servers. Add one with `",
            brand::command!(),
            " mcp add <name> <url|command>`, or a project's ",
            brand::dir!(),
            "/mcp.json.\n"
        )
        .into();
    }
    let width = servers.iter().map(|server| server.name.len()).max().unwrap_or(0);
    let states = servers.iter().map(|server| state(server.state).len() + 1).max().unwrap_or(0).max(21);
    let mut out = String::new();
    for server in servers {
        let (origin, state) = (origin(&server.origin), state(server.state));
        out.push_str(&format!("{:<width$}  {origin:<14} {state:<states$} {}\n", server.name, server.target));
    }
    let waiting: Vec<&str> =
        servers.iter().filter(|server| server.state == McpState::Pending).map(|server| server.name.as_str()).collect();
    if !waiting.is_empty() {
        out.push_str(&format!(
            "\nApprove with `{brand_command} mcp approve <name>`: {}\n",
            waiting.join(", "),
            brand_command = brand::COMMAND
        ));
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
            concat!(
                "jira            project        waiting for approval  jira-mcp\n\
             plugin_acme_dd  plugin acme    ready                 dd-mcp\n\
             \nApprove with `",
                brand::command!(),
                " mcp approve <name>`: jira\n"
            )
        );
        assert!(render(&[]).starts_with("No MCP servers."));
    }

    #[test]
    fn adding_a_server_confirms_that_one_server_only() {
        let servers = [
            server("cloudflare", McpOrigin::User, "https://mcp.cloudflare.com/mcp", McpState::Ready),
            server("jira", McpOrigin::User, "https://mcp.atlassian.com/v2/mcp", McpState::Ready),
        ];
        assert_eq!(added("jira", &servers), "Added jira (user, ready): https://mcp.atlassian.com/v2/mcp\n");
        assert_eq!(added("gone", &servers), render(&servers), "an unexpected list is shown whole");
    }

    #[test]
    fn a_skipped_server_widens_the_state_column() {
        let shown = render(&[
            server("jira", McpOrigin::Project, "jira-mcp", McpState::Untrusted),
            server("docs", McpOrigin::User, "https://d.test", McpState::Ready),
        ]);
        assert_eq!(
            shown,
            "jira  project        skipped: folder not trusted  jira-mcp\n\
             docs  user           ready                        https://d.test\n"
        );
    }

    #[test]
    fn one_server_is_shown_with_its_file_and_entry() {
        let entry = McpEntry {
            info: server("docs", McpOrigin::User, "https://d.test", McpState::Ready),
            file: Some("/h/mcp.json".into()),
            entry: Some(serde_json::json!({"url": "https://d.test"})),
        };
        assert_eq!(
            shown(&entry),
            "docs\n  origin: user\n  state:  ready\n  target: https://d.test\n  file:   /h/mcp.json\n\
             {\n  \"url\": \"https://d.test\"\n}\n"
        );
    }

    #[test]
    fn pairs_split_once_and_trim() {
        assert_eq!(parse_pair(':')("Authorization: Bearer a:b"), Ok(("Authorization".into(), "Bearer a:b".into())));
        assert!(parse_pair('=')("NOPE").is_err());
    }
}
