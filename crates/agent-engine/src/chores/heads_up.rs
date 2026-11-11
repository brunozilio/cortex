use std::sync::atomic::AtomicBool;

use domain::HeadsUp;

use crate::model::{ModelApi, ModelError, TurnRequest};
use crate::transcript::Message;

pub(super) fn look(
    api: &dyn ModelApi,
    template: &TurnRequest,
    history: &[Message],
    stop: &AtomicBool,
) -> Result<Option<HeadsUp>, ModelError> {
    let mut messages = history.to_vec();
    while messages.last().is_some_and(|message| message.tool_uses().next().is_some()) {
        messages.pop();
    }
    messages.push(Message::user(crate::prompt::text("heads-up")));
    let request =
        TurnRequest { messages, tools: Vec::new(), max_tokens: template.max_tokens.min(1500), ..template.clone() };
    Ok(parse(&api.turn(&request, &mut |_| {}, stop)?.message.text()))
}

pub fn parse(answer: &str) -> Option<HeadsUp> {
    let after = |label: &str| {
        answer.lines().find_map(|line| line.trim().strip_prefix(label).map(|rest| rest.trim().trim_matches('`').trim()))
    };
    let learn = after("learn:")?.to_owned();
    if learn.is_empty() || learn.eq_ignore_ascii_case("none") {
        return None;
    }
    let urgent = after("tag:")?.to_ascii_lowercase().starts_with("heads up");
    let body = answer.split_once("explain:")?.1.trim();
    let (title, explain) = match body.strip_prefix("**").and_then(|rest| rest.split_once("**")) {
        Some((title, rest)) => (title.trim().to_owned(), rest.trim().to_owned()),
        None => (String::new(), body.to_owned()),
    };
    (!title.is_empty() && !explain.is_empty()).then_some(HeadsUp { urgent, learn, title, explain })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heads_up_has_its_parts_and_none_is_nothing() {
        let answer = "learn: The cache now spans users, so one user's data can leak to another.\n\
                      tag: Heads up\n\
                      explain:\n\n**Cached answers are shared**\n\nThe main agent keyed the cache by query only.";
        let found = parse(answer).expect("a heads-up");
        assert!(found.urgent);
        assert_eq!(found.title, "Cached answers are shared");
        assert_eq!(found.explain, "The main agent keyed the cache by query only.");
        assert!(found.learn.ends_with("leak to another."));
        assert_eq!(parse("learn: none"), None);
        assert_eq!(parse("learn: x\ntag: You should know\nexplain:\nno title"), None, "out of form");
        let ask = crate::prompt::text("heads-up");
        assert!(!ask.contains("Claude") && ask.contains("learn: none"));
    }
}
