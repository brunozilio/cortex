use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use domain::{EngineEvent, PromptRequest, SessionId};

use crate::boundary::Observer;
use crate::sessions::{Entry, Sessions};

pub(super) struct Recorder {
    sessions: Arc<dyn Sessions>,
    request: PromptRequest,
    session: SessionId,
    held: Mutex<Option<(u64, EngineEvent)>>,
}

impl Recorder {
    pub(super) fn start(sessions: Arc<dyn Sessions>, request: &PromptRequest, session: &SessionId) -> Observer {
        let recorder =
            Recorder { sessions, request: request.clone(), session: session.clone(), held: Mutex::new(None) };
        if request.command.is_none() {
            recorder.write(Entry::Prompted { at_ms: now(), prompt: request.prompt.clone() });
        }
        let recorder = Arc::new(recorder);
        Arc::new(move |event: &EngineEvent| recorder.see(event))
    }

    fn see(&self, event: &EngineEvent) {
        if event.is_partial_input() {
            return;
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

fn streams(event: &EngineEvent) -> bool {
    match event {
        EngineEvent::TextDelta(_) | EngineEvent::ThinkingDelta(_) => true,
        EngineEvent::AgentEvent { event, .. } => streams(event),
        _ => false,
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as u64).unwrap_or_default()
}
