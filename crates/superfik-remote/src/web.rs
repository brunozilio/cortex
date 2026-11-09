//! [`WebBackend`]: the web app's [`Backend`], the page's own WebSocket to
//! the server that served it. Same origin, so the session cookie goes with
//! the upgrade and nothing here holds a token. When the connection drops
//! it says so (`ServerEvent::Link`), tries again with a growing pause,
//! greets the server again and sends what waited (`queue::Outgoing`), as
//! the desktop does.
//!
//! A page has one thread: the socket and its callbacks live in a
//! thread-local link, and the backend the app holds only names it.

use std::cell::RefCell;
use std::collections::HashMap;

use crossbeam_channel::{Receiver, Sender};
use superfik_protocol::{Backend, BackendError, ClientCommand, Envelope, LinkState, ServerEvent, Wake};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{CloseEvent, MessageEvent, WebSocket};

use crate::queue::Outgoing;

const NORMAL_CLOSE: u16 = 1000;

thread_local! {
    static LINKS: RefCell<HashMap<u32, Link>> = RefCell::new(HashMap::new());
    static NEXT: RefCell<u32> = const { RefCell::new(0) };
}

struct Link {
    url: String,
    socket: Option<WebSocket>,
    open: bool,
    outgoing: Outgoing,
    incoming: Sender<ServerEvent>,
    wake: Wake,
    callbacks: Vec<Closure<dyn FnMut(web_sys::Event)>>,
}

#[derive(Debug)]
pub struct WebBackend {
    id: u32,
    events: Receiver<ServerEvent>,
}

impl WebBackend {
    pub fn connect(url: &str, wake: Wake) -> Self {
        let (incoming, events) = crossbeam_channel::unbounded();
        let id = NEXT.with(|next| {
            let mut next = next.borrow_mut();
            *next += 1;
            *next
        });
        let link = Link {
            url: url.to_owned(),
            socket: None,
            open: false,
            outgoing: Outgoing::default(),
            incoming,
            wake,
            callbacks: Vec::new(),
        };
        LINKS.with(|links| links.borrow_mut().insert(id, link));
        dial(id);
        Self { id, events }
    }
}

impl Backend for WebBackend {
    fn send(&self, command: Envelope<ClientCommand>) -> Result<(), BackendError> {
        let json = serde_json::to_string(&command).map_err(|_| BackendError::Disconnected)?;
        let hello = matches!(command.payload, ClientCommand::Hello { .. });
        with(self.id, |link| {
            link.outgoing.queue(json, hello);
            flush(link);
        })
        .ok_or(BackendError::Disconnected)
    }

    fn try_recv(&self) -> Option<ServerEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for WebBackend {
    fn drop(&mut self) {
        let link = LINKS.with(|links| links.borrow_mut().remove(&self.id));
        if let Some(socket) = link.and_then(|link| link.socket) {
            let _ = socket.close();
        }
    }
}

fn with<T>(id: u32, act: impl FnOnce(&mut Link) -> T) -> Option<T> {
    LINKS.with(|links| links.borrow_mut().get_mut(&id).map(act))
}

fn flush(link: &mut Link) {
    let Some(socket) = link.socket.as_ref().filter(|_| link.open) else { return };
    while let Some(json) = link.outgoing.take() {
        if socket.send_with_str(&json).is_err() {
            link.outgoing.put_back(json);
            return;
        }
    }
}

fn tell(link: &Link, event: ServerEvent) {
    if link.incoming.send(event).is_ok() {
        (link.wake)();
    }
}

fn dial(id: u32) {
    let Some(url) = with(id, |link| link.url.clone()) else { return };
    let Ok(socket) = WebSocket::new(&url) else {
        retry(id);
        return;
    };
    let on_open = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
        with(id, |link| {
            link.open = true;
            if link.outgoing.opened().restored {
                tell(link, ServerEvent::Link(LinkState::Restored));
            }
            flush(link);
        });
    });
    let on_message = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let Some(text) = event.dyn_ref::<MessageEvent>().and_then(|m| m.data().as_string()) else { return };
        match serde_json::from_str::<ServerEvent>(&text) {
            Ok(event) => {
                with(id, |link| tell(link, event));
            }
            Err(error) => tracing::warn!(%error, "unreadable server event"),
        }
    });
    let on_close = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let asked = event.dyn_ref::<CloseEvent>().is_some_and(|close| close.code() == NORMAL_CLOSE);
        let still = with(id, |link| {
            link.open = false;
            link.socket = None;
            if link.outgoing.closed() {
                tell(link, ServerEvent::Link(LinkState::Lost));
            }
        });
        if still.is_some() && !asked {
            retry(id);
        }
    });
    socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    with(id, |link| {
        link.socket = Some(socket);
        link.callbacks = vec![on_open, on_message, on_close];
    });
}

fn retry(id: u32) {
    let Some(wait) = with(id, |link| link.outgoing.next_wait()) else { return };
    let again = Closure::once_into_js(move || dial(id));
    if let Some(window) = web_sys::window() {
        let wait = i32::try_from(wait).unwrap_or(i32::MAX);
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(again.unchecked_ref(), wait);
    }
}
