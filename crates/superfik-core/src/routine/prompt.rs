//! What a firing tells the model. Not translated: it is read by the model.
//! Text from outside (a customer's message, a webhook's body) is fenced as
//! untrusted: the routine's brief and autonomy are what it follows.

use super::webhook::fence;
use super::{Autonomy, Incoming, Routine};

#[derive(Debug, Clone, Copy)]
pub enum Cause<'a> {
    Schedule,
    Message(&'a Incoming),
    Webhook(&'a str),
}

impl Routine {
    pub fn prompt(&self, branch: Option<&str>, cause: Cause<'_>) -> String {
        let (why, outside) = match cause {
            Cause::Schedule => ("started on its schedule".to_owned(), String::new()),
            Cause::Message(incoming) => (
                format!(
                    "started because a message arrived on {channel}, in the conversation `{exchange}`{from}. \
                     Read the conversation with the channel tools and answer it there if it is yours to answer",
                    channel = incoming.channel,
                    exchange = incoming.exchange,
                    from = incoming.from.as_ref().map_or(String::new(), |name| format!(" from {name}")),
                ),
                fence("a channel message", &incoming.text),
            ),
            Cause::Webhook(body) => (
                "started because its webhook was called. What the call sent follows; decide what the brief asks of it"
                    .to_owned(),
                fence("webhook", body),
            ),
        };
        let latitude = match self.autonomy {
            Autonomy::Draft => {
                "You may only read and investigate: do not change files, run commands that change anything, \
                 or contact anyone. Propose what should be done; a member will decide."
            }
            Autonomy::Bounded => {
                "You may change files in the project. Anything that would need permission will be refused, \
                 so propose it instead."
            }
            Autonomy::Auto => "You may act on your own within the tools you have.",
        };
        let place = branch.map_or(String::new(), |branch| {
            format!(
                "You are in a separate worktree on the new branch `{branch}`: commit what you change there, \
                 and name the branch in your report.\n"
            )
        });
        let outside = if outside.is_empty() { outside } else { format!("{outside}\n") };
        format!(
            "This is the routine \"{name}\", {why}; no one is watching this run.\n\
             {latitude}\n{place}{outside}\
             Finish with a short report for the member: what you found, what you did, what you propose.\n\n\
             {conventions}\n\n\
             {brief}",
            name = self.name,
            brief = self.brief,
            conventions = crate::job::CONVENTIONS,
        )
    }
}
