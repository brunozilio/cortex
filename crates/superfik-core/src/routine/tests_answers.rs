//! Routines that answer the messages of their channel accounts.

use super::*;

const MONDAY: i64 = 1_792_972_800;

fn draft(name: &str, answers: bool, account: &crate::Connection) -> RoutineDraft {
    RoutineDraft {
        name: name.into(),
        brief: "Answer customers".into(),
        schedule: Schedule::Every { minutes: 60 },
        autonomy: Autonomy::Draft,
        model: None,
        connections: vec![account.clone()],
        answers,
    }
}

fn whatsapp() -> crate::Connection {
    crate::Connection {
        id: crate::ConnectionId::new(),
        plugin: crate::PluginId("whatsapp".into()),
        plugin_name: "WhatsApp".into(),
        label: "support".into(),
        icon: String::new(),
    }
}

#[test]
fn a_message_prompt_quotes_the_message_and_its_conversation() {
    let mut book = RoutineBook::default();
    book.apply(&[RoutineEdit::Create { draft: draft("Upkeep", true, &whatsapp()) }], MONDAY);
    let incoming = Incoming {
        channel: "WhatsApp · support".into(),
        exchange: "5511999990000".into(),
        from: Some("Ana".into()),
        text: "O checkout\nnão abre".into(),
    };
    let prompt = book.routines[0].prompt(None, Cause::Message(&incoming));
    assert!(prompt.contains("a message arrived on WhatsApp · support, in the conversation `5511999990000` from Ana"));
    assert!(prompt.contains("<untrusted source=\"a channel message\">\nO checkout\nnão abre\n</untrusted>"));
    assert!(!prompt.contains("on its schedule"));
}

#[test]
fn only_enabled_answering_routines_with_the_account_listen_to_it() {
    let account = whatsapp();
    let mut book = RoutineBook::default();
    let edits = [
        RoutineEdit::Create { draft: draft("Support", true, &account) },
        RoutineEdit::Create { draft: draft("Digest", false, &account) },
    ];
    book.apply(&edits, MONDAY);
    assert_eq!(book.listening(account.id, "a"), vec![RoutineId(0)]);
    assert!(book.listening(crate::ConnectionId::new(), "a").is_empty());
    book.apply(&[RoutineEdit::Enable { id: RoutineId(0), enabled: false }], MONDAY);
    assert!(book.listening(account.id, "a").is_empty());
}

#[test]
fn a_routine_answering_a_conversation_is_not_started_again_for_it() {
    let account = whatsapp();
    let mut book = RoutineBook::default();
    book.apply(&[RoutineEdit::Create { draft: draft("Support", true, &account) }], MONDAY);
    let next = book.routines[0].next_run;
    let run = RunId::new();
    assert!(book.start_answer(RoutineId(0), run, "a", None, MONDAY + 10));
    assert_eq!(book.routines[0].next_run, next, "the schedule stays");
    assert_eq!(book.report(run).and_then(|r| r.exchange.clone()), Some("a".into()));
    assert!(book.listening(account.id, "a").is_empty());
    assert_eq!(book.listening(account.id, "b"), vec![RoutineId(0)]);
    book.finish(run, Ending::Completed, "", 0.0, MONDAY + 20);
    assert_eq!(book.listening(account.id, "a"), vec![RoutineId(0)]);
}
