use std::sync::Arc;

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message};
use domain::{Decision, EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

fn code_mode(mode: PermissionMode) -> Rig {
    code_mode_as(mode, domain::code_mode::Mode::Beside)
}

fn code_mode_as(mode: PermissionMode, code: domain::code_mode::Mode) -> Rig {
    let mut rig = Rig::new(mode);
    rig.workspace.policy.code_mode = code;
    rig.workspace.session_id = format!("repl-loop-{}", uuid_like(&rig));
    rig
}

fn uuid_like(rig: &Rig) -> String {
    rig.dir.path().file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

fn scripts(rig: &Rig, codes: &[&str]) -> Vec<Message> {
    let mut replies: Vec<_> = codes
        .iter()
        .enumerate()
        .map(|(n, code)| ScriptedModel::reply(vec![tool_use(&format!("r{n}"), "REPL", json!({ "code": code }))]))
        .collect();
    replies.push(ScriptedModel::reply(vec![Block::Text("Done.".into())]));
    let model = ScriptedModel::new(replies);
    let mut history = vec![Message::user("go")];
    rig.run_with(&model, &mut history, vec![Box::new(agent_engine::tools::repl::Repl)]).expect("run");
    history
}

fn results(history: &[Message]) -> Vec<(String, bool)> {
    history
        .iter()
        .flat_map(|message| message.blocks.iter())
        .filter_map(|block| match block {
            Block::ToolResult { content, is_error, .. } => Some((content.clone(), is_error.to_owned())),
            _ => None,
        })
        .collect()
}

fn started(rig: &Rig) -> Vec<String> {
    rig.events()
        .into_iter()
        .filter_map(|event| match event {
            EngineEvent::ToolStarted(call) => Some(call.name),
            _ => None,
        })
        .collect()
}

#[test]
fn a_script_calls_tools_and_only_what_it_returns_reaches_the_model() {
    let rig = code_mode(PermissionMode::AcceptEdits);
    let code = "const [text, found] = await Promise.all([Read({ file_path: 'notes.txt' }), \
                Grep({ pattern: 'beta', path: '.', output_mode: 'files_with_matches' })]);\n\
                self.log('read', text.length > 0);\n\
                ({ lines: text.includes('alpha'), grep: found.includes('notes.txt') })";

    let history = scripts(&rig, &[code]);

    let results = results(&history);
    assert_eq!(results.len(), 1, "the inner calls stay out of the transcript: {results:?}");
    let (content, failed) = &results[0];
    assert!(!failed, "{content}");
    assert!(content.contains("<log>\nread true\n</log>"), "{content}");
    assert!(content.contains("\"lines\": true") && content.contains("\"grep\": true"), "{content}");
    let started = started(&rig);
    assert!(started.contains(&"Read".to_owned()) && started.contains(&"Grep".to_owned()), "{started:?}");
}

#[test]
fn in_code_only_mode_the_model_sees_the_repl_and_reaches_the_rest_from_scripts() {
    let rig = code_mode_as(PermissionMode::AcceptEdits, domain::code_mode::Mode::Only);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use(
            "r0",
            "REPL",
            json!({ "code": "const listed = await $`ls`; listed.includes('notes.txt')" }),
        )]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    let mut history = vec![Message::user("go")];

    rig.run_with(&model, &mut history, vec![Box::new(agent_engine::tools::repl::Repl)]).expect("run");

    let offered: Vec<String> = model.asked()[0].tools.iter().map(|spec| spec.name.clone()).collect();
    assert!(offered.contains(&"REPL".to_owned()) && offered.contains(&"AskUserQuestion".to_owned()), "{offered:?}");
    assert!(!offered.iter().any(|name| ["Read", "Bash", "Grep", "Edit"].contains(&name.as_str())), "{offered:?}");
    let repl = model.asked()[0].tools.iter().find(|spec| spec.name == "REPL").map(|spec| spec.description.clone());
    assert!(repl.is_some_and(|text| text.contains("declare function Bash(input: {")));
    assert_eq!(results(&history)[0], ("<result>\ntrue\n</result>".to_owned(), false));
}

#[test]
fn search_tools_finds_deferred_tools_and_returns_their_declarations() {
    let mut rig = code_mode_as(PermissionMode::AcceptEdits, domain::code_mode::Mode::Only);
    let specs = (0..25).map(|n| agent_engine::model::ToolSpec {
        name: format!("mcp__tracker__tool_{n}"),
        description: if n == 7 { "Create an issue in the tracker.".into() } else { format!("Other thing {n}.") },
        schema: json!({ "type": "object", "properties": { "title": { "type": "string" } }, "required": ["title"] }),
    });
    let (_, deferred) = agent_engine::tools::tool_search::defer(specs.collect());
    rig.workspace.services.deferred = deferred.map(Arc::new);
    let code = "const found = await searchTools('create issue'); \
                [found.length, found[0].name, found[0].declaration.includes('title: string')]";

    let history = scripts(&rig, &[code]);

    let (content, failed) = &results(&history)[0];
    assert!(!failed, "{content}");
    assert!(content.contains("\"mcp__tracker__tool_7\"") && content.contains("true"), "{content}");
    let loaded =
        rig.workspace.services.deferred.as_ref().is_some_and(|deferred| deferred.is_loaded("mcp__tracker__tool_7"));
    assert!(loaded, "a found tool can be called from then on");
}

#[test]
fn reads_in_a_script_do_not_count_as_reads_the_model_has_seen() {
    let rig = code_mode(PermissionMode::AcceptEdits);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use(
            "r0",
            "REPL",
            json!({ "code": "const a = await Read({ file_path: 'notes.txt' }); \
                             const b = await Read({ file_path: 'notes.txt' }); a === b && a.includes('alpha')" }),
        )]),
        ScriptedModel::reply(vec![tool_use("d0", "Read", json!({ "file_path": "notes.txt" }))]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    let mut history = vec![Message::user("go")];

    rig.run_with(&model, &mut history, vec![Box::new(agent_engine::tools::repl::Repl)]).expect("run");

    let results = results(&history);
    assert_eq!(results[0], ("<result>\ntrue\n</result>".to_owned(), false), "a script gets the text every time");
    assert!(results[1].0.contains("alpha"), "the model's own read shows the text: {:?}", results[1]);
}

#[test]
fn a_script_after_earlier_ones_in_a_new_context_is_told_their_values_are_gone() {
    let rig = code_mode(PermissionMode::AcceptEdits);
    let mut history = vec![
        Message::user("go"),
        Message::assistant(vec![tool_use("before", "REPL", json!({ "code": "const kept = 1" }))]),
        Message {
            role: agent_engine::transcript::Role::User,
            blocks: vec![Block::ToolResult {
                id: "before".into(),
                content: "<result>\nundefined\n</result>".into(),
                is_error: false,
            }],
        },
    ];
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("r0", "REPL", json!({ "code": "typeof kept" }))]),
        ScriptedModel::reply(vec![tool_use("r1", "REPL", json!({ "code": "'again'" }))]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);

    rig.run_with(&model, &mut history, vec![Box::new(agent_engine::tools::repl::Repl)]).expect("run");

    let results = results(&history);
    assert!(results[1].0.starts_with("<note>\nThis script ran in a new JavaScript context"), "{:?}", results[1]);
    assert!(results[1].0.contains("<result>\nundefined\n</result>"));
    assert_eq!(results[2], ("<result>\nagain\n</result>".to_owned(), false), "the note comes once");
}

#[test]
fn what_one_script_keeps_the_next_one_reads() {
    let rig = code_mode(PermissionMode::AcceptEdits);

    let history = scripts(&rig, &["const notes = await Read({ file_path: 'notes.txt' })", "notes.includes('beta')"]);

    assert_eq!(results(&history)[1], ("<result>\ntrue\n</result>".to_owned(), false));
}

#[test]
fn a_call_the_user_refuses_throws_in_the_script_and_nothing_is_written() {
    let rig = code_mode(PermissionMode::Ask);
    let (control, events) = (Arc::clone(&rig.control), Arc::clone(&rig.events));
    let denier = std::thread::spawn(move || {
        for _ in 0..2000 {
            let ask = events.lock().expect("events").iter().find_map(|event| match event {
                EngineEvent::Asked(ask) => Some(ask.id.clone()),
                _ => None,
            });
            if let Some(ask) = ask {
                control.answer(&ask, Decision::Deny);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });
    let code = "try { await Write({ file_path: 'made.txt', content: 'x' }); 'written' } \
                catch (error) { `refused: ${error.name} ${error.tool}` }";

    let history = scripts(&rig, &[code]);
    denier.join().expect("denier");

    let (content, failed) = &results(&history)[0];
    assert!(!failed && content.starts_with("<result>\nrefused: ToolError Write\n</result>\n<refused>\n"), "{content}");
    assert!(content.contains("- Write: "), "the refused call is listed: {content}");
    assert!(!rig.dir.path().join("made.txt").exists());
}

#[test]
fn a_script_cannot_call_the_repl_itself_and_a_thrown_error_fails_the_call() {
    let rig = code_mode(PermissionMode::AcceptEdits);

    let history = scripts(&rig, &["await REPL({ code: '1' })", "await tools.REPL({ code: '1' })"]);

    let results = results(&history);
    assert!(results[0].1 && results[0].0.contains("REPL is not defined"), "{results:?}");
    assert!(results[1].1 && results[1].0.contains("REPL cannot be called from a REPL script"), "{results:?}");
}
