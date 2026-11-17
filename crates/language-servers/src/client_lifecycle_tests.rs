use std::path::PathBuf;

use crossbeam_channel::unbounded as channel;

use domain::lsp::uri::uri_of;

use super::*;
use crate::documents::{MOST_OPEN, MOST_PUBLISHED};

#[test]
fn a_client_that_is_let_go_asks_its_server_to_shut_down_and_then_to_exit() {
    let (said, heard) = channel();
    let (client, server) = connected(move |mut server| {
        server.handshake();
        let shutdown = server.next();
        assert_eq!(shutdown["method"], "shutdown");
        assert!(shutdown.get("params").is_none(), "{shutdown}");
        server.send(json!({"jsonrpc": "2.0", "id": shutdown["id"], "result": null}));
        let exit = server.next();
        said.send(exit["method"].as_str().map(str::to_owned)).expect("said");
    });
    drop(client);
    assert_eq!(heard.recv_timeout(WAIT).expect("heard").as_deref(), Some("exit"));
    server.join().expect("server");
}

#[test]
fn the_document_used_longest_ago_is_closed_when_too_many_are_open() {
    let file = |at: usize| PathBuf::from(format!("/w/src/f{at}.rs"));
    let (closed, heard) = channel();
    let (client, server) = connected(move |mut server| {
        server.handshake();
        loop {
            let message = server.next();
            if message["method"] == "textDocument/didClose" {
                closed.send(message["params"]["textDocument"]["uri"].as_str().map(str::to_owned)).expect("closed");
                assert_eq!(server.next()["method"], "textDocument/didOpen");
                return;
            }
        }
    });
    for at in 0..MOST_OPEN {
        client.sync(&file(at), "rust", "").expect("open");
    }
    assert_eq!(client.sync(&file(0), "rust", "fn a() {}").expect("change"), 2, "the first is used again");
    client.sync(&file(MOST_OPEN), "rust", "").expect("one too many");
    assert_eq!(heard.recv_timeout(WAIT).expect("heard"), Some(uri_of(&file(1))));
    assert_eq!(client.shared.documents.lock().expect("documents").len(), MOST_OPEN);
    server.join().expect("server");
    let deadline = std::time::Instant::now() + WAIT;
    while client.is_alive() && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(client.sync(&file(0), "rust", "").ok(), None, "the server is gone");
}

#[test]
fn diagnostics_for_files_nobody_opened_are_not_kept_without_bound() {
    let (client, server) = connected(|mut server| {
        server.handshake();
        let opened = server.next();
        let mine = opened["params"]["textDocument"]["uri"].clone();
        let publish = |uri: Value| {
            json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
            "params": {"uri": uri, "version": 1, "diagnostics": [{"message": "unused"}]}})
        };
        server.send(publish(mine));
        for other in 0..MOST_PUBLISHED * 2 {
            server.send(publish(json!(format!("file:///w/src/other{other}.rs"))));
        }
        server.send(json!({"jsonrpc": "2.0", "method": "window/logMessage", "params": {"message": "done"}}));
        let asked = server.next();
        server.send(json!({"jsonrpc": "2.0", "id": asked["id"], "result": null}));
    });
    let file = Path::new("/w/src/mine.rs");
    client.sync(file, "rust", "fn a() {}").expect("open");
    client.request("workspace/symbol", json!({}), &AtomicBool::new(false), WAIT).expect("after every publish");
    assert_eq!(client.shared.published.lock().expect("published").len(), MOST_PUBLISHED);
    assert_eq!(client.diagnostics(file, 1, WAIT), Some(vec![json!({"message": "unused"})]), "the open file's stay");
    server.join().expect("server");
}

#[cfg(unix)]
mod processes {
    use std::time::Instant;

    use domain::lsp::ServerSpec;

    use super::*;

    const SERVER: &str = r#"
answer() { printf 'Content-Length: %s\r\n\r\n%s' "${#1}" "$1"; }
while IFS= read -r header; do
  length=$(printf '%s' "${header#Content-Length: }" | tr -d '\r')
  IFS= read -r blank
  body=$(dd bs=1 count="$length" 2>/dev/null)
  case "$body" in
    *'"method":"initialize"'*) answer '{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}';;
    *'"method":"shutdown"'*) answer '{"jsonrpc":"2.0","id":2,"result":null}';;
    *'"method":"exit"'*) : > "$0"; exit 0;;
  esac
done
"#;

    fn spec(script: &str, said_goodbye: &Path) -> ServerSpec {
        ServerSpec {
            name: "scripted".into(),
            command: "/bin/sh".into(),
            args: vec!["-c".into(), script.into(), said_goodbye.display().to_string()],
            extension_to_language: Default::default(),
            root_markers: Vec::new(),
            env: Default::default(),
        }
    }

    fn gone_within(pid: u32, wait: Duration) -> bool {
        let until = Instant::now() + wait;
        while platform::alive(pid) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        !platform::alive(pid)
    }

    #[test]
    fn a_server_process_is_told_to_exit_and_leaves_no_zombie() {
        let dir = tempfile::tempdir().expect("dir");
        let goodbye = dir.path().join("goodbye");
        let spec = spec(SERVER, &goodbye);
        let client =
            LspClient::spawn(&spec, Path::new("/bin/sh"), dir.path(), &AtomicBool::new(false)).expect("started");
        let pid = client.child.as_ref().expect("a process").id();
        assert!(platform::alive(pid));
        drop(client);
        assert!(goodbye.exists(), "the server was killed before it was told to exit");
        assert!(gone_within(pid, WAIT), "pid {pid} is still there, running or a zombie");
    }

    #[test]
    fn a_server_that_ignores_everything_is_still_ended_and_reaped() {
        let dir = tempfile::tempdir().expect("dir");
        let silent = r#"printf 'Content-Length: 36\r\n\r\n{"jsonrpc":"2.0","id":1,"result":{}}'; exec sleep 60"#;
        let spec = spec(silent, &dir.path().join("unused"));
        let client =
            LspClient::spawn(&spec, Path::new("/bin/sh"), dir.path(), &AtomicBool::new(false)).expect("started");
        let pid = client.child.as_ref().expect("a process").id();
        drop(client);
        assert!(gone_within(pid, WAIT * 2), "pid {pid} is still there, running or a zombie");
    }

    #[test]
    fn a_server_that_fails_to_start_says_why_in_its_own_words() {
        let dir = tempfile::tempdir().expect("dir");
        let failing = "echo 'error: toolchain 1.99 is not installed' >&2; exit 1";
        let spec = spec(failing, &dir.path().join("unused"));
        let error = LspClient::spawn(&spec, Path::new("/bin/sh"), dir.path(), &AtomicBool::new(false))
            .expect_err("it never answers");
        assert!(matches!(error, crate::client::StartError::Handshake(_)), "{error:?}");
        let error = error.to_string();
        assert!(error.contains("scripted stopped"), "{error}");
        assert!(error.contains("toolchain 1.99 is not installed"), "{error}");
    }
}
