use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::api::{ProviderEvent, Reply};
use domain::provider::ModelProvider;
use llm_providers::ProviderError;
use platform::sync::Locked;

use super::ModelProviders;
use crate::handler::outbox::Replies;

const RELEASE_WAIT: Duration = Duration::from_secs(5);
const RELEASE_POLL: Duration = Duration::from_millis(10);
const STOPPED: &str = "this sign-in was cancelled or replaced by a newer one";

#[derive(Debug)]
pub(super) struct InFlight {
    stop: Arc<AtomicBool>,
    ended: AtomicBool,
}

fn failed(provider: ModelProvider, message: String) -> Reply {
    Reply::Providers(ProviderEvent::Failed { provider, message })
}

impl ModelProviders {
    pub fn sign_in(&self, provider: ModelProvider, replies: &Replies) {
        if let Some(message) = self.cannot_sign_in(provider) {
            return replies.send(failed(provider, message));
        }
        let (this, replies) = (self.clone(), replies.clone());
        platform::named::spawn("provider-sign-in", move || this.signing_in(provider, &replies));
    }

    pub fn close_and_sign_in(&self, provider: ModelProvider, port: u16, pid: u32, replies: &Replies) {
        if let Some(message) = self.secrets.unkept().map(|reason| crate::secrets::refusal(&reason)) {
            return replies.send(failed(provider, message));
        }
        let (this, replies) = (self.clone(), replies.clone());
        platform::named::spawn("provider-sign-in", move || {
            if pid != std::process::id()
                && this.ports.holder(port).is_some_and(|holder| holder.pid == pid)
                && let Err(message) = this.ports.close(port, pid)
            {
                return replies.send(failed(provider, message));
            }
            this.signing_in(provider, &replies);
        });
    }

    fn cannot_sign_in(&self, provider: ModelProvider) -> Option<String> {
        if !agent_engine::providers::find(provider).is_some_and(|known| known.access().subscription()) {
            return Some(format!("{} connects with an API key", agent_engine::providers::name(provider)));
        }
        self.secrets.unkept().map(|reason| crate::secrets::refusal(&reason))
    }

    fn signing_in(&self, provider: ModelProvider, replies: &Replies) {
        let mine = self.replacing_the_one_in_flight(provider, replies.stop());
        let mut open = |url: String, code: Option<String>| {
            replies.send(Reply::Providers(ProviderEvent::AuthorizeAt { provider, url, code }));
        };
        let signed = self.sign_in.sign_in(provider, &mine.stop, &mut open);
        self.ended(provider, &mine);
        if mine.stop.load(Ordering::SeqCst) {
            return replies.send(failed(provider, STOPPED.to_owned()));
        }
        let config = match signed {
            Err(ProviderError::PortInUse(port)) => return replies.send(self.port_busy(provider, port)),
            other => other.map_err(|error| error.to_string()),
        };
        let result = config.and_then(|config| self.keep(provider, agent_engine::providers::name(provider), &config));
        replies.send(self.answer(provider, result));
    }

    fn port_busy(&self, provider: ModelProvider, port: u16) -> Reply {
        let holder = self.ports.holder(port);
        if holder.as_ref().is_some_and(|holder| holder.pid == std::process::id()) {
            let message = format!(
                "the sign-in callback port {port} is held by this very session, which will not close itself; \
                 quit, start again and sign in once more"
            );
            return failed(provider, message);
        }
        Reply::Providers(ProviderEvent::PortBusy { provider, port, holder })
    }

    fn replacing_the_one_in_flight(&self, provider: ModelProvider, stop: &Arc<AtomicBool>) -> Arc<InFlight> {
        let mine = Arc::new(InFlight { stop: Arc::clone(stop), ended: AtomicBool::new(false) });
        let earlier = self.in_flight.locked().insert(provider, Arc::clone(&mine));
        if let Some(earlier) = earlier {
            earlier.stop.store(true, Ordering::SeqCst);
            let asked = Instant::now();
            while !earlier.ended.load(Ordering::SeqCst) && asked.elapsed() < RELEASE_WAIT {
                std::thread::sleep(RELEASE_POLL);
            }
        }
        mine
    }

    fn ended(&self, provider: ModelProvider, mine: &Arc<InFlight>) {
        mine.ended.store(true, Ordering::SeqCst);
        let mut in_flight = self.in_flight.locked();
        if in_flight.get(&provider).is_some_and(|kept| Arc::ptr_eq(kept, mine)) {
            in_flight.remove(&provider);
        }
    }
}
