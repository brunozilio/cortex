use std::io::{BufReader, PipeReader, PipeWriter};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde_json::{Value, json};

use super::LspClient;
use crate::lsp::framing::{read_message, write_message};

const WAIT: Duration = Duration::from_secs(5);

struct Server {
    input: BufReader<PipeReader>,
    output: PipeWriter,
}

impl Server {
    fn next(&mut self) -> Value {
        read_message(&mut self.input).expect("read").expect("a message")
    }

    fn send(&mut self, message: Value) {
        write_message(&mut self.output, &message).expect("write");
    }

    fn handshake(&mut self) {
        let initialize = self.next();
        assert_eq!(initialize["method"], "initialize");
        assert_eq!(initialize["params"]["rootUri"], "file:///w");
        self.send(json!({"jsonrpc": "2.0", "id": initialize["id"], "result": {"capabilities": {}}}));
        assert_eq!(self.next()["method"], "initialized");
    }
}

fn connected(script: impl FnOnce(Server) + Send + 'static) -> (LspClient, std::thread::JoinHandle<()>) {
    let (server_reads, client_writes) = std::io::pipe().expect("pipe");
    let (client_reads, server_writes) = std::io::pipe().expect("pipe");
    let server =
        std::thread::spawn(move || script(Server { input: BufReader::new(server_reads), output: server_writes }));
    let client =
        LspClient::connect("fake", client_reads, client_writes, Path::new("/w"), None, &AtomicBool::new(false))
            .expect("connected");
    (client, server)
}

#[test]
fn an_answer_is_matched_to_its_request_past_what_the_server_says_meanwhile() {
    let (client, server) = connected(|mut server| {
        server.handshake();
        let asked = server.next();
        assert_eq!(asked["method"], "textDocument/hover");
        server.send(json!({"jsonrpc": "2.0", "method": "window/logMessage", "params": {"message": "indexing"}}));
        server.send(
            json!({"jsonrpc": "2.0", "id": "cfg", "method": "workspace/configuration", "params": {"items": [{}, {}]}}),
        );
        let answered = server.next();
        assert_eq!(answered, json!({"jsonrpc": "2.0", "id": "cfg", "result": [null, null]}));
        server.send(json!({"jsonrpc": "2.0", "id": asked["id"], "result": {"contents": "fn a()"}}));
    });
    let answer = client.request("textDocument/hover", json!({}), &AtomicBool::new(false), WAIT).expect("answer");
    assert_eq!(answer, json!({"contents": "fn a()"}));
    server.join().expect("server");
}

#[test]
fn a_file_is_opened_once_then_changed_and_its_diagnostics_awaited_by_version() {
    let (client, server) = connected(|mut server| {
        server.handshake();
        let opened = server.next();
        assert_eq!(opened["method"], "textDocument/didOpen");
        assert_eq!(opened["params"]["textDocument"]["languageId"], "rust");
        assert_eq!(opened["params"]["textDocument"]["version"], 1);
        let changed = server.next();
        assert_eq!(changed["method"], "textDocument/didChange");
        assert_eq!(changed["params"]["textDocument"]["version"], 2);
        assert_eq!(changed["params"]["contentChanges"], json!([{"text": "fn b() {}"}]));
        let uri = changed["params"]["textDocument"]["uri"].clone();
        let publish = |version: u64, message: &str| {
            json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
            "params": {"uri": uri, "version": version, "diagnostics": [{"message": message}]}})
        };
        server.send(publish(1, "stale"));
        server.send(publish(2, "fresh"));
    });
    let file = Path::new("/w/src/a.rs");
    assert_eq!(client.sync(file, "rust", "fn a() {}").expect("open"), 1);
    assert_eq!(client.sync(file, "rust", "fn b() {}").expect("change"), 2);
    let found = client.diagnostics(file, 2, WAIT).expect("diagnostics");
    assert_eq!(found, [json!({"message": "fresh"})]);
    server.join().expect("server");
}

#[test]
fn an_error_answer_and_a_server_that_ends_are_reported() {
    let (client, server) = connected(|mut server| {
        server.handshake();
        let asked = server.next();
        server.send(
            json!({"jsonrpc": "2.0", "id": asked["id"], "error": {"code": -32801, "message": "content modified"}}),
        );
    });
    let error = client.request("textDocument/definition", json!({}), &AtomicBool::new(false), WAIT).expect_err("error");
    assert!(error.contains("content modified"), "{error}");
    server.join().expect("server");
    let gone = client.request("textDocument/definition", json!({}), &AtomicBool::new(false), WAIT).expect_err("gone");
    assert!(gone.contains("fake stopped"), "{gone}");
    assert!(!client.is_alive());
}
