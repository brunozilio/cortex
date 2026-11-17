use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use domain::inbox::Notice;
use domain::provider::ModelProvider;
use protocol::{ClientCommand, Envelope, ProviderEvent, Reply, RequestId, ServerEvent};

use super::*;

type Sent = crossbeam_channel::Receiver<ServerEvent>;

fn handler(services: LocalServices) -> (CommandHandler, Sent) {
    let (events, sent) = crossbeam_channel::unbounded();
    (CommandHandler::new(services, Outbox::new(events, Arc::new(|| {}))), sent)
}

fn answer_to(sent: &Sent, id: u64) -> Reply {
    match sent.recv_timeout(Duration::from_secs(5)) {
        Ok(ServerEvent::Reply { id: answered, reply }) if answered == RequestId(id) => reply,
        other => panic!("expected an answer to request {id}, got {other:?}"),
    }
}

#[test]
fn a_client_is_told_with_its_notices_once_that_keys_cannot_be_kept() {
    let mut services = crate::feature::testing::services(None);
    let unkept = Arc::new(crate::secrets::Unkept::because("this user has no data folder"));
    services.providers = services.providers.with_secrets(unkept);
    let (handler, sent) = handler(services);
    let taken = || -> Vec<Notice> {
        handler.handle(Envelope::local(RequestId(1), ClientCommand::TakeNotices));
        match answer_to(&sent, 1) {
            Reply::Notices(notices) => notices,
            other => panic!("expected notices, got {other:?}"),
        }
    };

    let first = taken();

    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(first[0].source, "secrets");
    assert!(
        first[0].text.contains("keys cannot be kept on this machine: this user has no data folder"),
        "{}",
        first[0].text
    );
    assert!(taken().is_empty(), "said once, not at every poll");
}

#[test]
fn a_client_becomes_a_peer_when_it_says_so_not_when_it_lists_skills() {
    let folder = tempfile::tempdir().expect("folder");
    let path = folder.path().to_path_buf();
    let (handler, sent) = handler(crate::feature::testing::services(None));
    let registered = || agent_engine::tools::peers::open().into_iter().find(|peer| peer.cwd == path);

    handler.handle(Envelope::local(RequestId(1), ClientCommand::ListSkills { path: path.clone() }));
    assert!(matches!(answer_to(&sent, 1), Reply::Skills(_)));
    handler.handle(Envelope::local(RequestId(2), ClientCommand::ListAgents { path: path.clone() }));
    assert!(matches!(answer_to(&sent, 2), Reply::Agents(_)));
    assert_eq!(registered(), None, "a query registers nobody");

    handler.handle(Envelope::local(RequestId(3), ClientCommand::RegisterPeer { path: path.clone() }));
    assert_eq!(answer_to(&sent, 3), Reply::Done);
    let peer = registered().expect("registered");
    assert!(peer.name.ends_with(&format!("-{}", peer.client)), "{peer:?}");

    drop(handler);
    assert_eq!(registered(), None, "forgotten with its client");
}

struct ReadOnly;

impl crate::settings::SettingsStore for ReadOnly {
    fn load(&self) -> Option<serde_json::Value> {
        None
    }

    fn save(&self, _settings: &serde_json::Value) -> Result<(), String> {
        Err("the disk is read-only".into())
    }
}

#[test]
fn a_failed_settings_save_is_answered_to_the_request_that_saved() {
    let mut services = crate::feature::testing::services(None);
    services.settings = Arc::new(ReadOnly);
    let (handler, sent) = handler(services);

    handler.handle(Envelope::local(RequestId(4), ClientCommand::LoadSettings));
    handler.handle(Envelope::local(RequestId(5), ClientCommand::SaveSettings { settings: serde_json::json!({}) }));

    assert_eq!(answer_to(&sent, 4), Reply::Settings { settings: None });
    assert_eq!(answer_to(&sent, 5), Reply::error("Could not save the settings: the disk is read-only"));
}

#[test]
fn settings_that_were_kept_are_acknowledged() {
    let (handler, sent) = handler(crate::feature::testing::services(None));
    handler.handle(Envelope::local(RequestId(9), ClientCommand::SaveSettings { settings: serde_json::json!({}) }));
    assert_eq!(answer_to(&sent, 9), Reply::Done);
}

#[derive(Default)]
struct HoldsThePort {
    held: AtomicBool,
}

impl crate::providers::SignIn for HoldsThePort {
    fn sign_in(
        &self,
        provider: ModelProvider,
        stop: &Arc<AtomicBool>,
        open: &mut dyn FnMut(String, Option<String>),
    ) -> Result<llm_providers::Config, llm_providers::ProviderError> {
        self.held.store(true, Ordering::SeqCst);
        open(format!("https://auth.test/{}", provider.id()), None);
        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.held.store(false, Ordering::SeqCst);
        Err(llm_providers::ProviderError::Invalid("stopped".into()))
    }
}

#[test]
fn cancelling_a_sign_in_frees_the_port() {
    llm_providers::testing::install();
    let browser = ModelProvider::new("browser-sub");
    let waiting = Arc::new(HoldsThePort::default());
    let mut services = crate::feature::testing::services(None);
    services.providers = services.providers.with_sign_in(waiting.clone());
    let (handler, sent) = handler(services);
    handler.handle(Envelope::local(RequestId(1), ClientCommand::SignInProvider { provider: browser }));
    assert!(matches!(answer_to(&sent, 1), Reply::Providers(ProviderEvent::AuthorizeAt { .. })));
    assert!(waiting.held.load(Ordering::SeqCst), "the sign-in is waiting on its port");

    handler.handle(Envelope::local(RequestId(2), ClientCommand::CancelRequest { request: RequestId(1) }));

    assert!(!waiting.held.load(Ordering::SeqCst), "the port is free by the time the cancel is acknowledged");
    match answer_to(&sent, 1) {
        Reply::Providers(ProviderEvent::Failed { provider, message }) => {
            assert_eq!(provider, browser);
            assert!(message.contains("cancelled"), "{message}");
        }
        other => panic!("expected the sign-in to end, got {other:?}"),
    }
    assert_eq!(answer_to(&sent, 2), Reply::Done);
    let more = sent.recv_timeout(Duration::from_millis(100));
    assert!(more.is_err(), "each request ends once: {more:?}");
}

#[test]
fn cancelling_a_request_that_already_ended_changes_nothing() {
    let (handler, sent) = handler(crate::feature::testing::services(None));
    handler.handle(Envelope::local(RequestId(1), ClientCommand::LoadSettings));
    handler.handle(Envelope::local(RequestId(2), ClientCommand::CancelRequest { request: RequestId(1) }));
    assert_eq!(answer_to(&sent, 1), Reply::Settings { settings: None });
    assert_eq!(answer_to(&sent, 2), Reply::Done);
}
