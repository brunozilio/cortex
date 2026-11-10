use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::Receiver;
use superfik_core::{Effort, EngineKind, ModelOption, PromptRequest, RunId, WorkspaceId};
use superfik_plugins::{PackageFile, PluginManifest};

use std::path::{Path, PathBuf};

use superfik_core::PluginOrigin;

use super::*;

struct FakeOAuth;

impl OAuthFlow for FakeOAuth {
    fn sign_in(&self, request: &OAuthRequest, open: &mut dyn FnMut(String)) -> Result<Secrets, String> {
        open(format!("https://auth.test/authorize?resource={}", request.resource));
        Ok(Secrets::from([("access_token".to_owned(), "tok-1".to_owned())]))
    }

    fn refresh(&self, secrets: &Secrets) -> Result<Option<Secrets>, String> {
        Ok(secrets
            .contains_key("refresh_token")
            .then(|| Secrets::from([("access_token".to_owned(), "fresh".to_owned())])))
    }
}

struct FakeCli(Result<String, String>);

impl TokenCommand for FakeCli {
    fn token(&self, _: &str, _: &[String]) -> Result<String, String> {
        self.0.clone()
    }
}

/// Writes a plugin folder: its manifest and files.
fn folder(root: &Path, json: &str, files: Vec<PackageFile>) -> PathBuf {
    let manifest = PluginManifest::parse(json).expect("manifest");
    let dir = root.join(manifest.id.as_str());
    for file in &files {
        let path = dir.join(&file.path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, &file.bytes).expect("file");
    }
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join(superfik_plugins::MANIFEST_FILE), json).expect("manifest");
    dir
}

fn sources() -> Vec<(&'static str, Vec<PackageFile>)> {
    vec![
        (
            r#"{"id":"mail","name":"Mail","version":"1","accounts":"many","auth":{"kind":"oauth"},
            "mcp":[{"transport":"http","name":"mail","url":"https://mail.test/mcp",
                    "headers":{"Authorization":"Bearer {{secret.access_token}}"}}]}"#,
            vec![PackageFile { path: "skills/inbox/SKILL.md".into(), bytes: b"---\ndescription: Inbox\n---".to_vec() }],
        ),
        (
            r#"{"id":"docs","name":"Docs","version":"1","auth":{"kind":"api_key","fields":[{"key":"api_key","label":"Key"}]},
            "mcp":[{"transport":"stdio","name":"docs","command":"docs-mcp","env":{"KEY":"{{secret.api_key}}"}}]}"#,
            vec![],
        ),
        (
            r#"{"id":"browser","name":"Browser","version":"1",
            "mcp":[{"transport":"stdio","name":"browser","command":"npx","args":["browser-mcp"]}]}"#,
            vec![],
        ),
        (
            r#"{"id":"hub","name":"Hub","version":"1","auth":{"kind":"cli","program":"hub","args":["token"]},
            "mcp":[{"transport":"http","name":"hub","url":"https://hub.test","headers":{"Authorization":"Bearer {{secret.token}}"}}]}"#,
            vec![],
        ),
    ]
}

fn write_sources(root: &Path) {
    for (json, files) in sources() {
        folder(root, json, files);
    }
}

struct Rig {
    plugins: Plugins,
    outbox: Outbox,
    events: Receiver<ServerEvent>,
    secrets: Arc<InMemorySecretStore>,
    dir: tempfile::TempDir,
}

impl Rig {
    fn source(&self, plugin: &str) -> PathBuf {
        self.dir.path().join("sources").join(plugin)
    }

    fn install(&self, plugin: &str) -> ServerEvent {
        self.plugins.install(&OWNER, &self.source(plugin))
    }
}

fn rig(cli: Result<String, String>) -> Rig {
    let dir = tempfile::tempdir().expect("dir");
    let secrets = Arc::new(InMemorySecretStore::default());
    write_sources(&dir.path().join("sources"));
    let plugins = Plugins::new(
        Arc::new(InMemoryPluginStore::new(dir.path().join("store"))),
        secrets.clone(),
        Arc::new(FakeOAuth),
        Arc::new(FakeCli(cli)),
    );
    let (tx, events) = crossbeam_channel::unbounded();
    Rig { plugins, outbox: Outbox::new(tx, Arc::new(|| {})), events, secrets, dir }
}

const OWNER: Principal = Principal::LocalOwner;

fn id(text: &str) -> PluginId {
    PluginId(text.into())
}

fn connections(event: &ServerEvent) -> Vec<Connection> {
    match event {
        ServerEvent::Plugins { connections, .. } => connections.clone(),
        other => panic!("expected plugins, got {other:?}"),
    }
}

fn next(rig: &Rig) -> ServerEvent {
    rig.events.recv_timeout(Duration::from_secs(5)).expect("event")
}

fn request(connections: Vec<Connection>) -> PromptRequest {
    PromptRequest {
        workspace: WorkspaceId::new(),
        cwd: "/repo".into(),
        prompt: "hi".into(),
        model: ModelOption::new(EngineKind::Native, "m", "M"),
        effort: Effort::Medium,
        repositories: vec![],
        skills: vec![],
        disabled_skills: Vec::new(),
        references: vec![],
        resume: None,
        command: None,
        memory: None,
        permission: Default::default(),
        safety_check: true,
        connections,
        plugins: None,
        credentials: Default::default(),
        output_style: None,
    }
}

#[test]
fn installing_lays_out_the_plugin_and_lists_its_skills() {
    let rig = rig(Ok("t".into()));
    let ServerEvent::Plugins { plugins, .. } = rig.plugins.list(&OWNER) else { panic!("plugins") };
    assert!(plugins.is_empty(), "no plugin ships with Superfik");

    let ServerEvent::Plugins { plugins, connections } = rig.install("mail") else { panic!("plugins") };
    assert_eq!(plugins.len(), 1);
    assert!(plugins[0].installed);
    assert!(connections.is_empty(), "an OAuth plugin waits for an account");
    assert_eq!(rig.plugins.skills(&OWNER)[0].name, "mail:inbox");
    assert!(matches!(rig.install("mail"), ServerEvent::PluginFailed { .. }), "already installed");
    assert!(matches!(rig.install("nope"), ServerEvent::PluginFailed { .. }), "no such folder");
}

#[test]
fn a_plugin_without_sign_in_gets_one_implicit_account() {
    let rig = rig(Ok("t".into()));
    let event = rig.install("browser");
    let accounts = connections(&event);
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].slash_name(), "browser");
}

#[test]
fn two_oauth_accounts_become_two_servers_in_a_run() {
    let rig = rig(Ok("t".into()));
    rig.install("mail");
    rig.plugins.connect(&OWNER, &id("mail"), "Work", &[], &rig.outbox);
    assert!(matches!(next(&rig), ServerEvent::AuthorizeAt { url, .. } if url.contains("mail.test")));
    assert_eq!(connections(&next(&rig)).len(), 1);
    rig.plugins.connect(&OWNER, &id("mail"), "Home", &[], &rig.outbox);
    next(&rig);
    let accounts = connections(&next(&rig));
    assert_eq!(accounts.len(), 2);

    let run = RunId::new();
    let briefed = rig.plugins.brief(&OWNER, run, &request(accounts.clone())).expect("brief");
    let brief = briefed.plugins.clone().expect("brief");
    let path = brief.mcp_config.clone().expect("config");
    let config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
    assert_eq!(config["mcpServers"]["mail-work"]["headers"]["Authorization"], "Bearer tok-1");
    assert!(config["mcpServers"]["mail-home"].is_object());
    assert_eq!(brief.instructions.len(), 2);
    assert_eq!(brief.plugin_dirs.len(), 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).expect("meta").permissions().mode() & 0o777, 0o600);
    }
    brief::discard(Some(&brief));
    assert!(!path.exists());

    rig.plugins.disconnect(&OWNER, accounts[0].id);
    assert!(rig.secrets.get(&OWNER, accounts[0].id).is_none());
    let briefed = rig.plugins.brief(&OWNER, RunId::new(), &request(accounts)).expect("brief");
    let brief = briefed.plugins.expect("brief");
    assert_eq!(brief.instructions.len(), 1);
    brief::discard(Some(&brief));
}

#[test]
fn api_keys_are_checked_and_one_account_plugins_replace_theirs() {
    let rig = rig(Ok("t".into()));
    rig.install("docs");
    rig.plugins.connect(&OWNER, &id("docs"), "", &[("api_key".into(), "  ".into())], &rig.outbox);
    assert!(matches!(next(&rig), ServerEvent::PluginFailed { message, .. } if message.contains("Key is empty")));
    rig.plugins.connect(&OWNER, &id("docs"), "a", &[("api_key".into(), "k1".into())], &rig.outbox);
    let first = connections(&next(&rig))[0].clone();
    rig.plugins.connect(&OWNER, &id("docs"), "b", &[("api_key".into(), "k2".into())], &rig.outbox);
    let accounts = connections(&next(&rig));
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].label, "b");
    assert!(rig.secrets.get(&OWNER, first.id).is_none(), "the replaced account's key is forgotten");

    let brief = rig.plugins.brief(&OWNER, RunId::new(), &request(accounts)).expect("brief").plugins.expect("brief");
    let config = std::fs::read_to_string(brief.mcp_config.as_ref().expect("config")).expect("read");
    assert!(config.contains(r#""KEY":"k2""#));
    brief::discard(Some(&brief));
}

#[test]
fn cli_accounts_need_the_cli_signed_in() {
    let rig = rig(Err("`hub` is not signed in".into()));
    rig.install("hub");
    rig.plugins.connect(&OWNER, &id("hub"), "", &[], &rig.outbox);
    assert!(matches!(next(&rig), ServerEvent::PluginFailed { .. }));

    let rig = self::rig(Ok("gho_1".into()));
    rig.install("hub");
    rig.plugins.connect(&OWNER, &id("hub"), "", &[], &rig.outbox);
    let accounts = connections(&next(&rig));
    let brief = rig.plugins.brief(&OWNER, RunId::new(), &request(accounts)).expect("brief").plugins.expect("brief");
    let config = std::fs::read_to_string(brief.mcp_config.as_ref().expect("config")).expect("read");
    assert!(config.contains("Bearer gho_1"));
    brief::discard(Some(&brief));
}

#[test]
fn uninstalling_forgets_accounts_and_files() {
    let rig = rig(Ok("t".into()));
    rig.install("docs");
    rig.plugins.connect(&OWNER, &id("docs"), "a", &[("api_key".into(), "k".into())], &rig.outbox);
    let account = connections(&next(&rig))[0].clone();
    let event = rig.plugins.uninstall(&OWNER, &id("docs"));
    assert!(connections(&event).is_empty());
    assert!(rig.secrets.get(&OWNER, account.id).is_none());
    assert!(rig.plugins.skills(&OWNER).is_empty());
    let brief = rig.plugins.brief(&OWNER, RunId::new(), &request(vec![account])).expect("brief");
    assert_eq!(brief.plugins, None, "nothing installed, nothing briefed");
}

#[test]
fn a_local_folder_installs_too() {
    let rig = rig(Ok("t".into()));
    let folder = tempfile::tempdir().expect("folder");
    std::fs::write(
        folder.path().join(superfik_plugins::MANIFEST_FILE),
        r#"{"id":"mine","name":"Mine","version":"0.1"}"#,
    )
    .expect("write");
    let ServerEvent::Plugins { plugins, connections } = rig.plugins.install(&OWNER, folder.path()) else {
        panic!("plugins")
    };
    let mine = plugins.iter().find(|p| p.id == id("mine")).expect("listed");
    assert_eq!(mine.origin, PluginOrigin::Local(folder.path().to_path_buf()));
    assert!(connections.is_empty(), "no tools: no account to pick");
}

struct FakeSetup(Result<(), String>, std::sync::Mutex<Vec<String>>);

impl SetupRunner for FakeSetup {
    fn run(&self, command: &superfik_plugins::SetupCommand) -> Result<(), String> {
        self.1.lock().expect("lock").push(command.command.clone());
        self.0.clone()
    }
}

fn with_setup(result: Result<(), String>) -> (Rig, Arc<FakeSetup>) {
    let mut rig = rig(Ok("t".into()));
    let setup = Arc::new(FakeSetup(result, std::sync::Mutex::default()));
    folder(
        &rig.dir.path().join("sources"),
        r#"{"id":"browser2","name":"Browser","version":"1","setup":{"command":"npx","args":["install"]},
        "mcp":[{"transport":"stdio","name":"browser","command":"npx"}]}"#,
        vec![],
    );
    rig.plugins = rig.plugins.clone().with_setup(setup.clone());
    (rig, setup)
}

#[test]
fn setup_runs_after_installing_and_reports_back() {
    let (rig, setup) = with_setup(Ok(()));
    rig.plugins.install_and_set_up(&OWNER, &rig.source("browser2"), &rig.outbox);
    assert!(matches!(next(&rig), ServerEvent::Plugins { .. }), "installed at once");
    assert!(matches!(next(&rig), ServerEvent::Plugins { .. }), "then set up");
    assert_eq!(*setup.1.lock().expect("lock"), vec!["npx".to_owned()]);

    let (rig, _) = with_setup(Err("no network".into()));
    rig.plugins.install_and_set_up(&OWNER, &rig.source("browser2"), &rig.outbox);
    next(&rig);
    assert!(matches!(next(&rig), ServerEvent::PluginFailed { message, .. } if message.contains("no network")));
}

#[test]
fn a_members_folder_plugin_never_runs_its_setup_on_the_server() {
    let (rig, setup) = with_setup(Ok(()));
    let folder = tempfile::tempdir().expect("folder");
    let manifest = r#"{"id":"mine","name":"Mine","version":"1","setup":{"command":"sh","args":["-c","id"]}}"#;
    std::fs::write(folder.path().join("superfik-plugin.json"), manifest).expect("manifest");
    let member = Principal::Member(superfik_core::Member {
        user: superfik_core::UserId::new(),
        org: superfik_core::OrgId::new(),
    });
    rig.plugins.install_and_set_up(&member, folder.path(), &rig.outbox);
    assert!(matches!(next(&rig), ServerEvent::Plugins { .. }), "installed");
    let later = rig.events.recv_timeout(std::time::Duration::from_millis(200));
    assert!(later.is_err(), "no setup followed: {later:?}");
    assert!(setup.1.lock().expect("lock").is_empty(), "its setup never ran");
}
