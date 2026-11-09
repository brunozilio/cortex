//! The page: winit over `<canvas id="superfik">` for input and size, the
//! GPU behind it (made asynchronously; nothing may block a tab), a frame
//! per display refresh, and the browser's IO the client asks for (the
//! clipboard).

use superfik_client::input::keys::{KeyCommand, map_key};
use superfik_client::state::{Action, EditKey};
use superfik_protocol::{Backend, Envelope};
use superfik_remote::WebBackend;
use superfik_render::gpu::GpuContext;
use superfik_render::{Point, Renderer, TextSystem};
use wasm_bindgen::prelude::*;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::platform::web::WindowAttributesExtWebSys;
use winit::window::{CursorIcon, Window, WindowId};

use crate::{Io, WebClient};

pub(crate) const CANVAS: &str = "superfik";
const LINE: f32 = 20.0;

pub(crate) enum Later {
    Gpu(Result<GpuContext, String>),
    Pasted(fn(String) -> Action, String),
    SignedIn(bool),
    Picked(Vec<(String, Vec<u8>)>),
    Key(KeyCommand, Option<superfik_core::page::KeyInput>),
    PageKey(superfik_core::page::KeyInput),
    Text(String),
    Modifiers(ModifiersState),
    Face(superfik_render::Script, Vec<u8>),
}

pub(crate) struct Page {
    pub canvas: web_sys::HtmlCanvasElement,
    pub proxy: EventLoopProxy<Later>,
    pub window: Option<Window>,
    pub renderer: Option<Renderer>,
    pub text: TextSystem,
    pub client: WebClient,
    pub backend: Option<WebBackend>,
    pub locales: Vec<String>,
    pub modifiers: ModifiersState,
    pub cursor: CursorIcon,
    pub streamed: crate::streamed::StreamedPages,
}

pub(crate) fn console_error(message: &str) {
    web_sys::console::error_1(&JsValue::from_str(message));
}

fn now() -> f64 {
    web_sys::window().and_then(|window| window.performance()).map_or(0.0, |performance| performance.now() / 1000.0)
}

impl Page {
    fn scale(&self) -> f64 {
        self.window.as_ref().map_or(1.0, Window::scale_factor)
    }

    fn run_io(&mut self, io: Vec<Io>) {
        for io in io {
            match io {
                Io::Send(command) => {
                    let Some(backend) = &self.backend else { continue };
                    if let Err(error) = backend.send(Envelope::local(*command)) {
                        web_sys::console::warn_1(&JsValue::from_str(&error.to_string()));
                    }
                }
                Io::Open(url) => {
                    if let Some(window) = web_sys::window() {
                        let _ = window.open_with_url_and_target(&url, "_blank");
                    }
                }
                Io::Copy(text) => {
                    if let Some(window) = web_sys::window() {
                        let _ = window.navigator().clipboard().write_text(&text);
                    }
                }
                Io::Paste(into) => self.read_clipboard(into),
                Io::PickFiles => crate::files::pick(self.proxy.clone()),
                Io::PaintPage(paint) => {
                    let scale = self.scale() as f32;
                    let Some(renderer) = &mut self.renderer else { continue };
                    let id = paint.id;
                    if !self.streamed.paint(renderer, paint, scale)
                        && let Some(backend) = &self.backend
                    {
                        let refresh = superfik_core::page::PageCall::Refresh(id);
                        let _ = backend.send(Envelope::local(superfik_protocol::ClientCommand::Page(refresh)));
                    }
                }
            }
        }
    }

    fn read_clipboard(&self, into: fn(String) -> Action) {
        let Some(window) = web_sys::window() else { return };
        let read = wasm_bindgen_futures::JsFuture::from(window.navigator().clipboard().read_text());
        let proxy = self.proxy.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(text) = read.await
                && let Some(text) = text.as_string()
            {
                let _ = proxy.send_event(Later::Pasted(into, text));
            }
        });
    }

    fn show_cursor(&mut self) {
        let cursor = if let Some(page) = self.client.page_cursor() {
            superfik_client::input::page_keys::cursor(page)
        } else if self.client.hovering_text() {
            CursorIcon::Text
        } else if self.client.hovering_button() {
            CursorIcon::Pointer
        } else {
            CursorIcon::Default
        };
        if cursor != self.cursor
            && let Some(window) = &self.window
        {
            window.set_cursor(cursor);
            self.cursor = cursor;
        }
    }

    fn drain_server(&mut self) {
        let mut events = Vec::new();
        while let Some(event) = self.backend.as_ref().and_then(Backend::try_recv) {
            events.push(event);
        }
        for event in events {
            let io = self.client.event(event, &self.locales, now());
            self.run_io(io);
        }
    }

    fn render(&mut self) {
        self.drain_server();
        let (Some(window), Some(renderer)) = (&self.window, &mut self.renderer) else { return };
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        let viewport = (size.width as f32 / scale, size.height as f32 / scale);
        let at = now();
        let client = &self.client;
        self.streamed.retain(renderer, |id| client.page_open(id));
        let painted = self.client.paint(at, viewport, scale, self.streamed.images(), &mut self.text);
        let passes = [(&painted.canvas, scale), (&painted.sidebar, scale), (&painted.overlay, scale)];
        if let Err(error) = renderer.render_passes(&passes, &mut self.text, at as f32) {
            web_sys::console::warn_1(&JsValue::from_str(&error.to_string()));
        }
        for script in self.text.take_missing() {
            crate::fonts::fetch(script, self.proxy.clone());
        }
        window.request_redraw();
        let io = self.client.fit_pages();
        self.run_io(io);
    }
}

impl ApplicationHandler<Later> for Page {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes =
            Window::default_attributes().with_canvas(Some(self.canvas.clone())).with_prevent_default(false);
        let window = match event_loop.create_window(attributes) {
            Ok(window) => window,
            Err(error) => return console_error(&error.to_string()),
        };
        let size = window.inner_size();
        let (canvas, proxy) = (self.canvas.clone(), self.proxy.clone());
        wasm_bindgen_futures::spawn_local(async move {
            let gpu = GpuContext::new_async(wgpu::SurfaceTarget::Canvas(canvas), size.width.max(1), size.height.max(1))
                .await
                .map_err(|error| format!("{error} (this browser needs WebGPU)"));
            let _ = proxy.send_event(Later::Gpu(gpu));
        });
        self.window = Some(window);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: Later) {
        match event {
            Later::Gpu(Ok(gpu)) => {
                let mut renderer = Renderer::new(gpu);
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    renderer.resize(size.width.max(1), size.height.max(1));
                    window.request_redraw();
                }
                self.renderer = Some(renderer);
            }
            Later::Gpu(Err(error)) => console_error(&error),
            Later::SignedIn(signed_in) => match crate::launch::connect(signed_in) {
                Ok(backend) => self.backend = backend,
                Err(error) => console_error(&error),
            },
            Later::Key(command, page) => {
                let io = self.client.key_for_page(command, page, now());
                self.run_io(io);
            }
            Later::PageKey(key) => {
                let io = self.client.page_key(key);
                self.run_io(io);
            }
            Later::Text(text) => {
                let io = self.client.key(KeyCommand::Edit(EditKey::Insert(text)), now());
                self.run_io(io);
            }
            Later::Modifiers(modifiers) => {
                self.modifiers = modifiers;
                self.client.set_modifiers(modifiers);
            }
            Later::Face(script, bytes) => {
                if !self.text.add_face(script, bytes) {
                    console_error(&format!("the font fetched for {script:?} has no face"));
                }
            }
            Later::Picked(files) => {
                let io = self.client.picked(files, now());
                self.run_io(io);
            }
            Later::Pasted(into, text) => {
                let io = self.client.pasted(into, text, now());
                self.run_io(io);
            }
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width.max(1), size.height.max(1));
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
                self.client.set_modifiers(self.modifiers);
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let command = map_key(&event.logical_key, event.text.as_deref(), self.modifiers);
                let io = self.client.key(command, now());
                self.run_io(io);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.scale();
                let at = Point::new((position.x / scale) as f32, (position.y / scale) as f32);
                let io = self.client.moved(at, now());
                self.run_io(io);
                self.show_cursor();
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => match state {
                ElementState::Pressed => {
                    let io = self.client.press(now());
                    self.run_io(io);
                }
                ElementState::Released => {
                    let io = self.client.release();
                    self.run_io(io);
                }
            },
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * LINE, y * LINE),
                    MouseScrollDelta::PixelDelta(p) => {
                        let scale = self.scale();
                        ((p.x / scale) as f32, (p.y / scale) as f32)
                    }
                };
                let zooming = self.modifiers.super_key() || self.modifiers.control_key();
                let io = self.client.wheel(dx, dy, zooming, now());
                self.run_io(io);
            }
            _ => {}
        }
    }
}
