use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use agent_engine::Engine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::{Entry, FileSessions, INTERRUPTED, Sessions};
use agent_engine::transcript::{Block, Message, Role};
use domain::{EngineEvent, PromptRequest, SessionId};
use serde_json::json;

use crate::{Source, finish, model, start};

struct DiesAfter {
    kept: FileSessions,
    saves: AtomicUsize,
    dies_after: usize,
    dead: AtomicBool,
}

impl DiesAfter {
    fn saves(store: &Path, dies_after: usize) -> Arc<Self> {
        let kept = FileSessions::new(store);
        Arc::new(Self { kept, saves: AtomicUsize::new(0), dies_after, dead: AtomicBool::new(false) })
    }

    fn is_dead(&self) -> bool {
        self.dead.load(Ordering::SeqCst)
    }
}

impl Sessions for DiesAfter {
    fn load(&self, id: &SessionId) -> Option<Vec<Message>> {
        self.kept.load(id)
    }

    fn save(&self, id: &SessionId, history: &[Message]) -> Result<(), String> {
        if self.is_dead() {
            return Err("the process is gone".into());
        }
        self.kept.save(id, history)?;
        if self.saves.fetch_add(1, Ordering::SeqCst) + 1 == self.dies_after {
            self.dead.store(true, Ordering::SeqCst);
            panic!("the process died after save {}", self.dies_after);
        }
        Ok(())
    }

    fn record(&self, cwd: &std::path::Path, id: &SessionId, entry: &Entry) -> Result<(), String> {
        if self.is_dead() {
            return Ok(());
        }
        self.kept.record(cwd, id, entry)
    }

    fn flush_trace(&self, cwd: &std::path::Path, id: &SessionId) {
        if !self.is_dead() {
            self.kept.flush_trace(cwd, id);
        }
    }
}

struct Crashed {
    dir: tempfile::TempDir,
    store: PathBuf,
    session: SessionId,
}

fn read_then_answer() -> Arc<ScriptedModel> {
    Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![
            Block::Text("Reading.".into()),
            Block::ToolUse { id: "r1".into(), name: "Read".into(), input: json!({"file_path": "notes.txt"}) },
        ]),
        ScriptedModel::reply(vec![Block::Text("It says alpha.".into())]),
    ]))
}

fn crashed_after(saves: usize) -> Crashed {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("notes.txt"), "alpha\n").expect("file");
    let store = dir.path().join("native");
    let dying = DiesAfter::saves(&store, saves);
    let engine = Engine::new(Arc::new(Source(Mutex::new(vec![read_then_answer()]))), dying.clone());
    let events = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "read the notes", model())));
    assert!(dying.is_dead(), "the run was meant to die: {events:?}");
    assert!(matches!(events.last(), Some(EngineEvent::Failed(_))), "{events:?}");
    let session = events
        .iter()
        .find_map(|event| if let EngineEvent::SessionStarted(id) = event { Some(id.clone()) } else { None })
        .expect("a session");
    Crashed { dir, store, session }
}

fn resumed(crashed: &Crashed) -> (Vec<Message>, Vec<Message>) {
    let later = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Going on.".into())])]));
    let sessions = Arc::new(FileSessions::new(&crashed.store));
    let engine = Engine::new(Arc::new(Source(Mutex::new(vec![later.clone()]))), sessions.clone());
    let mut request = PromptRequest::plain(crashed.dir.path().into(), "go on", model());
    request.session.resume = Some(crashed.session.clone());
    let on_disk = sessions.load_for_checked(&request.session.cwd, &crashed.session).expect("readable").expect("kept");
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    (on_disk, later.asked()[0].messages.clone())
}

fn shape(history: &[Message]) -> Vec<String> {
    history
        .iter()
        .map(|message| {
            let blocks: Vec<String> = message
                .blocks
                .iter()
                .filter_map(|block| match block {
                    Block::Text(text) => Some(format!("text:{text}")),
                    Block::ToolUse { id, .. } => Some(format!("call:{id}")),
                    Block::ToolResult { id, content, is_error } => Some(format!("result:{id}:{is_error}:{content}")),
                    _ => None,
                })
                .collect();
            format!("{:?}[{}]", message.role, blocks.join("|"))
        })
        .collect()
}

#[test]
fn a_run_that_dies_before_the_first_model_call_has_kept_the_prompt() {
    let crashed = crashed_after(1);
    let (on_disk, asked) = resumed(&crashed);
    assert_eq!(shape(&on_disk), ["User[text:read the notes]"]);
    assert_eq!(shape(&asked), ["User[text:read the notes]", "User[text:go on]"]);
}

#[test]
fn a_run_that_dies_after_a_reply_with_a_tool_call_resumes_with_the_call_closed_as_interrupted() {
    let crashed = crashed_after(2);
    let (on_disk, asked) = resumed(&crashed);
    assert_eq!(shape(&on_disk), ["User[text:read the notes]", "Assistant[text:Reading.|call:r1]"]);
    assert_eq!(
        shape(&asked),
        [
            "User[text:read the notes]".to_owned(),
            "Assistant[text:Reading.|call:r1]".to_owned(),
            format!("User[result:r1:true:{INTERRUPTED}|text:go on]"),
        ]
    );
}

#[test]
fn a_run_that_dies_after_the_tool_results_has_kept_them() {
    let crashed = crashed_after(3);
    let (on_disk, asked) = resumed(&crashed);
    let with_results =
        ["User[text:read the notes]", "Assistant[text:Reading.|call:r1]", "User[result:r1:false:     1\talpha\n]"];
    assert_eq!(shape(&on_disk), with_results);
    assert_eq!(shape(&asked)[..2], with_results[..2]);
    assert_eq!(shape(&asked)[2], "User[result:r1:false:     1\talpha\n|text:go on]");
    assert_eq!(asked.len(), 3);
}

#[test]
fn a_conversation_resumed_after_a_crash_is_whole_on_disk_again() {
    let crashed = crashed_after(2);
    resumed(&crashed);
    let sessions = FileSessions::new(&crashed.store);
    let request = PromptRequest::plain(crashed.dir.path().into(), "", model());
    let kept = sessions.load_for_checked(&request.session.cwd, &crashed.session).expect("readable").expect("kept");
    assert_eq!(agent_engine::sessions::repaired(kept.clone()), kept, "no call is left without its result");
    assert_eq!(kept.last().map(|last| (last.role, last.text())), Some((Role::Assistant, "Going on.".to_owned())));
}

#[test]
fn a_run_of_many_turns_writes_each_message_once_and_never_reads_the_file_back() {
    const TURNS: usize = 40;
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("notes.txt"), "alpha\n".repeat(200)).expect("file");
    let read = |at: usize| {
        let call =
            Block::ToolUse { id: format!("r{at}"), name: "Read".into(), input: json!({"file_path": "notes.txt"}) };
        ScriptedModel::reply(vec![call])
    };
    let replies = (0..TURNS).map(read).chain([ScriptedModel::reply(vec![Block::Text("Done.".into())])]);
    let store = dir.path().join("native");
    let sessions = Arc::new(FileSessions::new(&store));
    let engine =
        Engine::new(Arc::new(Source(Mutex::new(vec![Arc::new(ScriptedModel::new(replies))]))), sessions.clone());
    let mut request = PromptRequest::plain(dir.path().into(), "read it many times", model());
    request.policy.max_turns = Some(100);
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");

    let written = sessions.written();
    let transitions = 1 + 2 * TURNS as u64 + 1;
    assert_eq!(written.appends, transitions, "the prompt, each reply, each batch of results, the answer");
    assert_eq!(written.syncs, transitions, "one sync per transition");
    assert!(written.rereads <= 1, "the conversation file is not read back at every save: {written:?}");
    let conversation = std::fs::read_dir(&store)
        .expect("store")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            name.ends_with(".jsonl") && !name.contains(".trace.")
        })
        .expect("the conversation file");
    let on_disk = std::fs::metadata(&conversation).expect("file").len();
    assert_eq!(written.bytes, on_disk, "every byte was written once: the cost grows with the turns, not their square");
    let lines = std::fs::read_to_string(&conversation).expect("text").lines().count() as u64;
    assert_eq!(lines, transitions, "one message per transition");
}
