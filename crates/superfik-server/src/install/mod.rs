//! `superfik-server install`: the server as a systemd service on Linux, the
//! way the Docker image runs it (deploy/Dockerfile). The server runs as its
//! own user over a `0700` data folder; agents' file, shell and MCP tools run
//! as `superfik-runner` (group `superfik-runs`, which also owns the
//! checkouts) through `native-as-runner`, the one command sudo lets the
//! server run as them. Every step is a program with its arguments (no
//! shell), and every failure stops the install.

use std::path::{Path, PathBuf};

use crate::cli::{self, Command};
use crate::config::Config;

pub const LIB: &str = "/usr/local/lib/superfik-server";
pub const RUNNER: &str = "superfik-runner";
pub const RUNS_GROUP: &str = "superfik-runs";
pub const RUNNER_HOME: &str = "/var/lib/superfik-runner";
pub const CHECKOUTS: &str = "/var/lib/superfik-runs";
pub const PROXIED: &str = "127.0.0.1:7420";

const UNIT: &str = "/etc/systemd/system/superfik.service";
const SUDOERS: &str = "/etc/sudoers.d/superfik-runner";
const CADDYFILE: &str = "/etc/caddy/Caddyfile";
const CADDY_IMPORT: &str = "import sites/*";

pub type Failure = Box<dyn std::error::Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub user: String,
    pub domain: Option<String>,
    pub data: PathBuf,
    pub listen: String,
    pub lib: PathBuf,
}

impl Plan {
    pub fn new(config: &Config, domain: Option<String>, user: String) -> Result<Self, Failure> {
        let listen = if domain.is_some() { PROXIED.to_owned() } else { config.listen.to_string() };
        let data = config.data.clone();
        if !plain_path(&data) {
            return Err(format!(
                "the data folder {} must be an absolute path of letters, digits, '/', '.', '_' and '-' \
                 (it goes in the systemd unit)",
                data.display()
            )
            .into());
        }
        Ok(Self { user, domain, data, listen, lib: PathBuf::from(LIB) })
    }

    pub fn binary(&self) -> PathBuf {
        self.lib.join("superfik-server")
    }

    pub fn worker(&self) -> PathBuf {
        self.lib.join("superfik-native-worker")
    }

    pub fn as_runner(&self) -> PathBuf {
        self.lib.join("native-as-runner")
    }
}

pub fn domain_name(text: &str) -> Option<String> {
    let name = text.trim().trim_end_matches('.').to_ascii_lowercase();
    let label = |label: &str| {
        (1..=63).contains(&label.len())
            && label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    let labels: Vec<&str> = name.split('.').collect();
    let top_is_a_name = labels.last().is_some_and(|top| top.bytes().any(|byte| byte.is_ascii_alphabetic()));
    (name.len() <= 253 && labels.len() >= 2 && labels.iter().all(|part| label(part)) && top_is_a_name).then_some(name)
}

pub fn service_user(text: &str) -> Option<String> {
    let mut bytes = text.bytes();
    let first = bytes.next().is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_');
    let rest = bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-');
    (first && rest && text.len() <= 32 && text != "root" && text != RUNNER).then(|| text.to_owned())
}

fn plain_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    path.is_absolute()
        && !text.contains("..")
        && text.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
}

pub fn systemd_unit(plan: &Plan) -> String {
    let data = plan.data.display();
    format!(
        "[Unit]\n\
         Description=Superfik server\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         User={user}\n\
         SupplementaryGroups={RUNS_GROUP}\n\
         Environment=SUPERFIK_DATA={data}\n\
         Environment=SUPERFIK_LISTEN={listen}\n\
         Environment=SUPERFIK_CHECKOUTS={CHECKOUTS}\n\
         Environment=SUPERFIK_NATIVE_WORKER={as_runner}\n\
         Environment=HOME={data}\n\
         Environment=XDG_CONFIG_HOME={data}\n\
         # Checkouts are group-writable for the runner; the data folder stays 0700.\n\
         UMask=0002\n\
         ExecStart={binary} serve\n\
         Restart=on-failure\n\
         RestartSec=2\n\
         # Hardening that keeps sudo (set-uid) working: no NoNewPrivileges=.\n\
         ProtectSystem=strict\n\
         ReadWritePaths={data} {CHECKOUTS} {RUNNER_HOME}\n\
         ReadWritePaths=-/run/sudo\n\
         ProtectHome=yes\n\
         PrivateTmp=yes\n\
         \n\
         [Install]\n\
         WantedBy=multi-user.target\n",
        user = plan.user,
        listen = plan.listen,
        as_runner = plan.as_runner().display(),
        binary = plan.binary().display(),
    )
}

pub fn sudoers(plan: &Plan) -> String {
    format!("{} ALL=({RUNNER}:{RUNS_GROUP}) NOPASSWD: {}\n", plan.user, plan.worker().display())
}

pub fn caddy_site(domain: &str, listen: &str) -> String {
    format!("{domain} {{\n\treverse_proxy {listen}\n}}\n")
}

pub fn trusted(uid: u32, mode: u32) -> bool {
    uid == 0 && mode & 0o022 == 0
}

pub trait Machine {
    fn run(&mut self, program: &str, args: &[&str]) -> std::io::Result<bool>;
    fn has(&mut self, program: &str) -> bool;
    fn read(&mut self, path: &Path) -> Option<String>;
    fn write(&mut self, path: &Path, contents: &str, mode: u32) -> std::io::Result<()>;
    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()>;
    fn untrusted(&mut self, path: &Path) -> Option<String>;
}

fn must(machine: &mut dyn Machine, program: &str, args: &[&str]) -> Result<(), Failure> {
    let command = || format!("`{program} {}`", args.join(" "));
    match machine.run(program, args) {
        Ok(true) => Ok(()),
        Ok(false) => Err(format!("{} failed; nothing after it was done", command()).into()),
        Err(error) => Err(format!("could not run {}: {error}", command()).into()),
    }
}

pub fn install(plan: &Plan, config: &Config, machine: &mut dyn Machine) -> Result<String, Failure> {
    for path in [plan.lib.clone(), plan.binary(), plan.worker(), plan.as_runner()] {
        if let Some(why) = machine.untrusted(&path) {
            return Err(format!(
                "{} {why}: install from the release archive (scripts/install-server.sh), \
                 which puts the server in {LIB}, root's alone",
                path.display()
            )
            .into());
        }
    }
    if !machine.has("sudo") {
        return Err("agents' tools run as superfik-runner through sudo: install sudo first".into());
    }
    let user = plan.user.as_str();
    let data = plan.data.to_string_lossy().into_owned();
    let nologin = "/usr/sbin/nologin";

    if !machine.run("getent", &["group", RUNS_GROUP])? {
        must(machine, "groupadd", &["--system", RUNS_GROUP])?;
    }
    if machine.run("id", &["-u", user])? {
        must(machine, "usermod", &["--append", "--groups", RUNS_GROUP, user])?;
    } else {
        let args = ["--system", "--home-dir", &data, "--no-create-home", "--gid", RUNS_GROUP, "--shell", nologin, user];
        must(machine, "useradd", &args)?;
    }
    if !machine.run("id", &["-u", RUNNER])? {
        let args = ["--system", "--home-dir", RUNNER_HOME, "--no-create-home", "--gid", RUNS_GROUP, "--shell", nologin];
        must(machine, "useradd", &[&args[..], &[RUNNER]].concat())?;
    }
    must(machine, "install", &["-d", "-o", RUNNER, "-g", RUNS_GROUP, "-m", "0700", RUNNER_HOME])?;
    must(machine, "install", &["-d", "-o", user, "-g", RUNS_GROUP, "-m", "2770", CHECKOUTS])?;

    let pending = PathBuf::from(format!("{SUDOERS}.new"));
    machine.write(&pending, &sudoers(plan), 0o440)?;
    must(machine, "visudo", &["-c", "-q", "-f", &pending.to_string_lossy()])?;
    machine.rename(&pending, Path::new(SUDOERS))?;

    let first = cli::run(&Command::Init { owner: "admin".to_owned() }, config)?;
    if let Some(domain) = &plan.domain {
        cli::run(&Command::Domain(Some(domain.clone())), config)?;
    }
    must(machine, "chown", &["-R", &format!("{user}:"), &data])?;
    must(machine, "chmod", &["0700", &data])?;

    machine.write(Path::new(UNIT), &systemd_unit(plan), 0o644)?;
    if let Some(domain) = &plan.domain {
        caddy(machine, domain, &plan.listen)?;
    }
    must(machine, "systemctl", &["daemon-reload"])?;
    must(machine, "systemctl", &["enable", "superfik"])?;
    must(machine, "systemctl", &["restart", "superfik"])?;

    let address = plan
        .domain
        .as_ref()
        .map_or_else(|| format!("http://<this server>:{}", config.listen.port()), |domain| format!("https://{domain}"));
    Ok(format!("Superfik is running at {address}.\n{first}"))
}

fn caddy(machine: &mut dyn Machine, domain: &str, listen: &str) -> Result<(), Failure> {
    if !machine.has("caddy") {
        println!("Installing Caddy for HTTPS…");
        if machine.has("apt-get") {
            must(machine, "apt-get", &["update", "-qq"])?;
            must(machine, "apt-get", &["install", "-y", "-qq", "caddy"])?;
        } else if machine.has("dnf") {
            must(machine, "dnf", &["install", "-y", "-q", "caddy"])?;
        } else {
            return Err("install Caddy (caddyserver.com), then run this again".into());
        }
    }
    must(machine, "install", &["-d", "-m", "0755", "/etc/caddy/sites"])?;
    machine.write(&PathBuf::from(format!("/etc/caddy/sites/{domain}.caddy")), &caddy_site(domain, listen), 0o644)?;
    let caddyfile = machine.read(Path::new(CADDYFILE)).unwrap_or_default();
    if !caddyfile.lines().any(|line| line.trim() == CADDY_IMPORT) {
        let separator = if caddyfile.is_empty() || caddyfile.ends_with('\n') { "" } else { "\n" };
        machine.write(Path::new(CADDYFILE), &format!("{caddyfile}{separator}{CADDY_IMPORT}\n"), 0o644)?;
    }
    must(machine, "systemctl", &["enable", "caddy"])?;
    must(machine, "systemctl", &["reload-or-restart", "caddy"])
}

mod system;
pub use system::{System, effective_uid};

#[cfg(all(test, unix))]
mod tests;
