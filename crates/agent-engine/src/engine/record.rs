use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use domain::trace::{ModelCall, RunInfo};
use domain::{EngineEvent, PromptRequest, SessionId};

use crate::boundary::Observer;
use crate::meter::Calls;
use crate::model::{Reply, TurnRequest};
use crate::sessions::{Entry, Sessions};

pub(super) struct Recorder {
    sessions: Arc<dyn Sessions>,
    request: PromptRequest,
    session: SessionId,
    held: Mutex<Option<(u64, EngineEvent)>>,
    seen: Mutex<HashSet<String>>,
}

impl Recorder {
    pub(super) fn start(sessions: Arc<dyn Sessions>, request: &PromptRequest, session: &SessionId) -> Arc<Self> {
        let recorder = Recorder {
            sessions,
            request: request.clone(),
            session: session.clone(),
            held: Mutex::new(None),
            seen: Mutex::default(),
        };
        if request.command.is_none() {
            recorder.write(Entry::Prompted { at_ms: now(), prompt: request.prompt.clone() });
        }
        Arc::new(recorder)
    }

    pub(super) fn observer(self: &Arc<Self>) -> Observer {
        let recorder = Arc::clone(self);
        Arc::new(move |event: &EngineEvent| recorder.see(event))
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
        };
        let Ok(mut held) = self.held.lock() else { return };
        if let Some((at_ms, event)) = held.take() {
            self.write(Entry::Event { at_ms, event });
        }
        self.write(Entry::Called { at_ms: now(), agent: crate::agent::current::agent(), call });
    }

    pub(super) fn opened(&self, run: RunInfo) {
        self.write(Entry::Opened { at_ms: now(), run });
    }

    fn see(&self, event: &EngineEvent) {
        if event.is_partial_input() {
            return;
        }
        if let (EngineEvent::Usage(_), Some(index)) = (event, crate::agent::current::agent()) {
            return self.see(&EngineEvent::AgentEvent { index, event: Box::new(event.clone()) });
        }
        let Ok(mut held) = self.held.lock() else { return };
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
    }

    fn write(&self, entry: Entry) {
        if let Err(error) = self.sessions.record(&self.request, &self.session, &entry) {
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
