//! Each linked account sends from a thread of its own, one message at a
//! time, as a person would: under its ceilings (`limits`), "typing…" for a
//! while before each message, a drawn gap after it, and a failed send tried
//! again later (`pace`).

use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use superfik_channels::{Channel, OutgoingFile};
use superfik_core::channel::limits::{Ledger, Limits, PlannedSend, Planner, Priority};
use superfik_core::channel::pace::{MAX_ATTEMPTS, gap_ms, retry_delay_ms, typing_ms};
use superfik_core::channel::{Attachment, MAX_MEDIA_BYTES, Media, Message, Source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub exchange: String,
    pub text: String,
    pub priority: Priority,
    pub attachment: Option<Attachment>,
}

pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
    fn sleep(&self, duration: Duration);
    fn draw(&self) -> f64;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        crate::channels::now_ms()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn draw(&self) -> f64 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos());
        f64::from(nanos % 1_000_000) / 1_000_000.0
    }
}

pub fn spawn(
    channel: Arc<dyn Channel>,
    clock: Arc<dyn Clock>,
    limits: Limits,
    sent: Ledger,
    record: Record,
) -> Option<Sender<Outgoing>> {
    let (queue, outgoing) = crossbeam_channel::unbounded();
    std::thread::Builder::new()
        .name("superfik-channel-sender".into())
        .spawn(move || run(channel.as_ref(), clock.as_ref(), limits, sent, &outgoing, &record))
        .ok()?;
    Some(queue)
}

pub type Record = Box<dyn Fn(Message) + Send>;

pub fn run(
    channel: &dyn Channel,
    clock: &dyn Clock,
    limits: Limits,
    sent: Ledger,
    outgoing: &Receiver<Outgoing>,
    record: &dyn Fn(Message),
) {
    let mut planner = Planner::new(limits, sent, 0);
    for message in outgoing {
        let file = match message.attachment.as_ref().map(read) {
            Some(Err(problem)) => {
                tracing::warn!(%problem, "a file to send could not be read");
                continue;
            }
            Some(Ok(bytes)) => Some(bytes),
            None => None,
        };
        let send = PlannedSend { priority: message.priority, to: Some(message.exchange.clone()) };
        let at = planner.allowed_at(&send, clock.now_ms());
        wait_until(clock, at);
        let _ = channel.typing(&message.exchange);
        clock.sleep(Duration::from_millis(typing_ms(&message.text)));
        let mut attempts = 0;
        loop {
            attempts += 1;
            let sent = match (&message.attachment, &file) {
                (Some(attachment), Some(bytes)) => {
                    let file = OutgoingFile { media: &attachment.media, bytes, caption: &message.text };
                    channel.send_file(&message.exchange, &file)
                }
                _ => channel.send_text(&message.exchange, &message.text),
            };
            match sent {
                Ok(id) => {
                    record(Message {
                        id,
                        exchange: message.exchange.clone(),
                        is_group: message.exchange.contains('@'),
                        sender: None,
                        sender_name: None,
                        text: message.text.clone(),
                        media: message
                            .attachment
                            .as_ref()
                            .map(|a| Media { file: Some(a.path.clone()), ..a.media.clone() }),
                        mentioned_me: false,
                        reply_to_me: false,
                        at: clock.now_ms(),
                        source: Source::Agent,
                    });
                    break;
                }
                Err(error) if attempts < MAX_ATTEMPTS => {
                    tracing::warn!(%error, attempts, "send failed; trying again");
                    clock.sleep(Duration::from_millis(retry_delay_ms(attempts)));
                }
                Err(error) => {
                    tracing::warn!(%error, "send failed for good");
                    break;
                }
            }
        }
        planner.record(&send, clock.now_ms());
        clock.sleep(Duration::from_millis(gap_ms(clock.draw())));
    }
}

fn read(attachment: &Attachment) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(&attachment.path).map_err(|error| error.to_string())?.len();
    if size > MAX_MEDIA_BYTES {
        return Err(format!("{} is larger than {MAX_MEDIA_BYTES} bytes", attachment.path.display()));
    }
    std::fs::read(&attachment.path).map_err(|error| error.to_string())
}

fn wait_until(clock: &dyn Clock, at: i64) {
    let wait = at - clock.now_ms();
    if wait > 0 {
        clock.sleep(Duration::from_millis(u64::try_from(wait).unwrap_or(0)));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use superfik_channels::{Recorder, Sent};

    use super::*;

    #[derive(Default)]
    struct Fake {
        now: Mutex<i64>,
        slept: Mutex<Vec<u64>>,
    }

    impl Clock for Fake {
        fn now_ms(&self) -> i64 {
            *self.now.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
        }

        fn sleep(&self, duration: Duration) {
            let ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
            self.slept.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(ms);
            *self.now.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += i64::try_from(ms).unwrap_or(0);
        }

        fn draw(&self) -> f64 {
            0.0
        }
    }

    fn reply(text: &str) -> Outgoing {
        Outgoing { exchange: "5511999990000".into(), text: text.into(), priority: Priority::Reply, attachment: None }
    }

    #[test]
    fn each_message_types_then_sends_then_waits() {
        let (recorder, clock) = (Recorder::default(), Fake::default());
        let (queue, outgoing) = crossbeam_channel::unbounded();
        queue.send(reply("Olá!")).ok();
        drop(queue);
        let recorded = Mutex::new(Vec::new());
        let record =
            |message: Message| recorded.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(message);
        run(&recorder, &clock, Limits::default(), Ledger::default(), &outgoing, &record);
        let recorded = recorded.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(recorded.len(), 1, "the send is kept as the agent's");
        assert_eq!((recorded[0].source, recorded[0].text.as_str()), (Source::Agent, "Olá!"));
        let exchange = "5511999990000".to_owned();
        assert_eq!(
            recorder.sent(),
            vec![Sent::Typing { exchange: exchange.clone() }, Sent::Text { exchange, text: "Olá!".into() }]
        );
        assert_eq!(*clock.slept.lock().unwrap_or_else(std::sync::PoisonError::into_inner), vec![1_160, 2_000]);
    }

    #[test]
    fn a_full_hour_holds_the_next_message() {
        let (recorder, clock) = (Recorder::default(), Fake::default());
        let (queue, outgoing) = crossbeam_channel::unbounded();
        queue.send(reply("one")).ok();
        queue.send(reply("two")).ok();
        drop(queue);
        run(&recorder, &clock, Limits { per_hour: 1, ..Limits::default() }, Ledger::default(), &outgoing, &|_| {});
        let texts = recorder.sent().into_iter().filter(|s| matches!(s, Sent::Text { .. })).count();
        assert_eq!(texts, 2);
        assert!(clock.now_ms() >= 60 * 60 * 1_000);
    }

    #[test]
    fn sends_before_a_restart_still_count() {
        let (recorder, clock) = (Recorder::default(), Fake::default());
        *clock.now.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = 30 * 60 * 1_000;
        let (queue, outgoing) = crossbeam_channel::unbounded();
        queue.send(reply("after the restart")).ok();
        drop(queue);
        let before = Ledger { sent: vec![0], opened: vec![] };
        run(&recorder, &clock, Limits { per_hour: 1, ..Limits::default() }, before, &outgoing, &|_| {});
        let waited: u64 = clock.slept.lock().unwrap_or_else(std::sync::PoisonError::into_inner)[0];
        assert_eq!(waited, 30 * 60 * 1_000);
    }
}
