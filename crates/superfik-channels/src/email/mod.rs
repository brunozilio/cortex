//! E-mail: a mailbox read over IMAP (polled) and answered over SMTP.
//!
//! The account's fields are `address`, `password` (an app password for
//! Gmail and Outlook), `imap_host` and `smtp_host`. On linking, the mailbox
//! is read up to its newest message, which is not news; from then on each
//! new message in INBOX is a contact's, in the conversation named by its
//! sender's address. A reply goes to that address in the same thread
//! (`Re:` subject, `In-Reply-To`, `References`).

mod compose;
mod parse;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use parse::{Threading, parse, reply_only, reply_subject};
use superfik_core::channel::{ChannelKind, Delivery};

use compose::compose;

use crate::{Bytes, Channel, ChannelError, ChannelEvent, CloseReason, Events, LinkSpec, Opener, OutgoingFile};

const POLL: Duration = Duration::from_secs(60);
const IMAPS: u16 = 993;
const SUBMISSION: u16 = 587;

#[derive(Clone)]
struct Mailbox {
    address: String,
    password: String,
    imap_host: String,
    smtp_host: String,
}

impl Mailbox {
    fn of(spec: &LinkSpec) -> Result<Self, ChannelError> {
        let field = |key: &str| {
            spec.credentials
                .get(key)
                .map(str::to_owned)
                .ok_or_else(|| ChannelError::Failed(format!("the account has no {key}")))
        };
        Ok(Self {
            address: field("address")?,
            password: field("password")?,
            imap_host: field("imap_host")?,
            smtp_host: field("smtp_host")?,
        })
    }
}

#[derive(Debug, Default)]
pub struct EmailOpener;

impl Opener for EmailOpener {
    fn kind(&self) -> ChannelKind {
        ChannelKind::Email
    }

    fn open(&self, spec: &LinkSpec, events: Events) -> Result<Arc<dyn Channel>, ChannelError> {
        let mailbox = Mailbox::of(spec)?;
        let channel =
            Arc::new(EmailChannel { mailbox: mailbox.clone(), threads: Arc::default(), stopped: Arc::default() });
        let (threads, stopped) = (Arc::clone(&channel.threads), Arc::clone(&channel.stopped));
        std::thread::Builder::new()
            .name("superfik-email".into())
            .spawn(move || watch(&mailbox, &events, &threads, &stopped))
            .map_err(|error| ChannelError::Failed(error.to_string()))?;
        Ok(channel)
    }
}

fn watch(mailbox: &Mailbox, events: &Events, threads: &Mutex<HashMap<String, Threading>>, stopped: &AtomicBool) {
    let mut last_uid: Option<u32> = None;
    while !stopped.load(Ordering::Relaxed) {
        match read_since(mailbox, last_uid) {
            Ok((uid, messages)) => {
                if last_uid.is_none() {
                    events(ChannelEvent::Linked { account: mailbox.address.clone() });
                }
                let delivery = if last_uid.is_some() { Delivery::Live } else { Delivery::Backlog };
                for raw in messages {
                    if let Some((message, threading, file)) = parse(&raw, &mailbox.address) {
                        if let Ok(mut threads) = threads.lock() {
                            threads.insert(message.exchange.clone(), threading);
                        }
                        if let (Some(media), Some(bytes)) = (message.media.clone(), file) {
                            events(ChannelEvent::File { message: message.id.clone(), media, bytes: Bytes(bytes) });
                        }
                        events(ChannelEvent::Message { message, delivery });
                    }
                }
                last_uid = Some(uid);
            }
            Err(error) if last_uid.is_none() => {
                events(ChannelEvent::Closed(CloseReason::Failed(error)));
                return;
            }
            Err(error) => tracing::warn!(%error, "could not read the mailbox; trying again"),
        }
        std::thread::sleep(POLL);
    }
}

fn read_since(mailbox: &Mailbox, since: Option<u32>) -> Result<(u32, Vec<Vec<u8>>), String> {
    let tls = native_tls::TlsConnector::new().map_err(|e| e.to_string())?;
    let client =
        imap::connect((mailbox.imap_host.as_str(), IMAPS), &mailbox.imap_host, &tls).map_err(|e| e.to_string())?;
    let mut session = client.login(&mailbox.address, &mailbox.password).map_err(|(e, _)| e.to_string())?;
    let inbox = session.examine("INBOX").map_err(|e| e.to_string())?;
    let newest = inbox.uid_next.map_or(0, |next| next.saturating_sub(1));
    let mut messages = Vec::new();
    if let Some(since) = since.filter(|since| newest > *since) {
        let fetched = session.uid_fetch(format!("{}:{newest}", since + 1), "RFC822").map_err(|e| e.to_string())?;
        messages.extend(
            fetched
                .iter()
                .filter(|m| m.uid.is_some_and(|uid| uid > since))
                .filter_map(|m| m.body().map(<[u8]>::to_vec)),
        );
    }
    let _ = session.logout();
    Ok((newest.max(since.unwrap_or(0)), messages))
}

struct EmailChannel {
    mailbox: Mailbox,
    threads: Arc<Mutex<HashMap<String, Threading>>>,
    stopped: Arc<AtomicBool>,
}

impl EmailChannel {
    fn send(&self, exchange: &str, text: &str, file: Option<&OutgoingFile<'_>>) -> Result<String, ChannelError> {
        use lettre::{SmtpTransport, Transport};
        let threading = self.threads.lock().ok().and_then(|threads| threads.get(exchange).cloned()).unwrap_or_default();
        let id = format!("<{}@superfik>", superfik_core::RunId::new());
        let email = compose(&self.mailbox.address, exchange, &threading, &id, text, file)?;
        let credentials = lettre::transport::smtp::authentication::Credentials::new(
            self.mailbox.address.clone(),
            self.mailbox.password.clone(),
        );
        let transport = SmtpTransport::starttls_relay(&self.mailbox.smtp_host)
            .map_err(|e| ChannelError::Failed(e.to_string()))?
            .port(SUBMISSION)
            .credentials(credentials)
            .build();
        transport.send(&email).map_err(|e| ChannelError::Failed(e.to_string()))?;
        Ok(id.trim_matches(['<', '>']).to_owned())
    }
}

impl Channel for EmailChannel {
    fn send_text(&self, exchange: &str, text: &str) -> Result<String, ChannelError> {
        self.send(exchange, text, None)
    }

    fn send_file(&self, exchange: &str, file: &OutgoingFile<'_>) -> Result<String, ChannelError> {
        self.send(exchange, file.caption, Some(file))
    }

    fn typing(&self, _exchange: &str) -> Result<(), ChannelError> {
        Ok(())
    }

    fn close(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }

    fn unlink(&self) {
        self.close();
    }
}
