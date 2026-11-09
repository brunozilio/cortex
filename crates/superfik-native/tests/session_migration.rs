use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use superfik_core::{EngineEvent, EngineKind, ModelOption, OrgId, Principal, PromptRequest, RunId, SessionId, UserId};
use superfik_engine::AgentEngine;
use superfik_native::{
    ApiSource, NativeEngine,
    model::{ModelApi, ModelError, scripted::ScriptedModel},
    sessions::{
        FileSessions, Sessions,
        migration::{self, Entry, Manifest, SourceFormat},
    },
    transcript::Message,
};
struct Fixture {
    dir: tempfile::TempDir,
    project: PathBuf,
    exports: PathBuf,
    manifest: PathBuf,
    owner: Principal,
    id: SessionId,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("fixture");
        let project = dir.path().join("project");
        let exports = dir.path().join("exports");
        std::fs::create_dir_all(&project).expect("project");
        std::fs::create_dir_all(&exports).expect("exports");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exports, std::fs::Permissions::from_mode(0o700)).expect("private exports");
        }
        let manifest = dir.path().join("imports.json");
        let fixture = Self {
            dir,
            project,
            exports,
            manifest,
            owner: Principal::member(UserId::new(), OrgId::new()),
            id: SessionId("legacy-session".into()),
        };
        let transcript = [
            json!({"type":"user","message":{"content":"old request"}}),
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"legacy report"}]}}),
        ]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
        std::fs::write(fixture.exports.join("old.jsonl"), transcript).expect("export");
        fixture.write(vec![fixture.entry()]);
        fixture
    }
    fn entry(&self) -> Entry {
        let bytes = std::fs::read(self.exports.join("old.jsonl")).expect("export");
        Entry {
            owner: self.owner.clone(),
            cwd: self.project.clone(),
            session_id: self.id.clone(),
            source: "old.jsonl".into(),
            format: SourceFormat::ClaudeJsonl,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        }
    }
    fn write(&self, entries: Vec<Entry>) {
        superfik_private_file::write(
            &self.manifest,
            &serde_json::to_vec(&Manifest { version: 1, entries }).expect("manifest"),
        )
        .expect("private manifest");
    }
    fn request(&self) -> PromptRequest {
        let mut request = PromptRequest::plain(
            self.project.clone(),
            "continue",
            ModelOption::new(EngineKind::Native, "fixture", "Fixture"),
        );
        request.credentials.owner = Some(self.owner.clone());
        request.resume = Some(self.id.clone());
        request
    }
    fn store(&self) -> FileSessions {
        FileSessions::new(self.dir.path().join("native-sessions"))
    }
}
#[test]
fn administrative_import_is_private_idempotent_and_bound_to_remote_owner_and_project() {
    let f = Fixture::new();
    let store = f.store();
    let prepared = migration::prepare(&f.manifest, &f.exports).expect("validate");
    assert_eq!(prepared.count(), 1);
    assert!(!f.dir.path().join("native-sessions").exists());
    assert_eq!(prepared.apply(&store).expect("import").imported, 1);
    assert_eq!(prepared.apply(&store).expect("repeat").already_present, 1);
    let request = f.request();
    let history = store.load_for_checked(&request, &f.id).expect("load").expect("imported");
    assert_eq!(history.iter().map(Message::text).collect::<Vec<_>>(), ["old request", "legacy report"]);
    let mut other = request.clone();
    other.credentials.owner = Some(Principal::member(UserId::new(), OrgId::new()));
    assert!(store.load_for_checked(&other, &f.id).expect("other owner").is_none());
    other = request.clone();
    other.cwd = f.exports.clone();
    assert!(store.load_for_checked(&other, &f.id).expect("other project").is_none());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for entry in std::fs::read_dir(f.dir.path().join("native-sessions")).expect("sessions") {
            assert_eq!(entry.expect("entry").metadata().expect("metadata").permissions().mode() & 0o777, 0o600);
        }
    }
}
#[test]
fn malformed_tampered_duplicate_and_conflicting_imports_never_overwrite_history() {
    let f = Fixture::new();
    let mut entry = f.entry();
    entry.sha256 = "0".repeat(64);
    f.write(vec![entry]);
    assert!(migration::prepare(&f.manifest, &f.exports).expect_err("wrong hash").contains("SHA-256"));
    let mut entry = f.entry();
    entry.source = "../outside.jsonl".into();
    f.write(vec![entry]);
    assert!(migration::prepare(&f.manifest, &f.exports).is_err());
    f.write(vec![f.entry(), f.entry()]);
    assert!(migration::prepare(&f.manifest, &f.exports).expect_err("duplicate").contains("repeats"));
    f.write(vec![f.entry()]);
    let prepared = migration::prepare(&f.manifest, &f.exports).expect("prepare");
    let store = f.store();
    prepared.apply(&store).expect("first");
    store.save_for(&f.request(), &f.id, &[Message::user("newer conversation")]).expect("resume saved");
    assert!(prepared.apply(&store).expect_err("conflict").contains("refusing to overwrite"));
    assert_eq!(store.load_for(&f.request(), &f.id), Some(vec![Message::user("newer conversation")]));
}
#[test]
#[cfg(unix)]
fn broad_manifest_permissions_and_export_symlink_escape_are_refused() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let f = Fixture::new();
    std::fs::set_permissions(&f.manifest, std::fs::Permissions::from_mode(0o644)).expect("mode");
    assert!(migration::prepare(&f.manifest, &f.exports).expect_err("permissions").contains("private"));
    let outside = f.dir.path().join("outside.jsonl");
    std::fs::write(&outside, b"not a transcript").expect("outside");
    symlink(&outside, f.exports.join("link.jsonl")).expect("symlink");
    let mut entry = f.entry();
    entry.source = "link.jsonl".into();
    f.write(vec![entry]);
    assert!(migration::prepare(&f.manifest, &f.exports).expect_err("escape").contains("escapes"));
}
struct Source(Arc<ScriptedModel>);
impl ApiSource for Source {
    fn api(&self, _: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        Ok(self.0.clone())
    }
}
#[test]
fn native_engine_resumes_an_imported_member_session_through_its_worker() {
    let f = Fixture::new();
    let store = f.store();
    migration::prepare(&f.manifest, &f.exports).expect("prepare").apply(&store).expect("import");
    let model = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![superfik_native::transcript::Block::Text(
        "continued natively".into(),
    )])]));
    let engine = NativeEngine::new(Arc::new(Source(model.clone())), Arc::new(store))
        .with_worker(Path::new(env!("CARGO_BIN_EXE_superfik-native-worker")).into());
    let (sender, receiver) = std::sync::mpsc::channel();
    let sink = Arc::new(move |_: RunId, event: EngineEvent| {
        let _ = sender.send(event);
    });
    let _handle = engine.start(RunId::new(), &f.request(), sink).expect("start");
    let mut events = Vec::new();
    while let Ok(event) = receiver.recv_timeout(Duration::from_secs(5)) {
        let end = event.is_terminal();
        events.push(event);
        if end {
            break;
        }
    }
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    assert_eq!(
        model.asked()[0].messages.iter().take(2).map(Message::text).collect::<Vec<_>>(),
        ["old request", "legacy report"]
    );
}
#[test]
fn migration_command_validates_by_default_and_applies_only_when_requested() {
    let f = Fixture::new();
    let destination = f.dir.path().join("native-sessions");
    let run = |apply: bool| {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_superfik-session-migrate"));
        command
            .arg("--manifest")
            .arg(&f.manifest)
            .arg("--source-root")
            .arg(&f.exports)
            .arg("--sessions-dir")
            .arg(&destination);
        if apply {
            command.arg("--apply");
        }
        command.output().expect("migration process")
    };
    let checked = run(false);
    assert!(checked.status.success(), "{}", String::from_utf8_lossy(&checked.stderr));
    assert!(!destination.exists());
    let applied = run(true);
    assert!(applied.status.success(), "{}", String::from_utf8_lossy(&applied.stderr));
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&applied.stdout).expect("report")["imported"], 1);
}

#[test]
fn native_exports_import_and_semantically_identical_files_are_idempotent() {
    let f = Fixture::new();
    let history = vec![
        Message::user("native export"),
        Message::assistant(vec![superfik_native::transcript::Block::Thinking {
            text: "old reasoning".into(),
            signature: None,
            origin: None,
        }]),
    ];
    let bytes = serde_json::to_vec_pretty(&history).expect("history");
    std::fs::write(f.exports.join("native.json"), &bytes).expect("export");
    let mut entry = f.entry();
    entry.source = "native.json".into();
    entry.format = SourceFormat::NativeJson;
    entry.sha256 = format!("{:x}", Sha256::digest(bytes));
    f.write(vec![entry]);
    let prepared = migration::prepare(&f.manifest, &f.exports).expect("prepare");
    let store = f.store();
    prepared.apply(&store).expect("apply");
    let sessions = f.dir.path().join("native-sessions");
    let path = std::fs::read_dir(sessions).expect("sessions").next().expect("one").expect("entry").path();
    let mut equivalent = serde_json::to_value(&history).expect("json");
    equivalent[1]["blocks"][0]["Thinking"].as_object_mut().expect("thinking").remove("origin");
    std::fs::write(path, serde_json::to_vec_pretty(&equivalent).expect("pretty")).expect("rewrite representation");
    assert_eq!(prepared.apply(&store).expect("idempotent").already_present, 1);
    assert_eq!(store.load_for(&f.request(), &f.id), Some(history));
}

#[test]
fn legacy_wire_shape_does_not_attest_the_reasoning_provider() {
    let history = superfik_native::sessions::import_jsonl(
        r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"legacy","signature":"opaque"},{"type":"text","text":"answer"}]}}"#,
    ).expect("import");
    assert!(matches!(&history[0].blocks[0], superfik_native::transcript::Block::Thinking { origin: None, .. }));
    assert_eq!(history[0].text(), "answer");
}
