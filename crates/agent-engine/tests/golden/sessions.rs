use agent_engine::sessions::Sessions;
use agent_engine::transcript::{Block, Message, Role};
use domain::{SessionCommand, SessionId};
use serde_json::json;

use crate::models::{call, no_findings, says};
use crate::stage::Stage;

#[test]
fn a_session_cut_between_a_call_and_its_result_resumes_with_the_call_closed_as_interrupted() {
    let stage = Stage::new();
    let session = SessionId("crashed".into());
    let dangling = Block::ToolUse { id: "t1".into(), name: "Read".into(), input: json!({"file_path": "notes.txt"}) };
    let crashed = [Message::user("Read the notes"), Message::assistant(vec![dangling])];
    stage.sessions.save_for(&stage.request("").session.cwd, &session, &crashed).expect("the crash-shaped session");
    stage.saved("as the crash left it", &stage.request(""), &session);
    let model = stage.script([says("Picking up where we stopped.")]);

    stage.play("resumed prompt", &stage.resuming(&session, "Go on"));

    let roles: Vec<_> = model.asked()[0].messages.iter().map(|message| message.role).collect();
    assert_eq!(roles, [Role::User, Role::Assistant, Role::User], "the roles alternate");
    stage.assert(
        r##"
        == as the crash left it: conversation ==
        user:
          text "Read the notes"
        assistant:
          tool_use t1 Read "{\"file_path\":\"notes.txt\"}"
        == as the crash left it: log appended ==
        message user
        message assistant
        == resumed prompt: events ==
        SessionStarted <session-1>
        Checkpoint(0)
        Text "Picking up where we stopped."
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Completed duration=<ms> cost=none
        == resumed prompt: conversation ==
        user:
          text "Read the notes"
        assistant:
          tool_use t1 Read "{\"file_path\":\"notes.txt\"}"
        user:
          tool_result t1 failed "This tool call was interrupted before it ran."
          text "Go on"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Picking up where we stopped."
        == resumed prompt: log appended ==
        prompted "Go on"
        event SessionStarted
        opened
        event Checkpoint
        message user
        event TextDelta
        called model=claude-opus-5-5 stop=EndTurn messages=3
        event Usage
        message assistant
        event Completed
        "##,
    );
}

fn two_prompts_that_write(stage: &Stage) -> SessionId {
    let writes = |file: &'static str| {
        [call("w1", "Write", json!({"file_path": file, "content": "written\n"})), says("Written."), no_findings()]
    };
    stage.script(writes("notes.txt"));
    stage.script(writes("more.txt"));
    let session = stage.prepare(&stage.request("Write the notes"));
    stage.prepare(&stage.resuming(&session, "Add more"));
    session
}

fn files(stage: &Stage) -> Vec<String> {
    ["notes.txt", "more.txt"]
        .iter()
        .map(|name| format!("{name}: {}", if stage.project.join(name).exists() { "present" } else { "absent" }))
        .collect()
}

#[test]
fn a_rewind_restores_the_files_cuts_the_conversation_and_drops_later_checkpoints() {
    let stage = Stage::new();
    let session = two_prompts_that_write(&stage);
    stage.checkpoints("before the rewind", &stage.request(""), &session);

    let mut rewind = stage.resuming(&session, "");
    rewind.session.command = Some(SessionCommand::Rewind(1));
    stage.play("rewind to the second prompt", &rewind);
    stage.checkpoints("after the rewind", &stage.request(""), &session);
    stage.note("files after the rewind", files(&stage));

    stage.assert(
        r##"
        == before the rewind: checkpoints ==
        0: prompt="Write the notes" history_len=0 files=[<project>/notes.txt]
        1: prompt="Add more" history_len=4 files=[<project>/more.txt]
        == rewind to the second prompt: events ==
        Rewound(Rewound { checkpoint: 1, prompt: "Add more", restored: ["<project>/more.txt"], conversation: true, skipped: [] })
        Completed duration=<ms> cost=none
        == rewind to the second prompt: conversation ==
        user:
          text "Write the notes"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use w1 Write "{\"content\":\"written\\n\",\"file_path\":\"notes.txt\"}"
        user:
          tool_result w1 ok "Created notes.txt (1 lines)."
        assistant:
          text "Written."
        == rewind to the second prompt: log appended ==
        opened
        truncated keep=4
        event Rewound
        event Completed
        == after the rewind: checkpoints ==
        0: prompt="Write the notes" history_len=0 files=[<project>/notes.txt]
        == files after the rewind ==
        notes.txt: present
        more.txt: absent
        "##,
    );
}

#[test]
fn a_branch_starts_a_new_session_cut_before_the_prompt_and_leaves_the_original_alone() {
    let stage = Stage::new();
    let session = two_prompts_that_write(&stage);

    let mut branch = stage.resuming(&session, "");
    branch.session.command = Some(SessionCommand::Branch(1));
    let branched = stage.play("branch at the second prompt", &branch);
    assert_ne!(branched, session);
    stage.checkpoints("the branch", &stage.request(""), &branched);
    stage.saved("the original", &stage.request(""), &session);
    stage.checkpoints("the original", &stage.request(""), &session);
    stage.note("files after the branch", files(&stage));

    stage.assert(
        r##"
        == branch at the second prompt: events ==
        SessionStarted <session-2>
        Branched { from: SessionId("<session-1>"), checkpoint: 1, prompt: "Add more" }
        Completed duration=<ms> cost=none
        == branch at the second prompt: conversation ==
        user:
          text "Write the notes"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use w1 Write "{\"content\":\"written\\n\",\"file_path\":\"notes.txt\"}"
        user:
          tool_result w1 ok "Created notes.txt (1 lines)."
        assistant:
          text "Written."
        == branch at the second prompt: log appended ==
        message user
        message assistant
        message user
        message assistant
        event SessionStarted
        opened
        event Branched
        event Completed
        == the branch: checkpoints ==
        0: prompt="Write the notes" history_len=0 files=[<project>/notes.txt]
        == the original: conversation ==
        user:
          text "Write the notes"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use w1 Write "{\"content\":\"written\\n\",\"file_path\":\"notes.txt\"}"
        user:
          tool_result w1 ok "Created notes.txt (1 lines)."
        assistant:
          text "Written."
        user:
          text "Add more"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          tool_use w1 Write "{\"content\":\"written\\n\",\"file_path\":\"more.txt\"}"
        user:
          tool_result w1 ok "Created more.txt (1 lines)."
        assistant:
          text "Written."
        == the original: log appended ==
        (nothing)
        == the original: checkpoints ==
        0: prompt="Write the notes" history_len=0 files=[<project>/notes.txt]
        1: prompt="Add more" history_len=4 files=[<project>/more.txt]
        == files after the branch ==
        notes.txt: present
        more.txt: present
        "##,
    );
}

#[test]
fn a_title_chore_runs_while_the_session_is_held_and_leaves_the_conversation_as_it_was() {
    let stage = Stage::new();
    stage.script([says("Hello.")]);
    let titler = stage.script([says("{\"title\": \"Greeting\"}")]);
    let session = stage.prepare(&stage.request("Say hello"));

    let mut title = stage.resuming(&session, "Say hello");
    title.session.command = Some(SessionCommand::Title);
    let answering = stage.sessions.hold(&title.session.cwd, &session).expect("another run holds the session");
    assert!(answering.held());
    stage.play("title chore", &title);
    drop(answering);

    assert!(titler.asked()[0].tools.is_empty());
    stage.assert(
        r##"
        == title chore: events ==
        Usage model=claude-opus-5-5 input=10 output=5 cost=none
        Chore(Title("Greeting"))
        Completed duration=<ms> cost=none
        == title chore: conversation ==
        user:
          text "Say hello"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Hello."
        == title chore: log appended ==
        (nothing)
        "##,
    );
}

#[test]
fn a_prompt_for_a_session_that_is_answering_elsewhere_is_turned_away_and_writes_nothing() {
    let stage = Stage::new();
    stage.script([says("Hello.")]);
    let session = stage.prepare(&stage.request("Say hello"));

    let again = stage.resuming(&session, "Again");
    let answering = stage.sessions.hold(&again.session.cwd, &session).expect("another run holds the session");
    stage.play("second prompt", &again);
    drop(answering);

    stage.assert(
        r##"
        == second prompt: events ==
        Failure(Other)
        Failed("This conversation is answering in another window. Wait for it to finish, then send again.")
        == second prompt: conversation ==
        user:
          text "Say hello"
          reminder "# Runtime reminder\nEdits are allowed; commands and external actions still require approval.\nCollect background agent reports before completing. Use TaskStop to cancel…
        assistant:
          text "Hello."
        == second prompt: log appended ==
        (nothing)
        "##,
    );
}

#[test]
fn a_prompt_for_a_session_that_does_not_exist_is_diagnosed_and_fails() {
    let stage = Stage::new();

    stage.play("prompt", &stage.resuming(&SessionId("gone".into()), "Anyone?"));

    stage.assert(
        r##"
        == prompt: events ==
        Diagnosed(SessionNotFound)
        Failure(Other)
        Failed("The conversation to continue was not found.")
        == prompt: conversation ==
        (nothing kept)
        == prompt: log appended ==
        (nothing)
        "##,
    );
}
