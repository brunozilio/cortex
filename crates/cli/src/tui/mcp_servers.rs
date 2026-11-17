use domain::command::McpCommand;

use super::app::{App, Msg};

pub fn run(app: &mut App, change: McpCommand) {
    let cwd = app.cwd.clone();
    if let McpCommand::Login(name) = &change {
        app.transcript.notice(format!("Signing in to {name} in the browser…"));
    }
    app.spawn(move |client| {
        let servers = match &change {
            McpCommand::List => client.mcp_servers(&cwd),
            McpCommand::Approve(name) => client.approve_mcp_server(&cwd, name, true),
            McpCommand::Deny(name) => client.approve_mcp_server(&cwd, name, false),
            McpCommand::Login(name) => client.sign_in_mcp_server(&cwd, name, crate::login::open),
            McpCommand::Logout(name) => client.sign_out_mcp_server(&cwd, name),
        };
        match servers {
            Ok(servers) => {
                let shown =
                    crate::mcp::render(&servers).replace(concat!(brand::command!(), " mcp approve"), "/mcp approve");
                let note = if change == McpCommand::List { "" } else { "\nThe change applies from the next message." };
                Msg::Notice(format!("MCP servers:\n{}{note}", shown.trim_end()))
            }
            Err(error) => Msg::Error(error.to_string()),
        }
    });
}
