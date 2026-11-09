//! The page's start: the canvas, then a signed-in visitor connects to the
//! server that served the page (others sign in on its `/sign-in` page,
//! with their username and password or first-access code), or `?demo`
//! shows the reference scenario with no server behind it.

use std::sync::Arc;

use superfik_client::state::Session;
use superfik_protocol::{Backend, ClientCommand, Envelope, PROTOCOL_VERSION};
use superfik_remote::WebBackend;
use superfik_render::TextSystem;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use winit::event_loop::EventLoop;
use winit::keyboard::ModifiersState;
use winit::platform::web::EventLoopExtWebSys;
use winit::window::CursorIcon;

use crate::WebClient;
use crate::page::{CANVAS, Later, Page, console_error};

#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|panic| console_error(&format!("Superfik stopped: {panic}"))));
    if let Err(error) = run() {
        console_error(&format!("Superfik could not start: {error}"));
    }
}

async fn signed_in(window: &web_sys::Window) -> bool {
    answered_ok(window.fetch_with_str("/auth/session")).await
}

async fn answered_ok(request: js_sys::Promise) -> bool {
    let answer = wasm_bindgen_futures::JsFuture::from(request).await;
    answer.ok().and_then(|response| response.dyn_into::<web_sys::Response>().ok()).is_some_and(|r| r.ok())
}

pub(crate) fn connect(signed_in: bool) -> Result<Option<WebBackend>, String> {
    let location = web_sys::window().ok_or("no window")?.location();
    if !signed_in {
        location.replace("/sign-in").map_err(|_| "could not go to sign in")?;
        return Ok(None);
    }
    let backend = WebBackend::connect(&socket_url(&location)?, Arc::new(|| {}));
    for command in [ClientCommand::Hello { version: PROTOCOL_VERSION }, ClientCommand::LoadSettings] {
        backend.send(Envelope::local(command)).map_err(|error| error.to_string())?;
    }
    Ok(Some(backend))
}

fn run() -> Result<(), String> {
    let canvas: web_sys::HtmlCanvasElement = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(CANVAS))
        .ok_or("no canvas")?
        .dyn_into()
        .map_err(|_| "not a canvas")?;
    keep_browser_shortcuts(&canvas);
    let event_loop = EventLoop::<Later>::with_user_event().build().map_err(|error| error.to_string())?;
    crate::textarea::attach(&canvas, &event_loop.create_proxy()).ok_or("no text field")?;
    let window = web_sys::window().ok_or("no window")?;
    let location = window.location();
    let demo = location.search().is_ok_and(|search| search.contains("demo"));
    let session = if demo {
        superfik_client::scenarios::by_name("reference").ok_or("no scenario")?.0
    } else {
        let proxy = event_loop.create_proxy();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(window) = web_sys::window() {
                let _ = proxy.send_event(Later::SignedIn(signed_in(&window).await));
            }
        });
        Session::launching()
    };
    let locales = window.navigator().languages().iter().filter_map(|tag| tag.as_string()).collect();
    let page = Page {
        canvas,
        proxy: event_loop.create_proxy(),
        window: None,
        renderer: None,
        text: TextSystem::without_system_fonts(),
        client: WebClient::new(session),
        backend: None,
        locales,
        modifiers: ModifiersState::empty(),
        cursor: CursorIcon::Default,
        streamed: crate::streamed::StreamedPages::default(),
    };
    event_loop.spawn_app(page);
    Ok(())
}

fn socket_url(location: &web_sys::Location) -> Result<String, String> {
    let secure = location.protocol().map_err(|_| "no protocol")? == "https:";
    let host = location.host().map_err(|_| "no host")?;
    Ok(format!("{}://{host}/ws", if secure { "wss" } else { "ws" }))
}

fn keep_browser_shortcuts(canvas: &web_sys::HtmlCanvasElement) {
    let keys = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(|event: web_sys::KeyboardEvent| {
        let editing = matches!(event.key().as_str(), "a" | "c" | "v" | "x" | "z" | "Z" | "\\" | "Enter");
        if !(event.meta_key() || event.ctrl_key()) || editing {
            event.prevent_default();
        }
    });
    let _ = canvas.add_event_listener_with_callback("keydown", keys.as_ref().unchecked_ref());
    keys.forget();
}
