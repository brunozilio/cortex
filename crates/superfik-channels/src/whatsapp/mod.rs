//! WhatsApp linked by QR code, through whatsapp-rust (MIT).
//!
//! Each account runs a `Bot` on the opener's Tokio runtime, its session in
//! a SQLite file under the account's `data_dir`. Its events become
//! `ChannelEvent`s: a pairing QR, the account connected, each message (a
//! live one or the offline drain, which `superfik_core::channel::is_news`
//! tells apart), and a logout. Messages the number sends come back too:
//! those Superfik sent (their ids are kept) are the agent's, the rest were
//! written by a member from the phone.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

mod media;

use superfik_core::channel::{ChannelKind, Delivery, Message, Source};
use whatsapp_rust::prelude::*;
use whatsapp_rust::waproto::whatsapp as wa;

use crate::{
    Bytes, Channel, ChannelError, ChannelEvent, CloseReason, Events, LinkSpec, Opener, OutgoingFile, addressed_to,
    exchange_of, recipient_of,
};

pub struct WhatsappOpener {
    runtime: tokio::runtime::Runtime,
}

impl std::fmt::Debug for WhatsappOpener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WhatsappOpener").finish_non_exhaustive()
    }
}

impl WhatsappOpener {
    pub fn new() -> Result<Self, ChannelError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("superfik-whatsapp")
            .enable_all()
            .build()
            .map_err(|error| ChannelError::Failed(error.to_string()))?;
        Ok(Self { runtime })
    }
}

impl Opener for WhatsappOpener {
    fn kind(&self) -> ChannelKind {
        ChannelKind::WhatsappWeb
    }

    fn open(&self, spec: &LinkSpec, events: Events) -> Result<Arc<dyn Channel>, ChannelError> {
        std::fs::create_dir_all(&spec.data_dir).map_err(|error| ChannelError::Failed(error.to_string()))?;
        let path = spec.data_dir.join("whatsapp.db");
        let sent: Arc<Mutex<HashSet<String>>> = Arc::default();
        let channel = Arc::new(WhatsappChannel {
            runtime: self.runtime.handle().clone(),
            bot: Mutex::new(None),
            sent: Arc::clone(&sent),
        });
        let store = self
            .runtime
            .block_on(SqliteStore::new(&path.to_string_lossy()))
            .map_err(|error| ChannelError::Failed(error.to_string()))?;
        let (qr, linked, closed) = (Arc::clone(&events), Arc::clone(&events), Arc::clone(&events));
        let bot = Bot::builder()
            .with_backend(store)
            .on_qr_code(move |code, timeout| {
                qr(ChannelEvent::Qr { code, valid_secs: timeout.as_secs() });
                async {}
            })
            .on_connected(move |client| {
                let account = client.pn().map(|jid| exchange_of(&jid.to_string())).unwrap_or_default();
                linked(ChannelEvent::Linked { account });
                async {}
            })
            .on_logged_out(move |_| {
                closed(ChannelEvent::Closed(CloseReason::LoggedOut));
                async {}
            })
            .on_event_for(&[EventKind::Messages], move |event, client| {
                let (events, sent) = (Arc::clone(&events), Arc::clone(&sent));
                async move {
                    let Event::Messages(batch) = &*event else { return };
                    let delivery = match batch.origin {
                        BatchOrigin::Live => Delivery::Live,
                        BatchOrigin::OfflineDrain => Delivery::Backlog,
                    };
                    let ours = sent.lock().map(|ids| ids.clone()).unwrap_or_default();
                    let me: Vec<String> =
                        [client.pn(), client.lid()].into_iter().flatten().map(|j| j.to_string()).collect();
                    for inbound in batch.messages.iter() {
                        let Some(message) = normalize(&inbound.info, &inbound.message, &ours, &me) else { continue };
                        if let Some(media) = message.media.clone().filter(|_| message.source == Source::Contact)
                            && let Some(bytes) = media::fetch(&client, &inbound.message).await
                        {
                            events(ChannelEvent::File { message: message.id.clone(), media, bytes: Bytes(bytes) });
                        }
                        events(ChannelEvent::Message { message, delivery });
                    }
                }
            });
        let bot = self.runtime.block_on(bot.build()).map_err(|error| ChannelError::Failed(error.to_string()))?;
        let _guard = self.runtime.enter();
        if let Ok(mut slot) = channel.bot.lock() {
            *slot = Some(bot.spawn());
        }
        Ok(channel)
    }
}

struct WhatsappChannel {
    runtime: tokio::runtime::Handle,
    bot: Mutex<Option<BotHandle>>,
    sent: Arc<Mutex<HashSet<String>>>,
}

impl WhatsappChannel {
    fn client(&self) -> Result<Arc<Client>, ChannelError> {
        let bot = self.bot.lock().map_err(|_| ChannelError::NotConnected)?;
        bot.as_ref().map(BotHandle::client).ok_or(ChannelError::NotConnected)
    }

    fn jid(exchange: &str) -> Result<Jid, ChannelError> {
        let address = recipient_of(exchange).ok_or_else(|| ChannelError::Address(exchange.to_owned()))?;
        address.parse().map_err(|_| ChannelError::Address(exchange.to_owned()))
    }

    fn take(&self) -> Option<BotHandle> {
        self.bot.lock().ok().and_then(|mut bot| bot.take())
    }
}

impl Channel for WhatsappChannel {
    fn send_text(&self, exchange: &str, text: &str) -> Result<String, ChannelError> {
        let (client, jid) = (self.client()?, Self::jid(exchange)?);
        let result = self
            .runtime
            .block_on(client.send_message(&jid, wa::Message::text(text)))
            .map_err(|error| ChannelError::Failed(error.to_string()))?;
        if let Ok(mut sent) = self.sent.lock() {
            sent.insert(result.message_id.clone());
        }
        Ok(result.message_id)
    }

    fn send_file(&self, exchange: &str, file: &OutgoingFile<'_>) -> Result<String, ChannelError> {
        let (client, jid) = (self.client()?, Self::jid(exchange)?);
        let failed = |error: whatsapp_rust::anyhow::Error| ChannelError::Failed(error.to_string());
        let unsent = |error: whatsapp_rust::SendError| ChannelError::Failed(error.to_string());
        let kind = media::media_type(file.media.kind);
        let upload = self
            .runtime
            .block_on(client.upload(file.bytes.to_vec(), kind, whatsapp_rust::UploadOptions::default()))
            .map_err(failed)?;
        let result = self
            .runtime
            .block_on(client.send_message(&jid, media::message_for(file, &upload.into())))
            .map_err(unsent)?;
        if let Ok(mut sent) = self.sent.lock() {
            sent.insert(result.message_id.clone());
        }
        Ok(result.message_id)
    }

    fn typing(&self, exchange: &str) -> Result<(), ChannelError> {
        let (client, jid) = (self.client()?, Self::jid(exchange)?);
        self.runtime
            .block_on(client.chatstate().send_composing(&jid))
            .map_err(|error| ChannelError::Failed(error.to_string()))
    }

    fn close(&self) {
        if let Some(bot) = self.take() {
            self.runtime.block_on(bot.shutdown());
        }
    }

    fn unlink(&self) {
        if let Ok(client) = self.client() {
            self.runtime.block_on(client.logout());
        }
        self.close();
    }
}

fn normalize(info: &MessageInfo, message: &wa::Message, ours: &HashSet<String>, me: &[String]) -> Option<Message> {
    let text = message.text_content().map(str::to_owned).or_else(|| media::caption_in(message)).unwrap_or_default();
    let media = media::media_in(message);
    if text.trim().is_empty() && media.is_none() {
        return None;
    }
    let source = &info.source;
    let id = info.id.to_string();
    let who = if !source.is_from_me {
        Source::Contact
    } else if ours.contains(&id) {
        Source::Agent
    } else {
        Source::Member
    };
    let (mentioned, quoted) = context_of(message.get_base_message());
    let (mentioned_me, reply_to_me) = addressed_to(&mentioned, quoted.as_deref(), me);
    Some(Message {
        exchange: exchange_of(&source.chat.to_string()),
        is_group: source.is_group,
        sender: Some(exchange_of(&source.sender.to_string())),
        sender_name: (!source.is_from_me && !info.push_name.is_empty()).then(|| info.push_name.clone()),
        text,
        media,
        mentioned_me,
        reply_to_me,
        at: info.timestamp.timestamp_millis(),
        source: who,
        id,
    })
}

fn context_of(base: &wa::Message) -> (Vec<String>, Option<String>) {
    let context = base
        .extended_text_message
        .as_option()
        .and_then(|m| m.context_info.as_option())
        .or_else(|| base.image_message.as_option().and_then(|m| m.context_info.as_option()))
        .or_else(|| base.video_message.as_option().and_then(|m| m.context_info.as_option()))
        .or_else(|| base.document_message.as_option().and_then(|m| m.context_info.as_option()))
        .or_else(|| base.audio_message.as_option().and_then(|m| m.context_info.as_option()));
    match context {
        Some(context) => (context.mentioned_jid.clone(), context.participant.clone()),
        None => (Vec::new(), None),
    }
}
