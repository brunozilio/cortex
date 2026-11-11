//! The channels each client opened (`superfik --channels telegram`): MCP
//! servers kept up while the client is, whose messages reach its inbox and
//! whose tools its runs reach through that same connection. The rules and
//! the bridge are `superfik_native::mcp::channel`'s; this keeps them per
//! client.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use superfik_core::mcp::LocalMcp;
use superfik_native::mcp::McpServer;
use superfik_native::mcp::channel::{self, Bridge};
use superfik_native::model::http::{Http, NetHttp};
use superfik_protocol::ServerEvent;

use crate::extensions::Extensions;

fn lock() -> std::sync::MutexGuard<'static, HashMap<u64, Vec<Arc<McpServer>>>> {
    static OPEN: OnceLock<Mutex<HashMap<u64, Vec<Arc<McpServer>>>>> = OnceLock::new();
    OPEN.get_or_init(Mutex::default).lock().unwrap_or_else(PoisonError::into_inner)
}

/// Opens the servers `names` a run in `path` could use as `client`'s
/// channels; answered with the channels open and why the others are not.
pub(crate) fn open(
    client: u64,
    path: &Path,
    names: &[String],
    extensions: &Extensions,
    home: Option<&Path>,
    http: &Arc<dyn Http>,
) -> ServerEvent {
    let request = crate::mcp_admin::request_in(path, extensions, home);
    let sources = match superfik_native::mcp::sources(&request) {
        Ok(sources) => sources,
        Err(error) => return ServerEvent::error(format!("Could not read the MCP servers: {error}")),
    };
    let specs = superfik_native::mcp::servers(&request, &sources);
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
    let opened = lock().get(&client).map(|open| open.iter().map(|s| s.name().to_owned()).collect()).unwrap_or_default();
    ServerEvent::Channels { opened, failed }
}

/// [`open`] over the network.
pub(crate) fn open_here(
    client: u64,
    path: &Path,
    names: &[String],
    extensions: &Extensions,
    home: Option<&Path>,
) -> ServerEvent {
    let http: Arc<dyn Http> = Arc::new(NetHttp);
    open(client, path, names, extensions, home, &http)
}

/// What a run of `client` reaches its open channels through.
pub(crate) fn for_run(client: u64) -> Vec<LocalMcp> {
    lock().get(&client).map(|open| open.iter().map(Bridge::local).collect()).unwrap_or_default()
}

/// Closes `client`'s channels (their processes end).
pub(crate) fn close(client: u64) {
    lock().remove(&client);
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::{Duration, Instant};

    use superfik_native::model::http::ScriptedHttp;

    use super::*;

    /// A channel server: it shakes hands, then pushes one message.
    const SERVER: &str = "read l; printf '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"capabilities\":{\"experimental\":{\"claude/channel\":{}}}}}\\n'; read l; printf '{\"jsonrpc\":\"2.0\",\"method\":\"notifications/claude/channel\",\"params\":{\"content\":\"hello from chat\"}}\\n'; sleep 30";

    #[test]
    fn a_client_s_channels_stay_open_reach_its_inbox_and_its_runs() {
        let home = tempfile::tempdir().expect("home");
        let project = tempfile::tempdir().expect("project");
        std::fs::create_dir_all(home.path().join(".superfik")).expect("dir");
        let config = serde_json::json!({ "mcpServers": { "chat": { "command": "/bin/sh", "args": ["-c", SERVER] } } });
        std::fs::write(home.path().join(".superfik/mcp.json"), config.to_string()).expect("config");
        let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
        let names = vec!["chat".to_owned(), "missing".to_owned()];
        let opened = open(9601, project.path(), &names, &Extensions::default(), Some(home.path()), &http);
        let ServerEvent::Channels { opened, failed } = opened else { panic!("{opened:?}") };
        assert_eq!(opened, ["chat"]);
        assert!(failed.len() == 1 && failed[0].starts_with("missing:"), "{failed:?}");

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut notices = Vec::new();
        while notices.is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
            notices = superfik_native::tools::inbox::take(9601);
        }
        assert_eq!(notices[0].text, "hello from chat");
        assert_eq!(for_run(9601).iter().map(|local| local.name.as_str()).collect::<Vec<_>>(), ["chat"]);
        let again = open(9601, project.path(), &names[..1], &Extensions::default(), Some(home.path()), &http);
        assert!(matches!(again, ServerEvent::Channels { ref opened, .. } if opened.len() == 1), "opened once");
        close(9601);
        assert!(for_run(9601).is_empty());
    }
}
