use std::collections::{BTreeMap, BTreeSet};

use super::*;

#[derive(Default)]
struct Recorded {
    done: Vec<String>,
    files: BTreeMap<PathBuf, (String, u32)>,
    known: BTreeSet<String>,
    programs: BTreeSet<&'static str>,
    failing: Option<String>,
    untrusted: Option<PathBuf>,
}

impl Recorded {
    fn new() -> Self {
        Self { programs: ["sudo", "apt-get"].into(), ..Self::default() }
    }

    fn at(&self, line: &str) -> usize {
        self.done.iter().position(|done| done == line).unwrap_or_else(|| panic!("never ran {line}: {:#?}", self.done))
    }
}

impl Machine for Recorded {
    fn run(&mut self, program: &str, args: &[&str]) -> std::io::Result<bool> {
        let line = [program].iter().chain(args).copied().collect::<Vec<_>>().join(" ");
        self.done.push(line.clone());
        Ok(match (program, args) {
            ("getent", ["group", name]) | ("id", ["-u", name]) => self.known.contains(*name),
            _ => self.failing.as_deref() != Some(line.as_str()),
        })
    }
    fn has(&mut self, program: &str) -> bool {
        self.programs.contains(program)
    }
    fn read(&mut self, path: &Path) -> Option<String> {
        self.files.get(path).map(|(contents, _)| contents.clone())
    }
    fn write(&mut self, path: &Path, contents: &str, mode: u32) -> std::io::Result<()> {
        self.done.push(format!("write {}", path.display()));
        self.files.insert(path.to_path_buf(), (contents.to_owned(), mode));
        Ok(())
    }
    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        self.done.push(format!("rename {} {}", from.display(), to.display()));
        let file = self.files.remove(from).ok_or_else(|| std::io::Error::other("no file"))?;
        self.files.insert(to.to_path_buf(), file);
        Ok(())
    }
    fn untrusted(&mut self, path: &Path) -> Option<String> {
        (self.untrusted.as_deref() == Some(path)).then(|| "is writable by others".to_owned())
    }
}

fn config(dir: &tempfile::TempDir) -> Config {
    Config { listen: crate::config::DEFAULT_LISTEN.parse().expect("addr"), data: dir.path().join("data") }
}

fn plan(config: &Config, domain: Option<&str>) -> Plan {
    Plan::new(config, domain.map(str::to_owned), "superfik".into()).expect("plan")
}

fn file(machine: &Recorded, path: &str) -> (String, u32) {
    machine.files.get(Path::new(path)).cloned().unwrap_or_else(|| panic!("no {path}"))
}

#[test]
fn domains_are_host_names_only() {
    assert_eq!(domain_name("Superfik.Acme.io"), Some("superfik.acme.io".into()));
    assert_eq!(domain_name("superfik.acme.io."), Some("superfik.acme.io".into()));
    for bad in [
        "",
        "localhost",
        "acme.io;rm -rf /",
        "acme.io/../../etc/passwd",
        "a b.io",
        "-acme.io",
        "acme-.io",
        "acme..io",
        "10.0.0.1",
        "acme.io\nimport /etc/shadow",
        &format!("{}.io", "a".repeat(64)),
    ] {
        assert_eq!(domain_name(bad), None, "{bad:?}");
    }
}

#[test]
fn the_service_user_is_a_plain_system_name_never_root_or_the_runner() {
    assert_eq!(service_user("superfik"), Some("superfik".into()));
    assert_eq!(service_user("_svc-2"), Some("_svc-2".into()));
    for bad in ["", "root", RUNNER, "Superfik", "-x", "a b", "a;b", "a:b", &"a".repeat(33)] {
        assert_eq!(service_user(bad), None, "{bad:?}");
    }
}

#[test]
fn a_data_folder_the_unit_cannot_hold_as_it_is_is_refused() {
    let mut config = config(&tempfile::tempdir().expect("dir"));
    config.data = PathBuf::from("/srv/super fik");
    assert!(Plan::new(&config, None, "superfik".into()).is_err());
    config.data = PathBuf::from("relative");
    assert!(Plan::new(&config, None, "superfik".into()).is_err());
}

#[test]
fn the_unit_runs_tools_through_the_runner_and_keeps_sudo_working() {
    let config = Config {
        listen: crate::config::DEFAULT_LISTEN.parse().expect("addr"),
        data: PathBuf::from("/var/lib/superfik"),
    };
    let unit = systemd_unit(&plan(&config, Some("superfik.acme.io")));
    for line in [
        "User=superfik",
        "SupplementaryGroups=superfik-runs",
        "Environment=SUPERFIK_DATA=/var/lib/superfik",
        "Environment=SUPERFIK_LISTEN=127.0.0.1:7420",
        "Environment=SUPERFIK_CHECKOUTS=/var/lib/superfik-runs",
        "Environment=SUPERFIK_NATIVE_WORKER=/usr/local/lib/superfik-server/native-as-runner",
        "UMask=0002",
        "ExecStart=/usr/local/lib/superfik-server/superfik-server serve",
        "ProtectSystem=strict",
        "ReadWritePaths=/var/lib/superfik /var/lib/superfik-runs /var/lib/superfik-runner",
        "ProtectHome=yes",
        "PrivateTmp=yes",
    ] {
        assert!(unit.lines().any(|unit_line| unit_line == line), "missing {line}:\n{unit}");
    }
    for directive in [
        "NoNewPrivileges",
        "SystemCallFilter",
        "SystemCallArchitectures",
        "RestrictSUIDSGID",
        "RestrictAddressFamilies",
        "RestrictNamespaces",
        "RestrictRealtime",
        "PrivateDevices",
        "ProtectKernelTunables",
        "ProtectKernelModules",
        "ProtectKernelLogs",
        "ProtectClock",
        "MemoryDenyWriteExecute",
        "LockPersonality",
        "DynamicUser",
        "CapabilityBoundingSet",
    ] {
        assert!(!unit.lines().any(|line| line.starts_with(&format!("{directive}="))), "{directive} breaks sudo");
    }
    assert_eq!(
        sudoers(&plan(&config, None)),
        "superfik ALL=(superfik-runner:superfik-runs) NOPASSWD: /usr/local/lib/superfik-server/superfik-native-worker\n"
    );
    assert_eq!(caddy_site("superfik.acme.io", PROXIED), "superfik.acme.io {\n\treverse_proxy 127.0.0.1:7420\n}\n");
}

#[test]
fn only_roots_files_are_trusted() {
    assert!(trusted(0, 0o100755));
    assert!(trusted(0, 0o40755));
    assert!(!trusted(1001, 0o100755), "the CI runner's uid, kept by tar");
    assert!(!trusted(0, 0o100775), "group-writable");
    assert!(!trusted(0, 0o100757), "world-writable");
}

#[test]
fn install_sets_up_the_runner_and_starts_the_service_last() {
    let dir = tempfile::tempdir().expect("dir");
    let config = config(&dir);
    let plan = plan(&config, Some("superfik.acme.io"));
    let mut machine = Recorded::new();
    let said = install(&plan, &config, &mut machine).expect("install");
    assert!(said.contains("https://superfik.acme.io") && said.contains("First-access code for admin"), "{said}");

    let data = config.data.display().to_string();
    machine.at("groupadd --system superfik-runs");
    machine.at(&format!(
        "useradd --system --home-dir {data} --no-create-home --gid superfik-runs --shell /usr/sbin/nologin superfik"
    ));
    machine.at("useradd --system --home-dir /var/lib/superfik-runner --no-create-home --gid superfik-runs \
         --shell /usr/sbin/nologin superfik-runner");
    machine.at("install -d -o superfik -g superfik-runs -m 2770 /var/lib/superfik-runs");
    let (rule, mode) = file(&machine, "/etc/sudoers.d/superfik-runner");
    assert_eq!((rule, mode), (sudoers(&plan), 0o440));
    assert!(
        machine.at("visudo -c -q -f /etc/sudoers.d/superfik-runner.new")
            < machine.at("rename /etc/sudoers.d/superfik-runner.new /etc/sudoers.d/superfik-runner"),
        "checked before it is in place"
    );
    assert_eq!(file(&machine, "/etc/systemd/system/superfik.service"), (systemd_unit(&plan), 0o644));
    assert_eq!(file(&machine, "/etc/caddy/Caddyfile").0, "import sites/*\n");

    let chown = machine.at(&format!("chown -R superfik: {data}"));
    let restart = machine.at("systemctl restart superfik");
    assert!(chown < restart && machine.at("systemctl enable superfik") < restart);
    assert_eq!(machine.done.last().map(String::as_str), Some("systemctl restart superfik"));
    assert!(machine.done.iter().all(|line| !line.starts_with("sh ")), "no shell: {:#?}", machine.done);
    let settings = std::fs::read_to_string(config.settings_path()).expect("settings");
    assert!(settings.contains("superfik.acme.io"), "{settings}");
}

#[test]
fn installing_again_upgrades_without_repeating_the_setup() {
    let dir = tempfile::tempdir().expect("dir");
    let config = config(&dir);
    let plan = plan(&config, Some("superfik.acme.io"));
    install(&plan, &config, &mut Recorded::new()).expect("first");
    let mut machine = Recorded::new();
    machine.known = ["superfik-runs", "superfik", "superfik-runner"].map(str::to_owned).into();
    machine.programs.insert("caddy");
    machine.files.insert(CADDYFILE.into(), (":80 {\n\tfile_server\n}\nimport sites/*\n".into(), 0o644));
    let said = install(&plan, &config, &mut machine).expect("again");
    assert!(said.contains("set up already"), "{said}");
    assert!(machine.done.iter().all(|line| !line.starts_with("useradd") && !line.starts_with("groupadd")));
    machine.at("usermod --append --groups superfik-runs superfik");
    assert!(!machine.done.iter().any(|line| line.starts_with("apt-get")), "Caddy is there");
    assert!(!machine.done.contains(&format!("write {CADDYFILE}")), "the import is there");
    machine.at("systemctl restart superfik");
}

#[test]
fn a_failed_step_stops_the_install_and_names_it() {
    let dir = tempfile::tempdir().expect("dir");
    let config = config(&dir);
    let mut machine = Recorded::new();
    machine.failing = Some("groupadd --system superfik-runs".into());
    let error = install(&plan(&config, None), &config, &mut machine).expect_err("stops").to_string();
    assert!(error.contains("groupadd --system superfik-runs"), "{error}");
    assert_eq!(machine.done.last().map(String::as_str), Some("groupadd --system superfik-runs"));

    let mut machine = Recorded::new();
    machine.failing = Some("visudo -c -q -f /etc/sudoers.d/superfik-runner.new".into());
    assert!(install(&plan(&config, None), &config, &mut machine).is_err());
    assert!(!machine.files.contains_key(Path::new(SUDOERS)), "a sudoers file sudo refuses never goes in place");
    assert!(!machine.done.iter().any(|line| line.starts_with("systemctl")));
}

#[test]
fn files_anyone_but_root_could_change_are_not_installed() {
    let dir = tempfile::tempdir().expect("dir");
    let config = config(&dir);
    let plan = plan(&config, None);
    let mut machine = Recorded::new();
    machine.untrusted = Some(plan.worker());
    let error = install(&plan, &config, &mut machine).expect_err("refused").to_string();
    assert!(error.contains("superfik-native-worker"), "{error}");
    assert!(machine.done.is_empty(), "nothing done: {:#?}", machine.done);

    let mut machine = Recorded::new();
    machine.programs.remove("sudo");
    assert!(install(&plan, &config, &mut machine).expect_err("no sudo").to_string().contains("sudo"));
}
