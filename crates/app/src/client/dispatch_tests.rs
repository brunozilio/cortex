use std::sync::atomic::AtomicUsize;

use crate::api::{BackendError, ProviderEvent};
use domain::ModelProvider;

use super::*;
use crate::client::testing::{Quiet, scripted, until, within_five_seconds};

struct Gone(Arc<AtomicUsize>);

impl Backend for Gone {
    fn send(&self, _command: Envelope<ClientCommand>) -> Result<(), BackendError> {
        Err(BackendError::Disconnected)
    }

    fn try_recv(&self) -> Option<ServerEvent> {
        self.0.fetch_add(1, Ordering::Relaxed);
        None
    }
}

fn settings(reply: Reply) -> Option<Result<Option<serde_json::Value>, String>> {
    match reply {
        Reply::Settings { settings } => Some(Ok(settings)),
        _ => None,
    }
}

fn models(reply: Reply) -> Option<Result<usize, String>> {
    match reply {
        Reply::Models(models) => Some(Ok(models.len())),
        _ => None,
    }
}

fn browser() -> ModelProvider {
    ModelProvider::new("browser-sub")
}

fn sign_in() -> ClientCommand {
    ClientCommand::SignInProvider { provider: browser() }
}

fn opened(reply: Reply) -> Option<Result<usize, String>> {
    match reply {
        Reply::Providers(ProviderEvent::Listed { providers }) => Some(Ok(providers.len())),
        Reply::Providers(ProviderEvent::Failed { message, .. }) => Some(Err(message)),
        _ => None,
    }
}

#[test]
fn a_backend_that_stopped_waking_is_not_polled_in_a_busy_loop() {
    let polled = Arc::new(AtomicUsize::new(0));
    let (wake, woken) = crossbeam_channel::bounded(1);
    drop(wake);
    let client = Dispatcher::new(Box::new(Gone(Arc::clone(&polled))), woken);
    std::thread::sleep(Duration::from_millis(600));
    drop(client);
    let polls = polled.load(Ordering::Relaxed);
    assert!(polls <= 6, "polled {polls} times in 600 ms");
}

#[test]
fn an_error_for_one_request_does_not_fail_another_concurrent_request() {
    let held = Arc::new(Mutex::new(None));
    let slot = Arc::clone(&held);
    let (client, outlet, _) = scripted(move |command, outlet| match &command.payload {
        ClientCommand::LoadSettings => *slot.locked() = Some(command.id),
        ClientCommand::ListModels => outlet.reply(command.id, Reply::error("no catalog")),
        _ => {}
    });
    let asking = Arc::clone(&client);
    let waiting = std::thread::spawn(move || asking.call(ClientCommand::LoadSettings, settings));
    let asked = until(|| *held.locked());

    let failed = client.call(ClientCommand::ListModels, models);

    assert!(matches!(&failed, Err(Error::Backend(message)) if message == "no catalog"), "{failed:?}");
    outlet.reply(asked, Reply::Settings { settings: Some(serde_json::json!({ "vim": true })) });
    let answered = waiting.join().expect("join");
    assert!(matches!(&answered, Ok(Some(kept)) if kept["vim"] == true), "{answered:?}");
}

#[test]
fn two_calls_from_two_threads_run_concurrently() {
    let earlier = Arc::new(Mutex::new(None));
    let slot = Arc::clone(&earlier);
    let (client, _, _) = scripted(move |command, outlet| match &command.payload {
        ClientCommand::ListModels => *slot.locked() = Some(command.id),
        ClientCommand::LoadSettings => {
            outlet.reply(command.id, Reply::Settings { settings: None });
            if let Some(first) = slot.locked().take() {
                outlet.reply(first, Reply::Models(Vec::new()));
            }
        }
        _ => {}
    });
    let asking = Arc::clone(&client);
    let first = std::thread::spawn(move || asking.call(ClientCommand::ListModels, models));
    until(|| earlier.locked().map(|_| ()));

    let second = within_five_seconds(move || client.call(ClientCommand::LoadSettings, settings));

    assert!(matches!(second, Ok(None)), "the second call was answered while the first still waited: {second:?}");
    let first = first.join().expect("join");
    assert!(matches!(first, Ok(0)), "{first:?}");
}

#[test]
fn an_unsolicited_event_is_delivered_even_when_no_call_is_waiting() {
    let (client, outlet, _) = scripted(|_, _| {});
    let failed = Unsolicited::CatalogFailed { provider: ModelProvider::new("keyed"), message: "401".into() };
    outlet.emit(ServerEvent::Unsolicited(failed.clone()));
    let told = until(|| Some(client.unsolicited()).filter(|told| !told.is_empty()));
    assert_eq!(told, [failed]);
    assert!(client.unsolicited().is_empty(), "told once");
}

#[test]
fn only_the_latest_unsolicited_events_are_kept_when_nobody_reads_them() {
    let (client, outlet, _) = scripted(|command, outlet| outlet.reply(command.id, Reply::Done));
    for n in 0..=UNSOLICITED_KEPT {
        let failed = Unsolicited::MemoryFailed { path: "/repo".into(), message: n.to_string() };
        outlet.emit(ServerEvent::Unsolicited(failed));
    }
    client.call(ClientCommand::LoadSettings, |_| Some(Ok(()))).expect("answered after them");
    let kept = client.unsolicited();
    assert_eq!(kept.len(), UNSOLICITED_KEPT);
    assert!(matches!(&kept[0], Unsolicited::MemoryFailed { message, .. } if message == "1"), "the oldest went first");
}

#[test]
fn a_reply_for_a_forgotten_id_is_dropped() {
    let (client, outlet, _) = scripted(|command, outlet| {
        if command.payload == ClientCommand::ListModels {
            outlet.reply(command.id, Reply::Models(Vec::new()));
        }
    });
    let forgotten = client.ask(ClientCommand::LoadSettings).expect("asked");
    let id = forgotten.id;
    drop(forgotten);

    outlet.reply(id, Reply::Settings { settings: None });
    outlet.reply(RequestId(u64::MAX), Reply::error("for a request never made"));

    let answered = client.call(ClientCommand::ListModels, models);
    assert!(matches!(answered, Ok(0)), "the next call gets its own answer: {answered:?}");
    assert!(client.shared.waiters.locked().is_empty(), "nothing is left waiting");
}

#[test]
fn an_answer_of_another_kind_ends_the_call_instead_of_leaving_it_waiting() {
    let (client, _, _) = scripted(|command, outlet| outlet.reply(command.id, Reply::Done));
    let answered = within_five_seconds(move || client.call(ClientCommand::ListModels, models));
    assert!(matches!(&answered, Err(Error::Backend(message)) if message.contains("something else")), "{answered:?}");
}

#[test]
fn a_sign_in_that_takes_longer_than_an_ordinary_answer_still_completes() {
    let (client, _, _) = scripted(|command, outlet| {
        if command.payload != sign_in() {
            return;
        }
        let (id, outlet) = (command.id, outlet.clone());
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            let url = "https://auth.test".to_owned();
            outlet.reply(id, Reply::Providers(ProviderEvent::AuthorizeAt { provider: browser(), url, code: None }));
            std::thread::sleep(Duration::from_millis(150));
            outlet.reply(id, Reply::Providers(ProviderEvent::Listed { providers: Vec::new() }));
        });
    });
    let ordinary = Duration::from_millis(50);
    let impatient = client.asking(sign_in(), false).expect("asked").answer(ordinary, opened);
    assert!(matches!(impatient, Err(Error::Timeout)), "an ordinary answer would have given up: {impatient:?}");

    let mut shown = Vec::new();
    let signed = client.flow(sign_in(), Duration::from_secs(5), |reply| match reply {
        Reply::Providers(ProviderEvent::AuthorizeAt { url, .. }) => {
            shown.push(url);
            None
        }
        other => opened(other),
    });

    assert!(matches!(signed, Ok(0)), "{signed:?}");
    assert_eq!(shown, ["https://auth.test"]);
    assert!(sign_in_patience() > ANSWER_TIMEOUT);
    assert!(sign_in_patience() > crate::api::SIGN_IN_TIMEOUT, "the backend gives up first and says why");
}

#[test]
fn a_sign_in_the_client_gave_up_on_is_cancelled_in_the_backend() {
    let (client, _, asked) = scripted(|_, _| {});
    let abandoned = client.flow(sign_in(), Duration::from_millis(50), opened);
    assert!(matches!(abandoned, Err(Error::Timeout)), "{abandoned:?}");
    let asked = asked.locked();
    let [began, cancelled] = &asked[..] else { panic!("a sign-in, then its cancellation: {asked:?}") };
    assert_eq!(began.payload, sign_in());
    assert_eq!(cancelled.payload, ClientCommand::CancelRequest { request: began.id });
}

#[test]
fn a_pending_sign_in_is_cancelled_from_another_thread_without_waiting_for_it() {
    let (client, _, asked) = scripted(|command, outlet| {
        if let ClientCommand::CancelRequest { request } = &command.payload {
            let message = "this sign-in was cancelled".to_owned();
            outlet.reply(*request, Reply::Providers(ProviderEvent::Failed { provider: browser(), message }));
            outlet.reply(command.id, Reply::Done);
        }
    });
    let signing = Arc::clone(&client);
    let waiting = std::thread::spawn(move || signing.flow(sign_in(), Duration::from_secs(30), opened));
    until(|| (!asked.locked().is_empty()).then_some(()));

    client.cancel_flows();

    let ended = within_five_seconds(move || waiting.join().expect("join"));
    assert!(matches!(&ended, Err(Error::Backend(message)) if message.contains("cancelled")), "{ended:?}");
}

#[test]
fn a_call_waiting_on_a_backend_that_went_away_fails_instead_of_waiting_out_its_time() {
    let (wake, woken) = crossbeam_channel::bounded::<()>(1);
    let client = Arc::new(Dispatcher::new(Box::new(Quiet), woken));
    let asking = Arc::clone(&client);
    let waiting = std::thread::spawn(move || asking.call(ClientCommand::LoadSettings, settings));
    until(|| (!client.shared.waiters.locked().is_empty()).then_some(()));

    drop(wake);

    let ended = within_five_seconds(move || waiting.join().expect("join"));
    assert!(matches!(&ended, Err(Error::Backend(message)) if message == SHUT_DOWN), "{ended:?}");
    let later = client.call(ClientCommand::LoadSettings, settings);
    assert!(matches!(&later, Err(Error::Backend(message)) if message == SHUT_DOWN), "{later:?}");
}

struct Dies(Arc<AtomicBool>);

impl Backend for Dies {
    fn send(&self, _command: Envelope<ClientCommand>) -> Result<(), BackendError> {
        Ok(())
    }

    fn try_recv(&self) -> Option<ServerEvent> {
        None
    }

    fn alive(&self) -> bool {
        !self.0.load(Ordering::SeqCst)
    }
}

#[test]
fn a_backend_whose_worker_died_fails_every_waiting_call_and_run_though_its_wake_is_still_held() {
    let dead = Arc::new(AtomicBool::new(false));
    let (wake, woken) = crossbeam_channel::bounded::<()>(1);
    let client = Arc::new(Dispatcher::new(Box::new(Dies(Arc::clone(&dead))), woken));
    let asking = Arc::clone(&client);
    let waiting = std::thread::spawn(move || asking.call(ClientCommand::LoadSettings, settings));
    until(|| (!client.shared.waiters.locked().is_empty()).then_some(()));
    let followed = client.follow(domain::RunId::new());

    dead.store(true, Ordering::SeqCst);
    wake.send(()).expect("woken");

    let ended = within_five_seconds(move || waiting.join().expect("join"));
    assert!(matches!(&ended, Err(Error::Backend(message)) if message == SHUT_DOWN), "{ended:?}");
    let run = within_five_seconds(move || followed.recv());
    assert!(run.is_err(), "the run's channel closed, which its reader turns into a failure: {run:?}");
    let later = client.call(ClientCommand::LoadSettings, settings);
    assert!(matches!(&later, Err(Error::Backend(message)) if message == SHUT_DOWN), "{later:?}");
}
