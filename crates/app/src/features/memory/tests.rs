use crate::api::features::memory::{MemoryOp, MemoryReply, MemoryRequest};
use domain::{Forgotten, MemoryKind, ModelOption};

use super::*;
use crate::dirs::project_key;
use crate::features::context::Context;

fn plain() -> Memories {
    Memories::new(Arc::new(InMemoryStore::default()))
}

fn listed(answer: &MemoryListing) -> Vec<Memory> {
    answer.memories.clone()
}

fn forgotten(answer: &MemoryListing) -> Vec<Forgotten> {
    answer.forgotten.clone()
}

fn request(cwd: &Path) -> PromptRequest {
    PromptRequest::plain(cwd.to_path_buf(), "Tune the renderer", ModelOption::new("opus", "Opus"))
        .remembering(MemoryBrief::new(Some("Night sky".into())))
}

fn remember(text: &str) -> MemoryEdit {
    MemoryEdit::Remember { kind: MemoryKind::Semantic, text: text.into(), tags: vec!["render".into()], touches: vec![] }
}

fn by_user() -> Provenance {
    Provenance { author: Author::User, now: 5, origin: None }
}

fn ended(completed: bool) -> domain::memory::scorecard::RunOutcome {
    domain::memory::scorecard::RunOutcome { completed, ..Default::default() }
}

#[test]
fn every_twentieth_request_goes_without_memory_and_the_score_compares_both() {
    let dir = tempfile::tempdir().expect("tempdir");
    let memories = plain();
    memories.edit(dir.path(), &[remember("The renderer uses wgpu")], by_user()).expect("edit");
    let mut held = Vec::new();
    for n in 1..=20 {
        let briefed = memories.brief(RunId::new(), &request(dir.path()), false);
        let brief = briefed.host.memory.clone().expect("brief");
        if brief.held_out {
            held.push(n);
            assert!(brief.recalled.is_empty(), "nothing recalled when held out");
        } else {
            assert!(!brief.recalled.is_empty());
        }
        let corrections = u32::from(brief.held_out);
        let outcome = domain::memory::scorecard::RunOutcome { corrections, ..ended(true) };
        memories.finish(&briefed, &outcome);
    }
    assert_eq!(held, [20]);
    let report = memories.score(dir.path());
    assert!(report.contains("With memory: 19 runs, 0.0 corrections"), "{report}");
    assert!(report.contains("Without memory: 1 runs, 100.0 corrections"), "{report}");
}

#[test]
fn project_keys_are_flat_file_names() {
    assert_eq!(project_key(Path::new("/Users/me/sky.app")), "-Users-me-sky-app");
}

fn on_disk(data: &Path) -> Memories {
    crate::DataRoot::under(data).stores().memories
}

#[test]
fn memory_lives_in_the_data_folder_s_database_or_only_in_the_process() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = Path::new("/repo");

    on_disk(dir.path()).edit(project, &[remember("Uses wgpu")], by_user()).expect("edit");
    assert!(dir.path().join("app.db").is_file());
    assert!(!dir.path().join("memory").exists(), "no file per project any more");
    assert_eq!(listed(&on_disk(dir.path()).load(project)).len(), 1, "still there for the next run of the program");

    let root = crate::DataRoot::in_memory();
    let in_process = root.stores().memories;
    in_process.edit(project, &[remember("Uses wgpu")], by_user()).expect("edit");
    assert_eq!(listed(&in_process.load(project)).len(), 1, "kept for the process");
    assert!(root.folders().iter().all(|folder| !folder.exists()), "never in a temp folder");
    assert!(listed(&root.stores().memories.load(project)).is_empty(), "and each such root has its own");
}

#[test]
fn edits_answer_with_the_whole_memory() {
    let memories = plain();
    memories.edit(Path::new("/repo"), &[remember("Uses wgpu")], by_user()).expect("edit");
    let answer = memories.edit(Path::new("/repo"), &[remember("Inter is the only font")], by_user());
    let answer = answer.expect("edit");
    assert_eq!(answer.path, Path::new("/repo"));
    assert_eq!(listed(&answer).len(), 2);
}

#[test]
fn a_run_is_briefed_and_its_edits_applied_on_completion() {
    let dir = tempfile::tempdir().expect("tempdir");
    let memories = plain();
    memories.edit(dir.path(), &[remember("The renderer uses signed distance fields")], by_user()).expect("edit");

    let briefed = memories.brief(RunId::new(), &request(dir.path()), false);
    let brief = briefed.host.memory.clone().expect("brief");
    assert_eq!(brief.recalled.len(), 1);
    assert_eq!(brief.origin.as_deref(), Some("Night sky"));
    assert!(!brief.output.as_os_str().is_empty());

    std::fs::write(&brief.output, r#"{"remember":[{"kind":"episodic","text":"Tuned the renderer"}],"forget":[0]}"#)
        .expect("write");
    let applied = memories.finish(&briefed, &ended(true)).expect("changed").expect("saved");
    assert_eq!((applied.optimizing, applied.added, applied.changed, applied.forgotten), (false, 1, 0, 1));
    let learned = listed(&applied.listing);
    assert_eq!(learned.len(), 1);
    assert_eq!((learned[0].kind, learned[0].author), (MemoryKind::Episodic, Author::Agent));
    assert_eq!(learned[0].origin.as_deref(), Some("Night sky"));
    assert!(!brief.output.exists(), "the edits file is removed");
}

#[test]
fn what_an_agent_is_given_counts_as_used() {
    let dir = tempfile::tempdir().expect("tempdir");
    let memories = plain();
    let font = MemoryEdit::Remember {
        kind: MemoryKind::Semantic,
        text: "Inter is the font".into(),
        tags: vec![],
        touches: vec![],
    };
    memories.edit(dir.path(), &[remember("The renderer uses wgpu"), font], by_user()).expect("edit");
    let uses = || listed(&memories.load(dir.path())).iter().map(|m| m.uses).collect::<Vec<_>>();

    memories.brief(RunId::new(), &request(dir.path()), false);
    assert_eq!(uses(), vec![1, 1], "everything recalled");
    memories.find(dir.path(), "wgpu", 5, &mut MemorySearches::default());
    assert_eq!(uses(), vec![2, 1], "only what the search found");
    memories.find(dir.path(), "wgpu", 5, &mut MemorySearches::new([MemoryId(0)]));
    assert_eq!(uses(), vec![2, 1], "what was already shown is not counted again");
}

#[test]
fn a_failed_run_changes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let memories = plain();
    let briefed = memories.brief(RunId::new(), &request(dir.path()), false);
    let output = briefed.host.memory.clone().expect("brief").output;
    std::fs::write(&output, r#"{"remember":[{"kind":"semantic","text":"Half done"}]}"#).expect("write");
    assert_eq!(memories.finish(&briefed, &ended(false)), None);
    assert!(!output.exists());
    let mut no_memory = request(dir.path());
    no_memory.host.memory = None;
    assert_eq!(memories.brief(RunId::new(), &no_memory, false), no_memory);
}

#[test]
fn an_optimisation_can_be_undone_until_the_next_edit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let memories = plain();
    let both = [remember("Uses wgpu"), remember("The renderer uses wgpu")];
    memories.edit(dir.path(), &both, by_user()).expect("edit");
    let request = memories.optimization(dir.path(), ModelOption::new("opus", "Opus"));
    assert!(request.prompt.text.contains("[0] semantic (render), from the user: Uses wgpu"));
    let briefed = memories.brief(RunId::new(), &request, false);
    let brief = briefed.host.memory.clone().expect("brief");
    assert!(brief.optimizing && brief.recalled.is_empty() && !brief.tool);

    std::fs::write(&brief.output, r#"{"forget":[0]}"#).expect("write");
    let optimized = memories.finish(&briefed, &ended(true)).expect("changed").expect("saved");
    assert_eq!((optimized.optimizing, optimized.added, optimized.forgotten), (true, 0, 1));
    let optimized = optimized.listing;
    assert_eq!(listed(&optimized).len(), 1);
    assert_eq!(forgotten(&optimized).iter().map(|f| f.memory.id).collect::<Vec<_>>(), vec![MemoryId(0)]);
    let undone = memories.undo(dir.path()).expect("undone");
    assert_eq!(listed(&undone).len(), 2, "both memories are back");
    assert!(forgotten(&undone).is_empty());
    assert!(memories.undo(dir.path()).is_err(), "undone only once");

    let briefed = memories.brief(RunId::new(), &request, false);
    std::fs::write(&briefed.host.memory.clone().expect("brief").output, r#"{"forget":[1]}"#).expect("write");
    let _ = memories.finish(&briefed, &ended(true));
    memories.edit(dir.path(), &[remember("Inter is the font")], by_user()).expect("edit");
    assert!(memories.undo(dir.path()).is_err(), "an edit ends the undo");
}

#[test]
fn a_tool_shortens_what_is_recalled() {
    let memories = plain();
    let dir = tempfile::tempdir().expect("tempdir");
    let edits: Vec<MemoryEdit> = (0..30).map(|n| remember(&format!("Fact number {n}"))).collect();
    memories.edit(dir.path(), &edits, by_user()).expect("edit");
    let with = memories.brief(RunId::new(), &request(dir.path()), true).host.memory.expect("brief");
    let without = memories.brief(RunId::new(), &request(dir.path()), false).host.memory.expect("brief");
    assert_eq!((with.recalled.len(), without.recalled.len()), (RecallBudget::WITH_TOOL.facts, 30));
    assert!(with.tool);
}

#[test]
fn memory_that_fits_in_the_prompt_offers_no_search() {
    let memories = plain();
    let dir = tempfile::tempdir().expect("tempdir");
    let empty = memories.brief(RunId::new(), &request(dir.path()), true).host.memory.expect("brief");
    assert!(!empty.tool, "an empty memory has nothing to search");
    memories.edit(dir.path(), &[remember("Uses wgpu")], by_user()).expect("edit");
    let small = memories.brief(RunId::new(), &request(dir.path()), true).host.memory.expect("brief");
    assert_eq!((small.recalled.len(), small.tool), (1, false), "everything is already recalled");
}

#[test]
fn the_feature_keeps_memory_in_the_data_folder_and_starts_optimisations() {
    let data = tempfile::tempdir().expect("data");
    let project = tempfile::tempdir().expect("project");
    let services = crate::features::context::testing::services(Some(data.path()));
    let asking = |op: MemoryOp| MemoryRequest { path: project.path().to_path_buf(), op };
    crate::features::context::testing::with(1, &services, |context| {
        let edited = handle(context, asking(MemoryOp::Edit { edits: vec![remember("Uses wgpu")] })).expect("edited");
        assert!(matches!(&edited, MemoryReply::Listing(listing) if listing.memories.len() == 1), "{edited:?}");
        let found = handle(context, asking(MemoryOp::Search { query: "wgpu".into() })).expect("found");
        assert!(matches!(&found, MemoryReply::Found { scores, .. } if scores[0].0 == MemoryId(0)), "{found:?}");
        assert!(handle(context, asking(MemoryOp::Undo)).is_err());
    });
    assert!(data.path().join("app.db").is_file());

    let started = std::sync::Mutex::new(Vec::new());
    let submit = |run: RunId, request: PromptRequest| {
        started.lock().expect("lock").push((run, request));
        Ok(())
    };
    let context = Context { client: 1, services: &services, submit: &submit };
    let run = RunId::new();
    let optimize = asking(MemoryOp::Optimize { run, model: ModelOption::new("m", "M") });
    assert_eq!(handle(&context, optimize).expect("started"), MemoryReply::Optimizing { run });
    let started = started.into_inner().expect("lock");
    assert_eq!(started[0].0, run);
    assert!(started[0].1.host.memory.as_ref().is_some_and(|brief| brief.optimizing));
}

#[test]
fn two_processes_on_one_project_keep_each_others_memories() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (first, second) = (on_disk(dir.path()), on_disk(dir.path()));
    let project = Path::new("/repo");
    second.load(project);
    first.edit(project, &[remember("Uses wgpu")], by_user()).expect("first");
    let answer = second.edit(project, &[remember("Ships on Linux")], by_user()).expect("second");
    let texts: Vec<String> = listed(&answer).into_iter().map(|memory| memory.text).collect();
    assert_eq!(texts, ["Uses wgpu", "Ships on Linux"]);
}

#[test]
fn what_another_process_changed_is_read_not_what_this_one_remembers_reading() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (here, elsewhere) = (on_disk(dir.path()), on_disk(dir.path()));
    let project = Path::new("/repo");
    here.edit(project, &[remember("Uses wgpu")], by_user()).expect("here");
    assert_eq!(listed(&here.load(project)).len(), 1);

    elsewhere.edit(project, &[remember("Ships on Linux")], by_user()).expect("elsewhere");

    assert_eq!(listed(&here.load(project)).len(), 2, "the list");
    assert_eq!(here.search(project, "linux").first().map(|hit| hit.0), Some(MemoryId(1)), "the search");
    let found = here.find(project, "linux", 5, &mut MemorySearches::default());
    assert!(matches!(&found, SearchAnswer::Found(found) if found[0].text == "Ships on Linux"), "{found:?}");
    let recalled = here.brief(RunId::new(), &request(project), false).host.memory.expect("brief").recalled;
    assert_eq!(recalled.len(), 2, "what a run is briefed with");
    let used =
        |memories: &Memories| listed(&memories.load(project)).iter().map(|memory| memory.uses).collect::<Vec<_>>();
    assert_eq!(used(&here), used(&elsewhere), "and both count the same uses");
    assert!(used(&here).iter().all(|uses| *uses > 0), "{:?}", used(&here));

    elsewhere.edit(project, &[MemoryEdit::Forget { id: MemoryId(0) }], by_user()).expect("forget");
    assert_eq!(forgotten(&here.load(project)).len(), 1);
    assert!(here.search(project, "wgpu").iter().all(|hit| hit.0 != MemoryId(0)));
}

#[test]
fn an_optimisation_can_still_be_undone_after_the_program_restarts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = tempfile::tempdir().expect("project");
    let before = on_disk(dir.path());
    before.edit(project.path(), &[remember("Uses wgpu"), remember("The renderer uses wgpu")], by_user()).expect("edit");
    let request = before.optimization(project.path(), ModelOption::new("opus", "Opus"));
    let briefed = before.brief(RunId::new(), &request, false);
    std::fs::write(&briefed.host.memory.clone().expect("brief").output, r#"{"forget":[0]}"#).expect("write");
    before.finish(&briefed, &ended(true)).expect("changed").expect("saved");
    drop(before);

    let restarted = on_disk(dir.path());
    assert_eq!(listed(&restarted.load(project.path())).len(), 1, "optimised");
    let undone = restarted.undo(project.path()).expect("undone");

    assert_eq!(listed(&undone).len(), 2, "both memories are back");
    assert!(on_disk(dir.path()).undo(project.path()).is_err(), "undone only once, whichever process asks");
    restarted.brief(RunId::new(), &request, false);
    on_disk(dir.path()).edit(project.path(), &[remember("Inter is the font")], by_user()).expect("edit");
    assert!(restarted.undo(project.path()).is_err(), "an edit ends the undo, whichever process made it");
}
