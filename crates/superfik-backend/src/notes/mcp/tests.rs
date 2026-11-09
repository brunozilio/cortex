use std::sync::mpsc;

use super::*;

fn tools() -> (NotesTools, mpsc::Receiver<(Principal, ServerEvent)>) {
    let (sent, published) = mpsc::channel();
    let sent = Mutex::new(sent);
    let publish: Publish = Arc::new(move |owner, event| {
        let _ = lock(&sent).send((owner.clone(), event));
    });
    (NotesTools::new(Notes::in_memory(), publish), published)
}

fn call(tools: &NotesTools, token: &str, name: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": arguments } });
    tools.handle(token, &message).expect("granted").expect("answer")
}

fn text(answer: &Value) -> &str {
    answer["result"]["content"][0]["text"].as_str().unwrap_or_default()
}

#[test]
fn lists_the_four_tools() {
    let (tools, _) = tools();
    let grant = tools.grant(&Principal::LocalOwner, "/repo".into(), RunId::new());
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
    let answer = tools.handle(&grant.token, &message).expect("granted").expect("answer");
    let names: Vec<&str> =
        answer["result"]["tools"].as_array().expect("tools").iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(names, [NOTES_LIST_TOOL, NOTES_READ_TOOL, NOTES_WRITE_TOOL, NOTES_CHECK_TOOL]);
}

#[test]
fn an_agent_writes_reads_and_ticks_a_note_and_every_change_is_published() {
    let (tools, published) = tools();
    let run = RunId::new();
    let grant = tools.grant(&Principal::LocalOwner, "/repo".into(), run);
    let body = "Release\n- [ ] tag\n- [ ] notes";
    let made = call(&tools, &grant.token, NOTES_WRITE_TOOL, json!({ "body": body, "paper": "blue" }));
    assert_eq!(text(&made), "Created note #0, linked to this session.");
    let (owner, event) = published.try_recv().expect("published");
    assert_eq!(owner, Principal::LocalOwner);
    assert!(
        matches!(event, ServerEvent::Notes { by: Some(by), ref created, .. } if by == run && created == &[NoteId(0)])
    );

    let ticked = call(&tools, &grant.token, NOTES_CHECK_TOOL, json!({ "id": 0, "item": 2, "done": true }));
    assert_eq!(text(&ticked), "Saved.");
    assert!(published.try_recv().is_ok());
    let read = call(&tools, &grant.token, NOTES_READ_TOOL, json!({ "id": 0 }));
    assert_eq!(text(&read), "#0 Release\nRelease\n- [ ] tag\n- [x] notes");
    let list = call(&tools, &grant.token, NOTES_LIST_TOOL, json!({}));
    assert_eq!(text(&list), "#0 Release (1 of 2 done)");

    let revised = call(&tools, &grant.token, NOTES_WRITE_TOOL, json!({ "id": 0, "body": "Shipped" }));
    assert_eq!(text(&revised), "Saved.");
    let note = tools.notes.book(&Principal::LocalOwner, std::path::Path::new("/repo")).notes[0].clone();
    assert_eq!((note.body.as_str(), note.paper, note.author), ("Shipped", Paper::Blue, Author::Agent));
}

#[test]
fn bad_calls_are_explained_to_the_agent() {
    let (tools, published) = tools();
    let grant = tools.grant(&Principal::LocalOwner, "/repo".into(), RunId::new());
    let missing = call(&tools, &grant.token, NOTES_READ_TOOL, json!({ "id": 4 }));
    assert_eq!(missing["result"]["isError"], true);
    let blank = call(&tools, &grant.token, NOTES_WRITE_TOOL, json!({ "body": "  " }));
    assert_eq!(text(&blank), "Give the note's text.");
    let colour = call(&tools, &grant.token, NOTES_WRITE_TOOL, json!({ "body": "x", "paper": "red" }));
    assert_eq!(colour["result"]["isError"], true);
    let unknown = call(&tools, &grant.token, NOTES_CHECK_TOOL, json!({ "id": 9, "item": 1, "done": true }));
    assert_eq!(text(&unknown), "Note #9 does not exist, or has no such item.");
    let zero = call(&tools, &grant.token, NOTES_CHECK_TOOL, json!({ "id": 0, "item": 0, "done": true }));
    assert_eq!(zero["result"]["isError"], true);
    assert_eq!(call(&tools, &grant.token, "other", json!({}))["error"]["code"], -32602);
    assert_eq!(published.try_iter().count(), 1, "only the refused check reached the store");
}

#[test]
fn a_token_works_only_until_the_run_ends() {
    let (tools, _) = tools();
    let grant = tools.grant(&Principal::LocalOwner, "/repo".into(), RunId::new());
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
    assert!(tools.handle(&grant.token, &message).is_ok());
    assert_eq!(tools.handle("other", &message), Err(401));
    tools.revoke(&grant.token);
    assert_eq!(tools.handle(&grant.token, &message), Err(401));
}

#[test]
fn a_run_sees_only_its_own_project() {
    let (tools, _) = tools();
    let a = tools.grant(&Principal::LocalOwner, "/a".into(), RunId::new());
    let b = tools.grant(&Principal::LocalOwner, "/b".into(), RunId::new());
    call(&tools, &a.token, NOTES_WRITE_TOOL, json!({ "body": "only in a" }));
    assert_eq!(text(&call(&tools, &b.token, NOTES_LIST_TOOL, json!({}))), "No notes yet.");
}
