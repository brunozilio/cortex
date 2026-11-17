use std::sync::Arc;

use super::*;
use crate::model::scripted::ScriptedModel;
use crate::model::{
    BlockingTurn, BoxFuture, CancellationToken, Delta, ModelApi, ModelError, OnDelta, Reply, TurnRequest,
};
use crate::transcript::Block;

fn request() -> TurnRequest {
    TurnRequest {
        model: "m".into(),
        system: String::new(),
        messages: vec![crate::transcript::Message::user("hello")],
        tools: Vec::new(),
        effort: domain::Effort::Medium,
        max_tokens: 1000,
    }
}

fn pool(apis: Vec<Arc<dyn ModelApi>>) -> Pooled {
    let labels = ["a@x.io", "b@x.io", "c@x.io"];
    Pooled::new(labels.iter().map(|label| (*label).to_owned()).zip(apis).collect())
}

fn ask(api: &dyn ModelApi) -> Result<Reply, ModelError> {
    api.turn_blocking(&request(), &mut |_| {}, &CancellationToken::new())
}

fn answer(text: &str) -> Result<Reply, ModelError> {
    ScriptedModel::reply(vec![Block::Text(text.into())])
}

fn text(reply: &Reply) -> String {
    reply.message.text()
}

#[test]
fn a_busy_account_hands_the_turn_to_the_next_which_keeps_the_run() {
    let first = Arc::new(ScriptedModel::new([Err(ModelError::Overloaded("429".into())), answer("never")]));
    let second = Arc::new(ScriptedModel::new([answer("from b"), answer("b again")]));
    let pooled = pool(vec![first.clone() as Arc<dyn ModelApi>, second.clone()]);
    assert_eq!(text(&ask(&pooled).expect("handed over")), "from b");
    assert_eq!(text(&ask(&pooled).expect("stays")), "b again");
    assert_eq!(first.asked().len(), 1, "the busy account is not asked again this run");
}

#[test]
fn the_hand_over_is_told_with_both_accounts_names() {
    let first = Arc::new(ScriptedModel::new([Err(ModelError::Overloaded("429".into()))]));
    let second = Arc::new(ScriptedModel::new([answer("ok")]));
    let pooled = pool(vec![first as Arc<dyn ModelApi>, second]);
    let mut seen = Vec::new();
    pooled.turn_blocking(&request(), &mut |delta| seen.push(delta), &CancellationToken::new()).expect("ok");
    seen.retain(|delta| matches!(delta, Delta::HandedOver { .. }));
    assert_eq!(seen, [Delta::HandedOver { from: "a@x.io".into(), to: "b@x.io".into() }]);
}

#[test]
fn a_signed_out_account_hands_over_too() {
    let first = Arc::new(ScriptedModel::new([Err(ModelError::Unauthorized("401".into()))]));
    let second = Arc::new(ScriptedModel::new([answer("ok")]));
    let pooled = pool(vec![first as Arc<dyn ModelApi>, second]);
    assert_eq!(text(&ask(&pooled).expect("handed over")), "ok");
}

#[test]
fn other_failures_are_the_model_s_and_do_not_hand_over() {
    let first = Arc::new(ScriptedModel::new([Err(ModelError::TooLong("big".into()))]));
    let second = Arc::new(ScriptedModel::new([answer("unused")]));
    let pooled = pool(vec![first as Arc<dyn ModelApi>, second.clone()]);
    assert_eq!(ask(&pooled).map(|_| ()), Err(ModelError::TooLong("big".into())));
    assert!(second.asked().is_empty());
}

#[test]
fn when_every_account_is_busy_the_last_failure_is_told() {
    let busy = |why: &str| Arc::new(ScriptedModel::new([Err(ModelError::Overloaded(why.into()))]));
    let pooled = pool(vec![busy("a") as Arc<dyn ModelApi>, busy("b")]);
    let mut seen = Vec::new();
    let failed = pooled.turn_blocking(&request(), &mut |delta| seen.push(delta), &CancellationToken::new());
    assert_eq!(failed.map(|_| ()), Err(ModelError::Overloaded("b".into())));
    assert_eq!(seen.len(), 1, "only a real hand-over is told: {seen:?}");
}

struct HalfAnswer;

impl ModelApi for HalfAnswer {
    fn turn<'a>(
        &'a self,
        _: &'a TurnRequest,
        on: OnDelta<'a>,
        _: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(async move {
            on(Delta::Text("partial".into()));
            Err(ModelError::Overloaded("cut".into()))
        })
    }
}

#[test]
fn a_turn_that_already_streamed_text_is_not_asked_again_elsewhere() {
    let second = Arc::new(ScriptedModel::new([answer("again")]));
    let pooled = pool(vec![Arc::new(HalfAnswer) as Arc<dyn ModelApi>, second.clone()]);
    assert_eq!(ask(&pooled).map(|_| ()), Err(ModelError::Overloaded("cut".into())));
    assert!(second.asked().is_empty());
}
