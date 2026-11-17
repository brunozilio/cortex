#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use domain::{Extensions, ModelOption, PromptRequest};

use super::McpPool;
use crate::mcp::{McpServer, McpSources};
use crate::model::http::{Http, NetHttp};

const SERVER: &str = r#"echo $$ >> "$1"
while IFS= read -r line; do
id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
case "$line" in
*'"method":"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{}}\n' "$id";;
*'"method":"tools/list"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"ping","inputSchema":{"type":"object"}}]}}\n' "$id";;
esac
done"#;

struct Fixture {
    dir: tempfile::TempDir,
    pool: McpPool,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("server.sh"), SERVER).expect("server");
        Self { dir, pool: McpPool::default() }
    }

    fn log(&self) -> PathBuf {
        self.dir.path().join("started")
    }

    fn request(&self, servers: &[(&str, &str)]) -> PromptRequest {
        let entries: serde_json::Map<String, serde_json::Value> = servers
            .iter()
            .map(|(name, flavour)| {
                let args = [self.dir.path().join("server.sh").display().to_string(), self.log().display().to_string()];
                let args: Vec<String> = args.into_iter().chain([(*flavour).to_owned()]).collect();
                ((*name).to_owned(), serde_json::json!({"command": "/bin/sh", "args": args}))
            })
            .collect();
        let config = self.dir.path().join("mcp.json");
        std::fs::write(&config, serde_json::json!({ "mcpServers": entries }).to_string()).expect("config");
        let mut request = PromptRequest::plain(self.dir.path().into(), "hi", ModelOption::new("m", "M"));
        request.host.extensions = Some(Extensions { mcp_config: Some(config), ..Extensions::default() });
        request
    }

    fn connect(&self, request: &PromptRequest) -> Vec<Arc<McpServer>> {
        let http: Arc<dyn Http> = Arc::new(NetHttp);
        self.pool.connect(request, &McpSources::default(), &http, &platform::Stop::new(), &Arc::default())
    }

    fn started(&self) -> Vec<String> {
        std::fs::read_to_string(self.log()).unwrap_or_default().lines().map(str::to_owned).collect()
    }
}

fn running(pid: &str) -> bool {
    std::process::Command::new("kill").args(["-0", pid]).output().is_ok_and(|done| done.status.success())
}

fn eventually(what: impl Fn() -> bool) -> bool {
    let until = Instant::now() + Duration::from_secs(5);
    while Instant::now() < until {
        if what() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn named(servers: &[Arc<McpServer>]) -> Vec<&str> {
    servers.iter().map(|server| server.name()).collect()
}

#[test]
fn a_server_stays_connected_across_connects_with_an_unchanged_config() {
    let fixture = Fixture::new();
    let request = fixture.request(&[("echo", "a")]);

    let first = fixture.connect(&request);
    let second = fixture.connect(&request);

    assert_eq!(named(&second), ["echo"]);
    assert!(Arc::ptr_eq(&first[0], &second[0]), "the same connection");
    assert_eq!(fixture.started().len(), 1, "started once");
    let listed = second[0].tools().expect("the kept connection still answers");
    assert_eq!(listed.len(), 1);
}

#[test]
fn a_changed_entry_restarts_its_server_and_closes_the_old_one() {
    let fixture = Fixture::new();
    fixture.connect(&fixture.request(&[("echo", "a")]));
    let old = fixture.started()[0].clone();

    let again = fixture.connect(&fixture.request(&[("echo", "b")]));

    assert_eq!(named(&again), ["echo"]);
    assert_eq!(fixture.started().len(), 2, "started again for the new entry");
    assert!(!running(&old), "the old process was waited for");
}

#[test]
fn a_server_no_longer_configured_or_approved_is_closed() {
    let fixture = Fixture::new();
    fixture.connect(&fixture.request(&[("echo", "a"), ("other", "a")]));
    let pids = fixture.started();
    assert_eq!(pids.len(), 2);

    let left = fixture.connect(&fixture.request(&[("other", "a")]));

    assert_eq!(named(&left), ["other"]);
    assert_eq!(fixture.pool.len(), 1);
    assert_eq!(pids.iter().filter(|pid| running(pid)).count(), 1, "one of the two was closed: {pids:?}");
}

#[test]
fn a_server_whose_process_died_is_started_again() {
    let fixture = Fixture::new();
    let request = fixture.request(&[("echo", "a")]);
    let first = fixture.connect(&request);
    let pid = fixture.started()[0].clone();
    std::process::Command::new("kill").args(["-9", &pid]).output().expect("kill");
    assert!(eventually(|| !first[0].alive()), "the death is seen");

    let again = fixture.connect(&request);

    assert!(!Arc::ptr_eq(&first[0], &again[0]));
    assert_eq!(fixture.started().len(), 2);
    assert!(again[0].alive());
}

#[test]
fn closing_the_pool_ends_every_server_it_kept() {
    let fixture = Fixture::new();
    fixture.connect(&fixture.request(&[("echo", "a")]));
    let pid = fixture.started()[0].clone();
    assert!(running(&pid));

    fixture.pool.close();

    assert!(fixture.pool.is_empty());
    assert!(!running(&pid), "terminated and waited for");
}

#[test]
fn nothing_is_started_for_a_project_without_servers() {
    let fixture = Fixture::new();
    assert!(fixture.connect(&fixture.request(&[])).is_empty());
    assert!(fixture.pool.is_empty());
    assert!(!Path::new(&fixture.log()).exists());
}
