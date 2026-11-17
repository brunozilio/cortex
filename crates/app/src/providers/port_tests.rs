use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use domain::provider::PortHolder;
use llm_providers::ProviderError;

use super::*;

const PORT: u16 = 49153;

#[derive(Default)]
struct FakePorts {
    holder: Mutex<Option<PortHolder>>,
    closed: Mutex<Vec<u32>>,
}

impl FakePorts {
    fn held_by(pid: u32, app: &str) -> Arc<Self> {
        let ports = Self::default();
        *ports.holder.lock().expect("holder") = Some(PortHolder { pid, app: app.into() });
        Arc::new(ports)
    }

    fn closed(&self) -> Vec<u32> {
        self.closed.lock().expect("closed").clone()
    }
}

impl Ports for FakePorts {
    fn holder(&self, port: u16) -> Option<PortHolder> {
        self.holder.lock().expect("holder").clone().filter(|_| port == PORT)
    }

    fn close(&self, port: u16, pid: u32) -> Result<(), String> {
        assert_eq!(port, PORT);
        self.closed.lock().expect("closed").push(pid);
        *self.holder.lock().expect("holder") = None;
        Ok(())
    }
}

struct NeedsPort(Arc<FakePorts>);

impl SignIn for NeedsPort {
    fn sign_in(
        &self,
        provider: ModelProvider,
        stop: &Arc<AtomicBool>,
        open: &mut dyn FnMut(String, Option<String>),
    ) -> Result<Config, ProviderError> {
        if self.0.holder(PORT).is_some() {
            return Err(ProviderError::PortInUse(PORT));
        }
        FakeSignIn(Ok(())).sign_in(provider, stop, open)
    }
}

struct WaitsOnThePortFirst {
    ports: Arc<FakePorts>,
    earlier: std::sync::atomic::AtomicUsize,
}

impl SignIn for WaitsOnThePortFirst {
    fn sign_in(
        &self,
        provider: ModelProvider,
        stop: &Arc<AtomicBool>,
        open: &mut dyn FnMut(String, Option<String>),
    ) -> Result<Config, ProviderError> {
        if self.ports.holder(PORT).is_some() {
            return Err(ProviderError::PortInUse(PORT));
        }
        if self.earlier.fetch_add(1, Ordering::SeqCst) > 0 {
            return FakeSignIn(Ok(())).sign_in(provider, stop, open);
        }
        *self.ports.holder.lock().expect("holder") = Some(PortHolder { pid: std::process::id(), app: "this".into() });
        open("https://auth.test/never-answered".into(), None);
        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        *self.ports.holder.lock().expect("holder") = None;
        Err(ProviderError::Invalid("replaced".into()))
    }
}

#[test]
fn a_retry_replaces_the_sign_in_this_process_still_had_waiting_on_the_port() {
    let ports = Arc::new(FakePorts::default());
    let waits = WaitsOnThePortFirst { ports: ports.clone(), earlier: std::sync::atomic::AtomicUsize::new(0) };
    let providers = rig().providers.with_sign_in(Arc::new(waits)).with_ports(ports.clone());
    let (tx, rx) = crossbeam_channel::unbounded();
    let outbox = Outbox::new(tx, Arc::new(|| {}));
    let browser = ModelProvider::new("browser-sub");
    providers.sign_in(browser, &outbox.answering(RequestId(1)));
    assert!(matches!(
        next(&rx),
        Reply::Providers(ProviderEvent::AuthorizeAt { url, .. }) if url.ends_with("never-answered")
    ));

    providers.sign_in(browser, &outbox.answering(RequestId(2)));

    let mut replaced = Vec::new();
    let mut retried = Vec::new();
    for _ in 0..3 {
        match sent(&rx) {
            crate::api::ServerEvent::Reply { id: RequestId(1), reply } => replaced.push(reply),
            crate::api::ServerEvent::Reply { id: RequestId(2), reply } => retried.push(reply),
            other => panic!("expected an answer to one of the two sign-ins, got {other:?}"),
        }
    }
    assert!(failure(&replaced[0]).contains("replaced"), "the replaced sign-in ends for the request that began it");
    assert!(matches!(
        &retried[0],
        Reply::Providers(ProviderEvent::AuthorizeAt { url, .. }) if url.ends_with("browser-sub")
    ));
    assert!(listed(&retried[1]).iter().any(|p| p.provider == browser && p.account.is_some()));
    assert!(ports.closed().is_empty(), "no process was closed to get the port back");
    let more = rx.recv_timeout(std::time::Duration::from_millis(200));
    assert!(more.is_err(), "each sign-in ends once: {more:?}");
}

fn signing_in(ports: &Arc<FakePorts>) -> ModelProviders {
    rig().providers.with_sign_in(Arc::new(NeedsPort(ports.clone()))).with_ports(ports.clone())
}

fn busy(holder: Option<PortHolder>) -> Reply {
    Reply::Providers(ProviderEvent::PortBusy { provider: ModelProvider::new("browser-sub"), port: PORT, holder })
}

#[test]
fn a_taken_callback_port_names_the_app_holding_it() {
    let ports = FakePorts::held_by(998, "Visual Studio Code");
    let (outbox, rx) = outbox();
    signing_in(&ports).sign_in(ModelProvider::new("browser-sub"), &outbox);
    assert_eq!(next(&rx), busy(Some(PortHolder { pid: 998, app: "Visual Studio Code".into() })));
    assert!(ports.closed().is_empty());
}

#[test]
fn closing_the_holder_frees_the_port_and_the_sign_in_goes_on() {
    let ports = FakePorts::held_by(998, "Visual Studio Code");
    let providers = signing_in(&ports);
    let (outbox, rx) = outbox();
    providers.close_and_sign_in(ModelProvider::new("browser-sub"), PORT, 998, &outbox);
    assert!(matches!(next(&rx), Reply::Providers(ProviderEvent::AuthorizeAt { .. })));
    assert!(listed(&next(&rx)).iter().any(|p| p.provider == ModelProvider::new("browser-sub") && p.account.is_some()));
    assert_eq!(ports.closed(), [998]);
}

#[test]
fn a_port_now_held_by_another_process_is_not_closed_and_is_asked_about_again() {
    let ports = FakePorts::held_by(1234, "Other");
    let (outbox, rx) = outbox();
    signing_in(&ports).close_and_sign_in(ModelProvider::new("browser-sub"), PORT, 998, &outbox);
    assert_eq!(next(&rx), busy(Some(PortHolder { pid: 1234, app: "Other".into() })));
    assert!(ports.closed().is_empty());
}

#[test]
fn this_very_process_is_never_closed_to_free_the_port() {
    let ports = FakePorts::held_by(std::process::id(), "this app");
    let (outbox, rx) = outbox();
    signing_in(&ports).close_and_sign_in(ModelProvider::new("browser-sub"), PORT, std::process::id(), &outbox);
    let message = failure(&next(&rx));
    assert!(message.contains(&PORT.to_string()), "{message}");
    assert!(ports.closed().is_empty(), "closing the holder would have ended this session");
}
