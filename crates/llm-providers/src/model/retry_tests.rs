use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use super::*;
use crate::transcript::{Message, StopReason, Usage};

fn reply() -> Reply {
    Reply { message: Message::assistant(Vec::new()), stop: StopReason::EndTurn, usage: Usage::default(), served: None }
}

fn request() -> TurnRequest {
    TurnRequest {
        model: "m".into(),
        system: String::new(),
        messages: Vec::new(),
        tools: Vec::new(),
        effort: domain::Effort::Medium,
        max_tokens: 10,
    }
}

struct Script {
    answers: Mutex<VecDeque<Result<Reply, Failed>>>,
    asked: AtomicUsize,
    speaks_first: bool,
    asked_at: Mutex<Vec<Instant>>,
}

impl Script {
    fn new(answers: impl IntoIterator<Item = Result<Reply, Failed>>) -> Self {
        Self {
            answers: Mutex::new(answers.into_iter().collect()),
            asked: AtomicUsize::new(0),
            speaks_first: false,
            asked_at: Mutex::default(),
        }
    }

    fn asked(&self) -> usize {
        self.asked.load(Ordering::SeqCst)
    }
}

impl Attempt for Script {
    fn once<'a>(
        &'a self,
        _request: &'a TurnRequest,
        on: OnDelta<'a>,
        _cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, Failed>> {
        Box::pin(async move {
            self.asked.fetch_add(1, Ordering::SeqCst);
            self.asked_at.lock().expect("times").push(Instant::now());
            if self.speaks_first {
                on(Delta::Text("half".into()));
            }
            self.answers
                .lock()
                .expect("answers")
                .pop_front()
                .unwrap_or_else(|| Err(ModelError::Overloaded("busy".into()).into()))
        })
    }
}

fn ask(script: &Script, backoff: Duration, cancel: &CancellationToken) -> Result<Reply, ModelError> {
    platform::runtime::block_on(with_retries("P", backoff, script, &request(), &mut |_| {}, cancel)).expect("runtime")
}

fn busy() -> Result<Reply, Failed> {
    Err(ModelError::Overloaded("busy".into()).into())
}

#[test]
fn a_busy_provider_is_asked_again_until_it_answers() {
    let script = Script::new([busy(), busy(), Ok(reply())]);
    assert!(ask(&script, Duration::ZERO, &CancellationToken::new()).is_ok());
    assert_eq!(script.asked(), 3);
}

#[test]
fn an_answer_already_shown_is_never_asked_again() {
    let script = Script { speaks_first: true, ..Script::new([Err(ModelError::Network("dropped".into()).into())]) };
    assert!(matches!(ask(&script, Duration::ZERO, &CancellationToken::new()), Err(ModelError::Network(_))));
    assert_eq!(script.asked(), 1);
}

#[test]
fn a_provider_busy_every_time_gives_up() {
    let script = Script::new([]);
    assert!(matches!(ask(&script, Duration::ZERO, &CancellationToken::new()), Err(ModelError::Overloaded(_))));
    assert_eq!(script.asked(), 4);
}

#[test]
fn cancelling_during_the_wait_ends_it_at_once() {
    let script = Script::new([]);
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        canceller.cancel();
    });
    let started = Instant::now();
    assert_eq!(ask(&script, Duration::from_secs(600), &cancel).map(|_| ()), Err(ModelError::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(5), "the wait went on: {:?}", started.elapsed());
}

#[test]
fn the_wait_the_server_asks_for_is_kept_instead_of_the_backoff() {
    let asked_for = Duration::from_millis(120);
    let script = Script::new([
        Err(Failed { error: ModelError::Overloaded("429".into()), retry_after: Some(asked_for) }),
        Ok(reply()),
    ]);
    assert!(ask(&script, Duration::ZERO, &CancellationToken::new()).is_ok());
    let times = script.asked_at.lock().expect("times").clone();
    assert!(times[1].duration_since(times[0]) >= asked_for, "{:?}", times[1].duration_since(times[0]));
}

#[test]
fn a_server_that_asks_for_a_longer_wait_than_is_kept_gets_the_error_at_once() {
    let long = Some(RETRY_AFTER_AT_MOST + Duration::from_secs(1));
    let script = Script::new([Err(Failed { error: ModelError::Overloaded("429".into()), retry_after: long })]);
    assert_eq!(
        ask(&script, Duration::ZERO, &CancellationToken::new()).map(|_| ()),
        Err(ModelError::Overloaded("429".into()))
    );
    assert_eq!(script.asked(), 1);
}

#[test]
fn the_backoff_is_jittered_between_half_and_all_of_the_wait() {
    let wait = Duration::from_secs(2);
    assert_eq!(jittered(wait, 0), Duration::from_secs(1));
    assert_eq!(jittered(wait, u64::try_from(Duration::from_secs(1).as_nanos()).expect("fits")), wait);
    for random in [7, 1_000_000_007, u64::MAX] {
        let paused = jittered(wait, random);
        assert!(paused >= wait / 2 && paused <= wait, "{paused:?}");
    }
    assert_eq!(jittered(Duration::ZERO, 99), Duration::ZERO);
    assert_eq!(pause(wait, Some(Duration::from_secs(3))), Some(Duration::from_secs(3)));
}

#[test]
fn retry_after_is_read_in_milliseconds_seconds_or_as_a_date() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_445_412_470);
    let after = |name: &str, value: &str| retry_after(&[(name.to_owned(), value.to_owned())], now);
    assert_eq!(after("retry-after-ms", "1500"), Some(Duration::from_millis(1500)));
    assert_eq!(after("Retry-After", "7"), Some(Duration::from_secs(7)));
    assert_eq!(after("retry-after", "Wed, 21 Oct 2015 07:28:00 GMT"), Some(Duration::from_secs(10)));
    assert_eq!(after("retry-after", "Wed, 21 Oct 2015 07:00:00 GMT"), Some(Duration::ZERO));
    assert_eq!(after("retry-after", "soon"), None);
    assert_eq!(retry_after(&[], now), None);
}

#[test]
fn an_error_answer_without_end_is_read_only_as_far_as_its_first_part() {
    let endless = std::io::BufReader::new(std::io::repeat(b'x'));
    let response = super::super::http::Response {
        status: 500,
        headers: Vec::new(),
        body: super::super::http::Body::reader(endless),
    };
    static SAID: AtomicUsize = AtomicUsize::new(usize::MAX);
    fn judge(_: u16, text: &str) -> ModelError {
        SAID.store(text.len(), Ordering::SeqCst);
        ModelError::Provider(String::new())
    }
    let started = Instant::now();
    let failed = platform::runtime::block_on(refusal(response, &CancellationToken::new(), judge)).expect("runtime");
    assert!(matches!(failed.error, ModelError::Provider(_)));
    assert!(SAID.load(Ordering::SeqCst) <= ERROR_BODY_AT_MOST, "{}", SAID.load(Ordering::SeqCst));
    assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
}

#[test]
fn a_short_answer_without_end_is_refused_past_its_limit() {
    let endless = std::io::BufReader::new(std::io::repeat(b'x'));
    let response = super::super::http::Response {
        status: 200,
        headers: Vec::new(),
        body: super::super::http::Body::reader(endless),
    };
    let read = platform::runtime::block_on(response.text(&CancellationToken::new())).expect("runtime");
    assert!(read.is_err());
}

#[test]
fn a_retry_after_too_large_to_be_a_duration_gives_up_instead_of_panicking() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_445_412_470);
    let after = |value: &str| retry_after(&[("retry-after-ms".to_owned(), value.to_owned())], now);
    for huge in ["1e300", "1.8e22", &f64::MAX.to_string()] {
        let asked = after(huge).expect("a wait was asked");
        assert!(asked > RETRY_AFTER_AT_MOST, "{huge}: {asked:?}");
        assert_eq!(pause(Duration::from_secs(1), Some(asked)), None, "{huge}: longer than the app waits");
    }
    for unusable in ["-5", "NaN", "inf", "-inf"] {
        assert_eq!(after(unusable), None, "{unusable}");
    }
    assert_eq!(after("60000"), Some(RETRY_AFTER_AT_MOST));
}
