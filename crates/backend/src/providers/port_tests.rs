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
        open: &mut dyn FnMut(String, Option<String>),
    ) -> Result<Config, ProviderError> {
        if self.0.holder(PORT).is_some() {
            return Err(ProviderError::PortInUse(PORT));
        }
        FakeSignIn(Ok(())).sign_in(provider, open)
    }
}

fn signing_in(ports: &Arc<FakePorts>) -> ModelProviders {
    rig().providers.with_sign_in(Arc::new(NeedsPort(ports.clone()))).with_ports(ports.clone())
}

fn busy(holder: Option<PortHolder>) -> ServerEvent {
    ServerEvent::Providers(ProviderEvent::PortBusy { provider: ModelProvider::new("browser-sub"), port: PORT, holder })
}

#[test]
fn a_taken_callback_port_names_the_app_holding_it() {
    let ports = FakePorts::held_by(998, "Visual Studio Code");
    let (outbox, rx) = outbox();
    signing_in(&ports).sign_in(&OWNER, ModelProvider::new("browser-sub"), &outbox);
    assert_eq!(next(&rx), busy(Some(PortHolder { pid: 998, app: "Visual Studio Code".into() })));
    assert!(ports.closed().is_empty());
}

#[test]
fn closing_the_holder_frees_the_port_and_the_sign_in_goes_on() {
    let ports = FakePorts::held_by(998, "Visual Studio Code");
    let providers = signing_in(&ports);
    let (outbox, rx) = outbox();
    providers.close_and_sign_in(&OWNER, ModelProvider::new("browser-sub"), PORT, 998, &outbox);
    assert!(matches!(next(&rx), ServerEvent::Providers(ProviderEvent::AuthorizeAt { .. })));
    assert!(listed(&next(&rx)).iter().any(|p| p.provider == ModelProvider::new("browser-sub") && p.account.is_some()));
    assert_eq!(ports.closed(), [998]);
}

#[test]
fn a_port_now_held_by_another_process_is_not_closed_and_is_asked_about_again() {
    let ports = FakePorts::held_by(1234, "Other");
    let (outbox, rx) = outbox();
    signing_in(&ports).close_and_sign_in(&OWNER, ModelProvider::new("browser-sub"), PORT, 998, &outbox);
    assert_eq!(next(&rx), busy(Some(PortHolder { pid: 1234, app: "Other".into() })));
    assert!(ports.closed().is_empty());
}
