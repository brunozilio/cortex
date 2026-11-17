#![allow(clippy::unwrap_used, reason = "a failed step fails the test")]

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::*;

const FAKE: &str = r#"#!/bin/sh
here=$(dirname "$0")
case "$*" in
  "tunnel login") touch "$here/cert.pem" ;;
  "tunnel list"*) if [ -f "$here/created" ]; then printf '[{"id":"t1","name":"%s"}]' "$6"; else echo '[]'; fi ;;
  "tunnel create"*) echo "$3" > "$here/created" ;;
  "tunnel route dns"*) echo "$5 $6" >> "$here/routed" ;;
  "tunnel --no-autoupdate run"*) echo "INF Registered tunnel connection connIndex=0 location=gru" >&2; exec sleep 30 ;;
  "tunnel --no-autoupdate --url"*) if [ -f "$here/broken" ]; then echo "ERR failed to request quick Tunnel" >&2; exit 1; fi
     echo "INF |  https://fake-words-1.trycloudflare.com  |" >&2; exec sleep 30 ;;
  *) echo "unexpected: $*" >&2; exit 2 ;;
esac
"#;

struct Fake {
    folder: tempfile::TempDir,
    data: tempfile::TempDir,
}

impl Fake {
    fn new() -> Self {
        let folder = tempfile::tempdir().unwrap();
        let program = folder.path().join("cloudflared");
        std::fs::write(&program, FAKE).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self { folder, data: tempfile::tempdir().unwrap() }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.folder.path().join(name)
    }

    fn cloudflared(&self) -> Cloudflared {
        Cloudflared::at(self.path("cloudflared"), self.path("cert.pem"))
    }

    fn book(&self) -> Book {
        Book::in_folder(self.data.path())
    }

    fn start(&self, hostname: Option<&str>) -> Result<(Up, Vec<String>), String> {
        let mut said = Vec::new();
        let plan = Plan { hostname: hostname.map(str::to_owned), port: 0 };
        let up = start(&self.book(), &self.cloudflared(), &plan, &mut |line| said.push(line.to_owned()))?;
        Ok((up, said))
    }
}

fn get(port: u16, path: &str) -> String {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(stream, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    answer
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn a_quick_tunnel_publishes_the_relay_and_the_book_learns_both_addresses() {
    let fake = Fake::new();
    let (up, said) = fake.start(None).unwrap();
    assert!(up.quick);
    assert_eq!(up.public, "https://fake-words-1.trycloudflare.com");
    assert!(said.iter().any(|line| line.contains("quick tunnel")));
    assert_eq!(fake.book().relay().as_deref(), Some("https://fake-words-1.trycloudflare.com"));
    assert_eq!(fake.book().machine_relay(), Some(format!("http://127.0.0.1:{}", up.local_port())));
    assert!(get(up.local_port(), "/relay.json").contains("\"relay\":true"));
}

#[test]
fn a_named_tunnel_signs_in_creates_and_routes_once_then_is_remembered() {
    let fake = Fake::new();
    let (up, said) = fake.start(Some("https://Relay.Example.com")).unwrap();
    assert!(!up.quick);
    assert_eq!(up.public, "https://relay.example.com");
    assert!(said.iter().any(|line| line.contains("Sign in to Cloudflare")));
    assert_eq!(read(&fake.path("created")).trim(), tunnel_name());
    assert_eq!(read(&fake.path("routed")).trim(), format!("{} relay.example.com", tunnel_name()));
    drop(up);
    let (again, said) = fake.start(None).unwrap();
    assert_eq!(again.public, "https://relay.example.com");
    assert!(!said.iter().any(|line| line.contains("Sign in") || line.contains("Creating")));
    assert_eq!(read(&fake.path("routed")).lines().count(), 2);
}

#[test]
fn a_tunnel_that_fails_says_what_cloudflared_said() {
    let fake = Fake::new();
    std::fs::write(fake.path("broken"), "").unwrap();
    let failed = fake.start(None).unwrap_err();
    assert!(failed.contains("failed to request quick Tunnel"), "{failed}");
    assert_eq!(fake.book().relay(), None);
}

#[test]
fn a_bad_host_name_is_refused_before_anything_starts() {
    let fake = Fake::new();
    assert!(fake.start(Some("not a host")).unwrap_err().contains("not a host name"));
    assert!(!fake.path("created").exists());
}

#[test]
fn a_port_already_taken_says_remote_up_may_be_running() {
    let fake = Fake::new();
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let plan = Plan { hostname: None, port: taken.local_addr().unwrap().port() };
    let failed = start(&fake.book(), &fake.cloudflared(), &plan, &mut |_| {}).unwrap_err();
    assert!(failed.contains("already running"), "{failed}");
}
