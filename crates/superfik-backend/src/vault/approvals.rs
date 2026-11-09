//! Asking the user before a secret is used. The ask travels as an ordinary
//! run event, so the app shows it on the ask card like the engine's own;
//! the answer comes back as `AnswerAsk` and is claimed here by its id.

use std::collections::HashMap;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use superfik_core::{AgentAsk, AskId, AskKind, Decision, EngineEvent, Principal, RunId};

pub type Notify = Arc<dyn Fn(RunId, EngineEvent) + Send + Sync>;

const ID_PREFIX: &str = "vault-";

struct Waiting {
    owner: Principal,
    run: RunId,
    reply: Sender<Decision>,
}

#[derive(Clone)]
pub struct Approvals {
    waiting: Arc<Mutex<HashMap<String, Waiting>>>,
    notify: Notify,
}

impl Approvals {
    pub fn new(notify: Notify) -> Self {
        Self { waiting: Arc::default(), notify }
    }

    pub fn ask(&self, owner: &Principal, run: RunId, asker: &str, subject: String, limit: Duration) -> bool {
        let id = format!("{ID_PREFIX}{}", uuid::Uuid::new_v4().simple());
        let (reply, answer) = mpsc::channel();
        self.lock().insert(id.clone(), Waiting { owner: owner.clone(), run, reply });
        let kind = AskKind::Permission { tool: asker.to_owned(), subject, rememberable: false };
        (self.notify)(run, EngineEvent::Asked(AgentAsk { id: AskId(id.clone()), kind }));
        match answer.recv_timeout(limit) {
            Ok(decision) => matches!(decision, Decision::Allow | Decision::AllowAlways),
            Err(RecvTimeoutError::Timeout) => {
                if self.lock().remove(&id).is_some() {
                    (self.notify)(run, EngineEvent::AskWithdrawn(AskId(id)));
                }
                false
            }
            Err(RecvTimeoutError::Disconnected) => false,
        }
    }

    pub fn answer(&self, owner: &Principal, ask: &AskId, decision: &Decision) -> bool {
        if !ask.0.starts_with(ID_PREFIX) {
            return false;
        }
        let mut waiting = self.lock();
        if waiting.get(&ask.0).is_some_and(|w| &w.owner == owner)
            && let Some(w) = waiting.remove(&ask.0)
        {
            let _ = w.reply.send(decision.clone());
        }
        true
    }

    pub fn end(&self, run: RunId) {
        self.lock().retain(|_, w| w.run != run);
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Waiting>> {
        self.waiting.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
