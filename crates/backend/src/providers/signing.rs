use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use domain::provider::ModelProvider;
use llm_providers::ProviderError;
use platform::sync::Locked;
use protocol::{ProviderEvent, ServerEvent};

use super::ModelProviders;
use crate::outbox::Outbox;

const RELEASE_WAIT: Duration = Duration::from_secs(5);
const RELEASE_POLL: Duration = Duration::from_millis(10);

#[derive(Debug, Default)]
pub(super) struct InFlight {
    stop: Arc<AtomicBool>,
    ended: AtomicBool,
}

fn failed(provider: ModelProvider, message: String) -> ServerEvent {
    ServerEvent::Providers(ProviderEvent::Failed { provider, message })
}

impl ModelProviders {
    pub fn sign_in(&self, provider: ModelProvider, outbox: &Outbox) {
        if let Some(message) = self.cannot_sign_in(provider) {
            return outbox.send(failed(provider, message));
        }
        let (this, outbox) = (self.clone(), outbox.clone());
        platform::named::spawn("provider-sign-in", move || this.signing_in(provider, &outbox));
    }

    pub fn close_and_sign_in(&self, provider: ModelProvider, port: u16, pid: u32, outbox: &Outbox) {
        if let Some(message) = self.secrets.unkept().map(|reason| crate::secrets::refusal(&reason)) {
            return outbox.send(failed(provider, message));
        }
        let (this, outbox) = (self.clone(), outbox.clone());
        platform::named::spawn("provider-sign-in", move || {
            if pid != std::process::id()
                && this.ports.holder(port).is_some_and(|holder| holder.pid == pid)
                && let Err(message) = this.ports.close(port, pid)
            {
                return outbox.send(failed(provider, message));
            }
            this.signing_in(provider, &outbox);
        });
    }

    fn cannot_sign_in(&self, provider: ModelProvider) -> Option<String> {
        if !agent_engine::providers::find(provider).is_some_and(|known| known.access().subscription()) {
            return Some(format!("{} connects with an API key", agent_engine::providers::name(provider)));
        }
        self.secrets.unkept().map(|reason| crate::secrets::refusal(&reason))
    }

    fn signing_in(&self, provider: ModelProvider, outbox: &Outbox) {
        let mine = self.replacing_the_one_in_flight(provider);
        let mut open = |url: String, code: Option<String>| {
            outbox.send(ServerEvent::Providers(ProviderEvent::AuthorizeAt { provider, url, code }));
        };
        let signed = self.sign_in.sign_in(provider, &mine.stop, &mut open);
        self.ended(provider, &mine);
        if mine.stop.load(Ordering::SeqCst) {
            return;
        }
        let config = match signed {
            Err(ProviderError::PortInUse(port)) => return outbox.send(self.port_busy(provider, port)),
            other => other.map_err(|error| error.to_string()),
        };
        let result = config.and_then(|config| self.keep(provider, agent_engine::providers::name(provider), &config));
        outbox.send(self.answer(provider, result));
    }

    fn port_busy(&self, provider: ModelProvider, port: u16) -> ServerEvent {
        let holder = self.ports.holder(port);
        if holder.as_ref().is_some_and(|holder| holder.pid == std::process::id()) {
            let message = format!(
                "the sign-in callback port {port} is held by this very session, which will not close itself; \
                 quit, start again and sign in once more"
            );
            return failed(provider, message);
        }
        ServerEvent::Providers(ProviderEvent::PortBusy { provider, port, holder })
    }

    fn replacing_the_one_in_flight(&self, provider: ModelProvider) -> Arc<InFlight> {
        let mine = Arc::new(InFlight::default());
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
