use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use platform::sync::Locked;

use crate::api::features::channels::{ChannelsReply, ChannelsRequest};
use agent_engine::mcp::McpServer;
use agent_engine::mcp::channel::{self, Bridge};
use agent_engine::model::http::{Http, NetHttp};
use domain::mcp::LocalMcp;

use crate::extensions::Extensions;
use crate::features::context::Context;

pub mod run;

pub fn handle(context: &Context<'_>, request: ChannelsRequest) -> Result<ChannelsReply, String> {
    let ChannelsRequest { path, names } = request;
    let services = context.services;
    let http: Arc<dyn Http> = Arc::new(NetHttp);
    open(context.client, &path, &names, &services.extensions, services.home().as_deref(), &http)
}

pub fn forget(client: u64) {
    lock().remove(&client);
}

fn lock() -> std::sync::MutexGuard<'static, HashMap<u64, Vec<Arc<McpServer>>>> {
    static OPEN: OnceLock<Mutex<HashMap<u64, Vec<Arc<McpServer>>>>> = OnceLock::new();
    OPEN.get_or_init(Mutex::default).locked()
}

fn open(
    client: u64,
    path: &Path,
    names: &[String],
    extensions: &Extensions,
    home: Option<&Path>,
    http: &Arc<dyn Http>,
) -> Result<ChannelsReply, String> {
    let host = extensions.host(path, home);
    let folder = domain::Folder::at(path, &host);
    let sources = match agent_engine::mcp::sources(folder) {
        Ok(sources) => sources,
        Err(error) => return Err(format!("Could not read the MCP servers: {error}")),
    };
    let specs = agent_engine::mcp::servers(folder, &sources);
    let mut failed = Vec::new();
    for name in names {
        if lock().get(&client).is_some_and(|open| open.iter().any(|server| server.name() == name)) {
            continue;
        }
        let Some(spec) = specs.iter().find(|spec| spec.name() == name) else {
            failed.push(format!("{name}: no such MCP server here (or it waits for approval)"));
            continue;
        };
        match channel::open(spec, http, client) {
            Ok(server) => lock().entry(client).or_default().push(server),
            Err(error) => failed.push(format!("{name}: {error}")),
        }
    }
    let opened: Vec<String> =
        lock().get(&client).map(|open| open.iter().map(|s| s.name().to_owned()).collect()).unwrap_or_default();
    Ok(ChannelsReply { opened, failed })
}

fn for_run(client: u64) -> Vec<LocalMcp> {
    lock().get(&client).map(|open| open.iter().map(Bridge::local).collect()).unwrap_or_default()
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::{Duration, Instant};

    use agent_engine::model::http::ScriptedHttp;

    use super::*;

    const SERVER: &str = "read l; printf '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"capabilities\":{\"experimental\":{\"claude/channel\":{}}}}}\\n'; read l; printf '{\"jsonrpc\":\"2.0\",\"method\":\"notifications/claude/channel\",\"params\":{\"content\":\"hello from chat\"}}\\n'; sleep 30";

    #[test]
    fn a_client_s_channels_stay_open_reach_its_inbox_and_its_runs() {
        let home = tempfile::tempdir().expect("home");
        let project = tempfile::tempdir().expect("project");
        std::fs::create_dir_all(home.path().join(brand::DIR)).expect("dir");
        let config = serde_json::json!({ "mcpServers": { "chat": { "command": "/bin/sh", "args": ["-c", SERVER] } } });
        std::fs::write(home.path().join(concat!(brand::dir!(), "/mcp.json")), config.to_string()).expect("config");
        let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
        let names = vec!["chat".to_owned(), "missing".to_owned()];
        let opened =
            open(9601, project.path(), &names, &Extensions::default(), Some(home.path()), &http).expect("open");
        assert_eq!(opened.opened, ["chat"]);
        assert!(opened.failed[0].starts_with("missing:"), "{opened:?}");

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut notices = Vec::new();
        while notices.is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
            notices = agent_engine::tools::inbox::take(9601);
        }
        assert_eq!(notices[0].text, "hello from chat");
        assert_eq!(for_run(9601).iter().map(|local| local.name.as_str()).collect::<Vec<_>>(), ["chat"]);
        let again = open(9601, project.path(), &names[..1], &Extensions::default(), Some(home.path()), &http);
        assert_eq!(again.expect("again").opened, ["chat"], "opened once");
        forget(9601);
        assert!(for_run(9601).is_empty());
    }
}
