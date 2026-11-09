//! The loop running pages for clients, and the service handing it calls.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use superfik_backend::{Outbox, PageService};
use superfik_browser::{BrowserHost, Call};
use superfik_core::page::{PageCall, PageReport};
use superfik_core::{BrowserId, Principal};
use superfik_frames::{JpegTiles, Order, PageEncoder, Pixels};
use superfik_protocol::ServerEvent;

use crate::owners::{Admitted, Owners};
use crate::readback::Gpu;

const FRAME_GAP: Duration = Duration::from_millis(33);

#[derive(Debug)]
pub struct Request {
    pub principal: Principal,
    pub call: PageCall,
    pub answer: Outbox,
}

#[derive(Debug, Clone)]
pub struct ServerPages {
    calls: Sender<Request>,
}

impl PageService for ServerPages {
    fn call(&self, principal: &Principal, call: PageCall, answer: Outbox) {
        let _ = self.calls.send(Request { principal: principal.clone(), call, answer });
    }
}

struct Stream {
    encoder: Box<dyn PageEncoder>,
    sent: Option<(u64, Instant)>,
    whole: bool,
}

pub struct PageLoop {
    host: Box<dyn BrowserHost>,
    gpu: Option<Gpu>,
    calls: Receiver<Request>,
    owners: Owners,
    answers: HashMap<BrowserId, Outbox>,
    streams: HashMap<BrowserId, Stream>,
}

impl std::fmt::Debug for PageLoop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageLoop").field("pages", &self.answers.len()).finish_non_exhaustive()
    }
}

pub fn pages(host: Box<dyn BrowserHost>, gpu: Option<Gpu>) -> (ServerPages, PageLoop) {
    let (calls, waiting) = crossbeam_channel::unbounded();
    let page_loop = PageLoop {
        host,
        gpu,
        calls: waiting,
        owners: Owners::default(),
        answers: HashMap::new(),
        streams: HashMap::new(),
    };
    (ServerPages { calls }, page_loop)
}

impl PageLoop {
    pub fn warm_up(&mut self) {
        if self.gpu.is_some() {
            self.host.warm_up();
        }
    }

    pub fn is_active(&self) -> bool {
        !self.answers.is_empty() || self.host.is_active()
    }

    pub fn step(&mut self, now: Instant) {
        let waiting: Vec<Request> = self.calls.try_iter().collect();
        for request in waiting {
            self.take(request);
        }
        let Some(gpu) = &mut self.gpu else { return };
        for (id, event) in self.host.pump(&gpu.device, &gpu.queue) {
            if let (Some(report), Some(answer)) = (event.report(), self.answers.get(&id)) {
                answer.send(ServerEvent::Page { browser: id, report });
            }
        }
        for (id, stream) in &mut self.streams {
            let Some(frame) = self.host.frame(*id) else { continue };
            let due = stream
                .sent
                .is_none_or(|(generation, at)| generation != frame.generation && now.duration_since(at) >= FRAME_GAP);
            if !(due || stream.whole) {
                continue;
            }
            let Some(read) = gpu.read(&frame) else { continue };
            let pixels = Pixels {
                data: &read.data,
                width: read.width,
                height: read.height,
                stride: read.stride,
                order: Order::Rgba,
            };
            let encoded = stream.encoder.encode(*id, &pixels, stream.whole);
            stream.sent = Some((frame.generation, now));
            stream.whole = false;
            if let (Some(frame), Some(answer)) = (encoded, self.answers.get(id)) {
                tracing::trace!(%id, tiles = frame.tiles.len(), bytes = frame.bytes(), "page frame");
                answer.send(ServerEvent::PageFrame(frame));
            }
        }
    }

    fn take(&mut self, Request { principal, call, answer }: Request) {
        let id = call.page();
        if self.gpu.is_none() {
            if let PageCall::Open { url, .. } = call {
                let error = "this server could not start its browser".to_owned();
                answer.send(ServerEvent::Page { browser: id, report: PageReport::Failed { url, error } });
            }
            return;
        }
        match self.owners.admit(&principal, call) {
            Admitted::Refused => tracing::warn!(%id, "a page call from someone else's client was refused"),
            Admitted::Forbidden(report) => {
                tracing::warn!(%id, "a page was kept from an address off the public web");
                answer.send(ServerEvent::Page { browser: id, report });
            }
            Admitted::Refresh(id) => {
                if let Some(stream) = self.streams.get_mut(&id) {
                    stream.whole = true;
                }
            }
            Admitted::Call(call) => {
                match &call {
                    Call::Open { .. } => {
                        self.answers.insert(id, answer);
                        let stream = Stream { encoder: Box::new(JpegTiles::default()), sent: None, whole: true };
                        self.streams.insert(id, stream);
                    }
                    Call::Close(_) => {
                        self.answers.remove(&id);
                        self.streams.remove(&id);
                    }
                    _ => {}
                }
                self.host.send(call);
            }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn run(mut self, done: impl Fn() -> bool) {
        self.warm_up();
        while !done() {
            self.step(Instant::now());
            let rest = if self.is_active() { 8 } else { 50 };
            superfik_browser::cef::spin(Duration::from_millis(rest));
        }
    }
}

#[cfg(test)]
mod tests;
