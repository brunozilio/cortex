use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use platform::Child;

pub const INSTALL: &str = "cloudflared is not installed. On macOS: `brew install cloudflared`; on other systems: \
https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/";
const QUICK_DOMAIN: &str = ".trycloudflare.com";
const KEPT_LINES: usize = 20;

#[derive(Debug, Clone)]
pub struct Cloudflared {
    program: PathBuf,
    cert: PathBuf,
}

#[derive(Debug)]
pub struct Running {
    child: Child,
    lines: mpsc::Receiver<String>,
    seen: Vec<String>,
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|folder| folder.join(name)).find(|path| path.is_file())
}

impl Cloudflared {
    pub fn find() -> Option<Self> {
        let program = on_path("cloudflared")?;
        let cert = std::env::home_dir()?.join(".cloudflared").join("cert.pem");
        Some(Self { program, cert })
    }

    #[cfg(test)]
    pub fn at(program: PathBuf, cert: PathBuf) -> Self {
        Self { program, cert }
    }

    pub fn signed_in(&self) -> bool {
        self.cert.is_file()
    }

    fn command(&self, arguments: &[&str]) -> Command {
        let mut command = Command::new(&self.program);
        command.args(arguments).stdin(Stdio::null());
        command
    }

    fn quietly(&self, arguments: &[&str]) -> Result<String, String> {
        let output = self.command(arguments).output().map_err(|error| format!("cloudflared could not start: {error}"))?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let said = String::from_utf8_lossy(&output.stderr);
        Err(format!("`cloudflared {}` failed: {}", arguments.join(" "), platform::child::last_lines(said.trim(), 5)))
    }

    pub fn sign_in(&self) -> Result<(), String> {
        let status = self
            .command(&["tunnel", "login"])
            .stdin(Stdio::inherit())
            .status()
            .map_err(|error| format!("cloudflared could not start: {error}"))?;
        if status.success() && self.signed_in() {
            Ok(())
        } else {
            Err("Signing in to Cloudflare did not finish; run `cloudflared tunnel login` and try again.".to_owned())
        }
    }

    pub fn tunnel_id(&self, name: &str) -> Result<Option<String>, String> {
        let listed = self.quietly(&["tunnel", "list", "--output", "json", "--name", name])?;
        Ok(tunnel_id_in(&listed, name))
    }

    pub fn create(&self, name: &str) -> Result<(), String> {
        self.quietly(&["tunnel", "create", name]).map(drop)
    }

    pub fn route(&self, name: &str, hostname: &str) -> Result<(), String> {
        self.quietly(&["tunnel", "route", "dns", "--overwrite-dns", name, hostname]).map(drop)
    }

    pub fn run_named(&self, name: &str, port: u16) -> Result<Running, String> {
        let origin = format!("http://127.0.0.1:{port}");
        self.spawn(&["tunnel", "--no-autoupdate", "run", "--url", &origin, name])
    }

    pub fn run_quick(&self, port: u16) -> Result<Running, String> {
        let origin = format!("http://127.0.0.1:{port}");
        self.spawn(&["tunnel", "--no-autoupdate", "--url", &origin])
    }

    fn spawn(&self, arguments: &[&str]) -> Result<Running, String> {
        let mut command = self.command(arguments);
        command.stdout(Stdio::null()).stderr(Stdio::piped());
        let child = Child::spawn(&mut command).map_err(|error| format!("cloudflared could not start: {error}"))?;
        let (sender, lines) = mpsc::channel();
        if let Some(stderr) = child.take_stderr() {
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        Ok(Running { child, lines, seen: Vec::new() })
    }
}

impl Running {
    pub fn alive(&self) -> bool {
        self.child.is_alive()
    }

    fn take(&mut self, line: String) {
        self.seen.push(line);
        if self.seen.len() > KEPT_LINES {
            self.seen.remove(0);
        }
    }

    pub fn drain(&mut self) {
        while let Ok(line) = self.lines.try_recv() {
            self.take(line);
        }
    }

    pub fn wait_for(&mut self, wanted: impl Fn(&str) -> Option<String>, within: Duration) -> Result<String, String> {
        let deadline = Instant::now() + within;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left.min(Duration::from_millis(200))) {
                Ok(line) => {
                    let found = wanted(&line);
                    self.take(line);
                    if let Some(found) = found {
                        return Ok(found);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) if left > Duration::ZERO && self.alive() => {}
                Err(_) => return Err(format!("cloudflared did not get ready:\n{}", self.said())),
            }
        }
    }

    pub fn said(&self) -> String {
        self.seen.join("\n")
    }
}

pub fn quick_address(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let address: String = line[start..].chars().take_while(|letter| !letter.is_whitespace() && *letter != '|').collect();
    let host = &address["https://".len()..];
    let fits = host.ends_with(QUICK_DOMAIN)
        && host.len() > QUICK_DOMAIN.len()
        && host.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'.');
    fits.then_some(address)
}

pub fn connected(line: &str) -> Option<String> {
    line.contains("Registered tunnel connection").then(String::new)
}

pub fn tunnel_id_in(listed: &str, name: &str) -> Option<String> {
    let tunnels: Vec<serde_json::Value> = serde_json::from_str(listed.trim()).ok()?;
    tunnels
        .iter()
        .filter(|tunnel| tunnel.get("deleted_at").is_none_or(|at| at.is_null() || at.as_str() == Some("0001-01-01T00:00:00Z")))
        .find(|tunnel| tunnel.get("name").and_then(|found| found.as_str()) == Some(name))
        .and_then(|tunnel| tunnel.get("id")?.as_str().map(str::to_owned))
}

pub fn hostname(given: &str) -> Result<String, String> {
    let host = given.trim().trim_start_matches("https://").trim_end_matches('/').to_ascii_lowercase();
    let labels: Vec<&str> = host.split('.').collect();
    let fits = labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        });
    if fits { Ok(host) } else { Err(format!("`{given}` is not a host name such as relay.example.com")) }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a failed step fails the test")]
mod tests {
    use super::*;

    #[test]
    fn the_quick_tunnel_address_is_read_from_cloudflared_s_banner() {
        let line = "2026-11-17T12:00:00Z INF |  https://lucky-words-here-1234.trycloudflare.com                    |";
        assert_eq!(quick_address(line).as_deref(), Some("https://lucky-words-here-1234.trycloudflare.com"));
        assert_eq!(quick_address("INF Visit https://www.cloudflare.com/website-terms/"), None);
        assert_eq!(quick_address("https://.trycloudflare.com"), None);
        assert_eq!(quick_address("https://evil.example/?x=.trycloudflare.com"), None);
    }

    #[test]
    fn a_tunnel_is_found_by_name_and_a_deleted_one_is_not() {
        let listed = r#"[{"id":"aaa","name":"other"},{"id":"bbb","name":"app-relay","deleted_at":"0001-01-01T00:00:00Z"}]"#;
        assert_eq!(tunnel_id_in(listed, "app-relay").as_deref(), Some("bbb"));
        let deleted = r#"[{"id":"ccc","name":"app-relay","deleted_at":"2026-01-01T00:00:00Z"}]"#;
        assert_eq!(tunnel_id_in(deleted, "app-relay"), None);
        assert_eq!(tunnel_id_in("null", "app-relay"), None);
        assert_eq!(tunnel_id_in("[]", "app-relay"), None);
    }

    #[test]
    fn a_host_name_is_kept_bare_and_lowercase() {
        assert_eq!(hostname(" https://Relay.Example.com/ ").unwrap(), "relay.example.com");
        for bad in ["relay", "relay..example.com", "-relay.example.com", "relay.example.com/path", "re lay.example.com"] {
            assert!(hostname(bad).is_err(), "{bad}");
        }
    }
}
