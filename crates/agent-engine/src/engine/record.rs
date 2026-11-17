use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use domain::trace::{ModelCall, RunInfo};
use domain::{EngineEvent, PromptRequest, SessionId};
use platform::sync::Locked;

use crate::boundary::RunGate;
use crate::meter::Calls;
use crate::model::{Reply, TurnRequest};
use crate::sessions::{Entry, Sessions};
use crate::transcript::Message;

pub(super) struct Recorder {
    sessions: Arc<dyn Sessions>,
    request: PromptRequest,
    session: SessionId,
    held: Mutex<Option<(u64, EngineEvent)>>,
    seen: Mutex<HashSet<String>>,
    conversations: Mutex<super::sent::Conversations>,
    waiting: Mutex<Option<Vec<Entry>>>,
    started: AtomicBool,
    compacted: AtomicBool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Session {
    Kept,
    NotYetCreated,
}

impl Recorder {
    pub(super) fn start(
        sessions: Arc<dyn Sessions>,
        request: &PromptRequest,
        session: &SessionId,
        state: Session,
    ) -> Arc<Self> {
        let recorder = Recorder {
            sessions,
            request: request.clone(),
            session: session.clone(),
            held: Mutex::new(None),
            seen: Mutex::default(),
            conversations: Mutex::default(),
            waiting: Mutex::new((state == Session::NotYetCreated).then(Vec::new)),
            started: AtomicBool::new(false),
            compacted: AtomicBool::new(false),
        };
        if request.session.command.is_none() {
            if let Some(shown) = &request.prompt.shown {
                recorder.write(Entry::Shown(shown.clone()));
            }
            recorder.write(Entry::Prompted { at_ms: now(), prompt: request.prompt.text.clone() });
        }
        Arc::new(recorder)
    }

    pub(super) fn calls(self: &Arc<Self>) -> Calls {
        let recorder = Arc::clone(self);
        Arc::new(move |request: &TurnRequest, reply: &Reply| recorder.called(request, reply))
    }

    fn called(&self, request: &TurnRequest, reply: &Reply) {
        let tools = serde_json::to_value(
            request
                .tools
                .iter()
                .map(|spec| serde_json::json!({"name": spec.name, "description": spec.description, "schema": spec.schema}))
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();
        let (system_id, tools_id) = (fingerprint(&request.system), fingerprint(&tools.to_string()));
        let (system, tools) = match self.seen.lock() {
            Ok(mut seen) => (
                seen.insert(system_id.clone()).then(|| request.system.clone()),
                seen.insert(tools_id.clone()).then_some(tools),
            ),
            Err(_) => (Some(request.system.clone()), Some(tools)),
        };
        let agent = crate::agent::current::agent();
        let delta =
            self.conversations.lock().ok().map(|mut conversations| conversations.delta(agent, &request.messages));
        let call = ModelCall {
            model: request.model.clone(),
            stop: format!("{:?}", reply.stop),
            messages: request.messages.len(),
            max_tokens: request.max_tokens,
            effort: format!("{:?}", request.effort),
            system_id,
            system,
            tools_id,
            tools,
            keep: delta.as_ref().map_or(0, |delta| delta.keep),
            added: delta.map(|delta| delta.added),
            reply: serde_json::to_value(&reply.message).ok(),
            served: reply.served.clone(),
        };
        self.flush();
        self.write(Entry::Called { at_ms: now(), agent, call });
    }

    pub(super) fn opened(&self, run: RunInfo) {
        self.flush();
        self.write(Entry::Opened { at_ms: now(), run });
    }

    pub(super) fn flush(&self) {
        let streamed = self.held.locked().take();
        if let Some((at_ms, event)) = streamed {
            self.write(Entry::Event { at_ms, event });
        }
    }

    pub(super) fn see(&self, event: &EngineEvent) {
        if event.is_partial_input() {
            return;
        }
        if matches!(event, EngineEvent::SessionStarted(_)) && self.started.swap(false, Ordering::SeqCst) {
            return;
        }
        if matches!(event, EngineEvent::Compacted(_)) {
            self.compacted.store(true, Ordering::SeqCst);
        }
        let mut held = self.held.locked();
        if let Some((_, streaming)) = held.as_mut()
            && streaming.absorb(event)
        {
            return;
        }
        if let Some((at_ms, event)) = held.take() {
            self.write(Entry::Event { at_ms, event });
        }
        if streams(event) {
            *held = Some((now(), event.clone()));
        } else {
            self.write(Entry::Event { at_ms: now(), event: event.clone() });
        }
        drop(held);
        if event.is_terminal() {
            self.sessions.end_trace(&self.request.session.cwd, &self.session);
        }
    }

    pub(super) fn kept(&self, history: &[Message]) -> Result<(), String> {
        self.flush();
        let saved = if self.compacted() {
            self.sessions.save_compacted(&self.request.session.cwd, &self.session, history)
        } else {
            self.sessions.save_for(&self.request.session.cwd, &self.session, history)
        };
        self.sessions.flush_trace(&self.request.session.cwd, &self.session);
        saved
    }

    pub(super) fn compacted(&self) -> bool {
        self.compacted.swap(false, Ordering::SeqCst)
    }

    pub(super) fn starts_unannounced(&self) {
        self.write(Entry::Event { at_ms: now(), event: EngineEvent::SessionStarted(self.session.clone()) });
        self.started.store(true, Ordering::SeqCst);
    }

    pub(super) fn announce(&self, gate: &RunGate) {
        let Some(entries) = self.created() else { return };
        entries.iter().for_each(|entry| self.keep(entry));
        gate.emit(EngineEvent::SessionStarted(self.session.clone()));
    }

    fn created(&self) -> Option<Vec<Entry>> {
        self.waiting.locked().take()
    }

    fn write(&self, entry: Entry) {
        let mut waiting = self.waiting.locked();
        match waiting.as_mut() {
            Some(waiting) => waiting.push(entry),
            None => self.keep(&entry),
        }
    }

    fn keep(&self, entry: &Entry) {
        if let Err(error) = self.sessions.record(&self.request.session.cwd, &self.session, entry) {
            tracing::warn!(%error, "an event of this run was not kept in its session");
        }
    }
}

fn fingerprint(text: &str) -> String {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn streams(event: &EngineEvent) -> bool {
    match event {
        EngineEvent::TextDelta(_) | EngineEvent::ThinkingDelta(_) => true,
        EngineEvent::AgentEvent { event, .. } => streams(event),
        _ => false,
    }
}

pub(super) fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as u64).unwrap_or_default()
}
