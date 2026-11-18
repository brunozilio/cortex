use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use domain::inbox::Notice;
use domain::observer::{Item, Observing};
use platform::sync::Locked;

use super::subagents::background::Spawn;
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

#[derive(Default)]
pub(in crate::agent) struct Watching {
    seen: Mutex<usize>,
    pending: Mutex<HashMap<String, Vec<Item>>>,
    busy: Mutex<HashSet<String>>,
    settled: Condvar,
}

impl Watching {
    fn gather(&self, turn: &[Message], observers: &[String], end: Option<&str>) {
        let mut seen = self.seen.locked();
        let mut fresh = items_of(turn.get(*seen..).unwrap_or_default());
        *seen = turn.len();
        if let Some(reason) = end {
            fresh.push(Item::TurnEnded(reason.to_owned()));
        }
        let mut pending = self.pending.locked();
        for name in observers {
            pending.entry(name.clone()).or_default().extend(fresh.iter().cloned());
        }
    }

    fn take_if_free(&self, name: &str) -> Option<Vec<Item>> {
        let mut busy = self.busy.locked();
        if busy.contains(name) {
            return None;
        }
        let waiting = self.pending.locked().remove(name).filter(|items| !items.is_empty())?;
        busy.insert(name.to_owned());
        Some(waiting)
    }

    fn take(&self, name: &str) -> Vec<Item> {
        self.pending.locked().remove(name).unwrap_or_default()
    }

    fn free(&self, name: &str) {
        self.busy.locked().remove(name);
        self.settled.notify_all();
    }

    fn wait_until_free(&self, stopped: impl Fn() -> bool) {
        let mut busy = self.busy.locked();
        while !busy.is_empty() && !stopped() {
            busy = match self.settled.wait_timeout(busy, Duration::from_millis(50)) {
                Ok((busy, _)) => busy,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
    }
}

fn items_of(turn: &[Message]) -> Vec<Item> {
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
    items
}

fn is_report(notice: &Notice) -> bool {
    notice.source.starts_with(domain::observer::SOURCE)
}

impl<'env> Agent<'env> {
    fn observer(&self, name: &str) -> Option<&'env FoundAgent> {
        let found = self.agents.iter().find(|agent| agent.name == name);
        if found.is_none() {
            tracing::warn!(observer = %name, "no agent of this type; it cannot observe");
        }
        found
    }

    fn digest(&self, observing: &Observing, items: &[Item]) -> String {
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        domain::observer::digest(&self.workspace.agent_name, &nonce, items, observing.message.as_deref())
    }

    pub(in crate::agent) fn observe_meanwhile<'s>(
        &'s self,
        turn: &[Message],
        watching: &Arc<Watching>,
        spawn: Spawn<'_, 's>,
    ) {
        let Some(observing) = self.workspace.observing.as_ref().filter(|_| !self.control.is_stopped()) else {
            return;
        };
        watching.gather(turn, &observing.observers, None);
        for name in &observing.observers {
            let Some(found) = self.observer(name) else { continue };
            let Some(items) = watching.take_if_free(name) else { continue };
            let digest = self.digest(observing, &items);
            let watching = Arc::clone(watching);
            spawn(Box::new(move || {
                let reports = Reports::default();
                self.deliver(found, observing, digest, Arc::clone(&reports));
                let client = self.workspace.credentials.client;
                for report in std::mem::take(&mut *reports.locked()) {
                    crate::tools::inbox::push(client, report);
                }
                watching.free(&found.name);
            }));
        }
    }

    pub(in crate::agent) fn observe(
        &self,
        turn: &[Message],
        outcome: &Result<crate::transcript::Usage, Ending>,
        watching: &Watching,
    ) -> Vec<Notice> {
        let Some(observing) = &self.workspace.observing else { return Vec::new() };
        watching.wait_until_free(|| self.control.is_stopped());
        let Some(reason) = ended(outcome).filter(|_| !self.control.is_stopped()) else { return Vec::new() };
        watching.gather(turn, &observing.observers, Some(reason));
        let reports = Reports::default();
        std::thread::scope(|scope| {
            for name in &observing.observers {
                let Some(found) = self.observer(name) else { continue };
                let items = watching.take(name);
                if items.is_empty() {
                    continue;
                }
                let digest = self.digest(observing, &items);
                let reports = Arc::clone(&reports);
                scope.spawn(move || self.deliver(found, observing, digest, reports));
            }
        });
        let mut heard = crate::tools::inbox::take_where(self.workspace.credentials.client, is_report);
        heard.extend(std::mem::take(&mut *reports.locked()));
        heard
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

        let watching = Watching::default();
        let observers = ["critic".to_owned()];
        watching.gather(&turn[..2], &observers, None);
        let first = watching.take_if_free("critic").expect("items to deliver");
        assert!(watching.take_if_free("critic").is_none(), "one delivery at a time");
        watching.gather(&turn, &observers, Some("done"));
        watching.free("critic");
        let second = watching.take_if_free("critic").expect("what came since");

        assert_eq!(
            first,
            [
                Item::UserMessage("fix it".into()),
                Item::Said("Looking.".into()),
                Item::ToolCall { name: "Read".into(), input: "{\"file_path\":\"a\"}".into() },
            ]
        );
        assert_eq!(
            second,
            [Item::ToolResult { text: "text".into(), failed: false }, Item::TurnEnded("done".into())],
            "each item once, in order"
        );
    }
}
