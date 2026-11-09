//! Routines a webhook starts.

use super::*;

const MONDAY: i64 = 1_792_972_800;

fn book() -> RoutineBook {
    let draft = RoutineDraft {
        name: "Orders".into(),
        brief: "Thank the buyer".into(),
        schedule: Schedule::Every { minutes: 60 },
        autonomy: Autonomy::Draft,
        model: None,
        connections: Vec::new(),
        answers: false,
    };
    let mut book = RoutineBook::default();
    book.apply(&[RoutineEdit::Create { draft }], MONDAY);
    book
}

#[test]
fn a_webhook_is_given_taken_and_renewed() {
    let mut book = book();
    let id = RoutineId(0);
    assert_eq!(book.routines[0].webhook, None);
    book.apply(&[RoutineEdit::Webhook { id, on: true }], MONDAY);
    let hook = book.routines[0].webhook.unwrap_or_else(|| panic!("no webhook"));
    book.apply(&[RoutineEdit::Webhook { id, on: true }], MONDAY);
    assert_eq!(book.routines[0].webhook, Some(hook), "on twice keeps the address");
    assert_eq!(book.hooked(hook), Some(id));
    book.apply(&[RoutineEdit::Webhook { id, on: false }], MONDAY);
    assert_eq!(book.hooked(hook), None);
    book.apply(&[RoutineEdit::Webhook { id, on: true }], MONDAY);
    assert_ne!(book.routines[0].webhook, Some(hook), "a new address");
    let outcome = book.apply(&[RoutineEdit::Webhook { id: RoutineId(9), on: true }], MONDAY);
    assert_eq!(outcome.refused, [RoutineId(9)]);
}

#[test]
fn a_paused_routine_answers_no_webhook() {
    let mut book = book();
    let id = RoutineId(0);
    book.apply(&[RoutineEdit::Webhook { id, on: true }, RoutineEdit::Enable { id, enabled: false }], MONDAY);
    let hook = book.routines[0].webhook.unwrap_or_else(|| panic!("no webhook"));
    assert_eq!(book.hooked(hook), None);
}

#[test]
fn a_hooked_firing_keeps_the_schedule_and_fences_the_body() {
    let mut book = book();
    let next = book.routines[0].next_run;
    assert!(book.start_hooked(RoutineId(0), crate::RunId::new(), None, MONDAY + 60));
    assert_eq!(book.routines[0].next_run, next);
    assert!(book.reports[0].is_running());
    let prompt = book.routines[0].prompt(None, Cause::Webhook("{\"order\": 42}"));
    assert!(prompt.contains("its webhook was called"));
    assert!(prompt.contains("<untrusted source=\"webhook\">\n{\"order\": 42}\n</untrusted>"));
    assert!(prompt.ends_with("Thank the buyer"));
}
