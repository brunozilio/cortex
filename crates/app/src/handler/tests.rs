use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::api::{ClientCommand, Envelope, Notices, ProviderEvent, Reply, RequestId, ServerEvent};
use domain::inbox::Notice;
use domain::provider::ModelProvider;

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
    let mut services = crate::features::context::testing::services(None);
    let unkept = Arc::new(crate::secrets::Unkept::because("this user has no data folder"));
    services.providers = services.providers.with_secrets(unkept);
    let (handler, sent) = handler(services);

    let first = taken(&handler, &sent, true);

    assert_eq!(first.for_agent, [], "what the harness says is never an event for the model");
    let [told] = &first.for_user[..] else { panic!("one notice for the user: {first:?}") };
    assert!(told.contains("keys cannot be kept on this machine: this user has no data folder"), "{told}");
    assert!(taken(&handler, &sent, true).is_empty(), "said once, not at every poll");
}

fn taken(handler: &CommandHandler, sent: &Sent, for_agent: bool) -> Notices {
    handler.handle(Envelope::local(RequestId(1), ClientCommand::TakeNotices { for_agent }));
    match answer_to(sent, 1) {
        Reply::Notices(notices) => notices,
        other => panic!("expected notices, got {other:?}"),
    }
}

#[test]
fn a_client_is_told_once_that_the_session_list_and_memory_are_not_kept_and_its_sibling_is_not_told_again() {
    let mut services = crate::features::context::testing::services(None);
    services.database = crate::Degraded::because("/data/app.db was written by a newer version");
    let (first, sent) = handler(services.clone());
    let (sibling, sent_to_sibling) = handler(services);

    let told = taken(&first, &sent, false);

    assert_eq!(
        told.for_user,
        ["The session list and memory could not be opened (/data/app.db was written by a newer version); \
          they will not be kept after this session."]
    );
    assert!(taken(&first, &sent, false).is_empty(), "said once");
    assert!(taken(&sibling, &sent_to_sibling, true).is_empty(), "once per process, not per client");
}

#[test]
fn what_waits_for_the_agent_stays_in_the_inbox_until_it_is_asked_for() {
    let (handler, sent) = handler(crate::features::context::testing::services(None));
    let arrived = Notice::new("monitor bash_4", "ERROR: disk full");
    agent_engine::tools::inbox::push(handler.client, arrived.clone());

    assert!(taken(&handler, &sent, false).is_empty(), "a look for the user's notices takes nothing from the agent");
    let asked = taken(&handler, &sent, true);
    assert_eq!((asked.for_user.len(), asked.for_agent), (0, vec![arrived]));
    assert!(taken(&handler, &sent, true).is_empty());
}

#[test]
fn a_client_becomes_a_peer_when_it_says_so_not_when_it_lists_skills() {
    let folder = tempfile::tempdir().expect("folder");
    let path = folder.path().to_path_buf();
    let (handler, sent) = handler(crate::features::context::testing::services(None));
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

    fn save(&self, _settings: &serde_json::Value) -> Result<(), crate::settings::SettingsError> {
        Err(crate::settings::SettingsError::Unwritten("the disk is read-only".into()))
    }
}

#[test]
fn a_failed_settings_save_is_answered_to_the_request_that_saved() {
    let mut services = crate::features::context::testing::services(None);
    services.settings = Arc::new(ReadOnly);
    let (handler, sent) = handler(services);

    handler.handle(Envelope::local(RequestId(4), ClientCommand::LoadSettings));
    handler.handle(Envelope::local(RequestId(5), ClientCommand::SaveSettings { settings: serde_json::json!({}) }));

    assert_eq!(answer_to(&sent, 4), Reply::Settings { settings: None });
    assert_eq!(answer_to(&sent, 5), Reply::error("Could not save the settings: the disk is read-only"));
}

#[test]
fn settings_that_were_kept_are_acknowledged() {
    let (handler, sent) = handler(crate::features::context::testing::services(None));
    handler.handle(Envelope::local(RequestId(9), ClientCommand::SaveSettings { settings: serde_json::json!({}) }));
    assert_eq!(answer_to(&sent, 9), Reply::Done);
}

#[test]
fn a_settings_change_is_applied_to_what_is_kept_and_its_failure_is_answered() {
    let (handler, sent) = handler(crate::features::context::testing::services(None));
    let change = |key: &str| ClientCommand::ChangeSettings {
        changes: vec![crate::settings::SettingsChange::set([key], serde_json::json!("on"))],
    };
    handler.handle(Envelope::local(RequestId(1), change("vim")));
    handler.handle(Envelope::local(RequestId(2), change("statusLine")));
    handler.handle(Envelope::local(RequestId(3), ClientCommand::LoadSettings));
    assert_eq!((answer_to(&sent, 1), answer_to(&sent, 2)), (Reply::Done, Reply::Done));
    let both = serde_json::json!({"vim": "on", "statusLine": "on"});
    assert_eq!(answer_to(&sent, 3), Reply::Settings { settings: Some(both) });

    let mut services = crate::features::context::testing::services(None);
    services.settings = Arc::new(ReadOnly);
    let (handler, sent) = self::handler(services);
    handler.handle(Envelope::local(RequestId(4), change("vim")));
    assert_eq!(answer_to(&sent, 4), Reply::error("Could not save the settings: the disk is read-only"));
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
    let mut services = crate::features::context::testing::services(None);
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
    let (handler, sent) = handler(crate::features::context::testing::services(None));
    handler.handle(Envelope::local(RequestId(1), ClientCommand::LoadSettings));
    handler.handle(Envelope::local(RequestId(2), ClientCommand::CancelRequest { request: RequestId(1) }));
    assert_eq!(answer_to(&sent, 1), Reply::Settings { settings: None });
    assert_eq!(answer_to(&sent, 2), Reply::Done);
}
