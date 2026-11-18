use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use domain::inbox::Notice;
use domain::observer::{Item, Observing};
use platform::sync::Locked;

use super::{Agent, Ending};
use crate::model::TurnRequest;
use crate::tools::Tool;
use crate::tools::observer_report::{ObserverReport, Reports};
use crate::tools::subagent::FoundAgent;
use crate::transcript::{Block, Message, Role};

const OBSERVER_TURNS: usize = 4;

fn conversations() -> MutexGuard<'static, HashMap<String, Vec<Message>>> {
    static KEPT: OnceLock<Mutex<HashMap<String, Vec<Message>>>> = OnceLock::new();
    KEPT.get_or_init(Mutex::default).locked()
}

fn ended(outcome: &Result<crate::transcript::Usage, Ending>) -> Option<&'static str> {
    match outcome {
        Ok(_) => Some("done"),
        Err(Ending::Cancelled | Ending::Moved) => None,
        Err(Ending::Failed(_)) => Some("failed"),
        Err(Ending::Refused) => Some("refused"),
        Err(Ending::TooManyTurns) => Some("too many turns"),
    }
}

pub(in crate::agent) fn items(turn: &[Message], reason: &str) -> Vec<Item> {
    let mut items = Vec::new();
    for message in turn {
        for block in &message.blocks {
            match (message.role, block) {
                (Role::User, Block::Text(text)) => items.push(Item::UserMessage(text.clone())),
                (Role::Assistant, Block::Text(text)) if !text.trim().is_empty() => {
                    items.push(Item::Said(text.clone()));
                }
                (_, Block::ToolUse { name, input, .. }) => {
                    items.push(Item::ToolCall { name: name.clone(), input: input.to_string() });
                }
                (_, Block::ToolResult { content, is_error, .. } | Block::ToolResultRich { content, is_error, .. }) => {
                    items.push(Item::ToolResult { text: content.clone(), failed: *is_error });
                }
                _ => {}
            }
        }
    }
    items.push(Item::TurnEnded(reason.to_owned()));
    items
}

impl Agent<'_> {
    pub(in crate::agent) fn observe(
        &self,
        turn: &[Message],
        outcome: &Result<crate::transcript::Usage, Ending>,
    ) -> Vec<Notice> {
        let Some(observing) = &self.workspace.observing else { return Vec::new() };
        let Some(reason) = ended(outcome).filter(|_| !self.control.is_stopped()) else { return Vec::new() };
        let items = items(turn, reason);
        let reports = Reports::default();
        std::thread::scope(|scope| {
            for name in &observing.observers {
                let Some(found) = self.agents.iter().find(|agent| &agent.name == name) else {
                    tracing::warn!(observer = %name, "no agent of this type; it cannot observe");
                    continue;
                };
                let nonce = uuid::Uuid::new_v4().simple().to_string();
                let digest =
                    domain::observer::digest(&self.workspace.agent_name, &nonce, &items, observing.message.as_deref());
                let reports = Arc::clone(&reports);
                scope.spawn(move || self.deliver(found, observing, digest, reports));
            }
        });
        std::mem::take(&mut *reports.locked())
    }

    pub(in crate::agent) fn heard(&self, reports: Vec<Notice>, went_well: bool) -> Option<Message> {
        if reports.is_empty() {
            return None;
        }
        if self.depth > 0 || !went_well {
            let client = self.workspace.credentials.client;
            reports.into_iter().for_each(|report| crate::tools::inbox::push(client, report));
            return None;
        }
        (self.emit)(domain::EngineEvent::Notified(reports.clone()));
        Some(Message::user(domain::inbox::as_message(&reports)))
    }

    fn deliver(&self, found: &FoundAgent, observing: &Observing, digest: String, reports: Reports) {
        let key =
            format!("{}/{}/{}/{}", self.workspace.session_id, self.workspace.agent_name, observing.depth, found.name);
        let mut history = conversations().remove(&key).unwrap_or_default();
        history.push(Message::user(digest));
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(ObserverReport { observer: found.name.clone(), reports })];
        let control = self.control.child();
        let mut workspace = self.workspace.fork(control.shared_stop());
        workspace.agent_name.clone_from(&found.name);
        workspace.observing = None;
        let provider = self.workspace.models.model.provider;
        let model = found.config.model(&self.request.model, provider).unwrap_or_else(|_| self.request.model.clone());
        let system = format!(
            "{}\n\n{}",
            crate::tools::subagent::system(&self.request.system, Some(found)),
            crate::prompt::text("observer")
        );
        let request =
            TurnRequest { model, system, tools: Agent::specs(&tools), messages: Vec::new(), ..self.request.clone() };
        let quiet = |_| {};
        let observer = Agent {
            api: self.api,
            tools: &tools,
            workspace: &workspace,
            control: &control,
            emit: &quiet,
            request,
            agents: &[],
            depth: domain::observer::DEEPEST + 1,
            turn_limit: OBSERVER_TURNS,
        };
        if let Err(ending) = observer.run(&mut history) {
            tracing::warn!(observer = %found.name, ?ending, "an observer's delivery did not finish");
        }
        conversations().insert(key, history);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_becomes_the_items_an_observer_reads() {
        let turn = vec![
            Message::user("fix it"),
            Message::assistant(vec![
                Block::Text("Looking.".into()),
                Block::ToolUse { id: "1".into(), name: "Read".into(), input: serde_json::json!({ "file_path": "a" }) },
            ]),
            Message {
                role: Role::User,
                blocks: vec![Block::ToolResult { id: "1".into(), content: "text".into(), is_error: false }],
            },
        ];

        let items = items(&turn, "done");

        assert_eq!(
            items,
            [
                Item::UserMessage("fix it".into()),
                Item::Said("Looking.".into()),
                Item::ToolCall { name: "Read".into(), input: "{\"file_path\":\"a\"}".into() },
                Item::ToolResult { text: "text".into(), failed: false },
                Item::TurnEnded("done".into()),
            ]
        );
    }
}
