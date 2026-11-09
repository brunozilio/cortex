//! The web app's client: the session on screen, what carries from frame
//! to frame, and input turned into actions with the desktop's own rules
//! (`superfik_client::input`). Pure apart from the text system the caller
//! hands in; the page does the browser's IO with what this returns.

use std::collections::HashMap;

use superfik_client::input::keys::{KeyCommand, for_platform};
use superfik_client::input::pointer::Pointer;
use superfik_client::input::press::{Pressed, dragged, pressed};
use superfik_client::input::route::{Route, route};
use superfik_client::input::scrolls::Scrolls;
use superfik_client::input::titlebar::DoubleClickAction;
use superfik_client::input::wheel::route_wheel;
use superfik_client::pages::RemotePages;
use superfik_client::state::{Action, Effect, Language, Platform, Preferences, Session};
use superfik_client::user_settings::Settings;
use superfik_client::view::camera::Camera;
use superfik_client::view::motions::Motions;
use superfik_client::view::screen::{self, Painted};
use superfik_client::view::target::{HitMap, Target};
use superfik_core::BrowserId;
use superfik_core::page::{KeyInput, PageReport};
use superfik_protocol::{LinkState, ServerEvent};
use superfik_render::{Point, TextSystem};
use superfik_ui::components::PageImage;
use superfik_ui::theme::metrics::SIDEBAR_MIN_WIDTH;
use winit::keyboard::ModifiersState;

use crate::effects::{Io, io};
use crate::picked::Picked;

mod pages;
pub use pages::PagePaint;

#[derive(Debug)]
pub struct WebClient {
    pub session: Session,
    motions: Motions,
    camera: Camera,
    hits: HitMap,
    pointer: Pointer,
    hovered: Option<Target>,
    modifiers: ModifiersState,
    scrolls: Scrolls,
    picked: Picked,
    sidebar_width: f32,
    last: Option<f64>,
    pages: RemotePages,
    scale: f32,
    page_pressed: bool,
    over_page: bool,
}

impl WebClient {
    pub fn new(mut session: Session) -> Self {
        session.platform = Platform::WEB;
        Self {
            session,
            motions: Motions::default(),
            camera: Camera::default(),
            hits: HitMap::default(),
            pointer: Pointer::new(DoubleClickAction::None),
            hovered: None,
            modifiers: ModifiersState::empty(),
            scrolls: Scrolls::default(),
            picked: Picked::default(),
            sidebar_width: SIDEBAR_MIN_WIDTH,
            last: None,
            pages: RemotePages::default(),
            scale: 1.0,
            page_pressed: false,
            over_page: false,
        }
    }

    pub fn paint(
        &mut self,
        now: f64,
        viewport: (f32, f32),
        scale: f32,
        pages: &HashMap<BrowserId, PageImage>,
        text: &mut TextSystem,
    ) -> Painted {
        let dt = self.last.map_or(0.0, |last| (now - last).clamp(0.0, 0.1)) as f32;
        self.last = Some(now);
        self.scale = scale;
        self.motions.step(dt);
        self.camera.step(dt);
        let (images, full, icons) = (HashMap::new(), HashMap::new(), HashMap::new());
        let assets = screen::Assets { images: &images, full_images: &full, pages, browser_icons: &icons };
        let pan = self.camera.offset();
        let moment = screen::Moment {
            viewport,
            scale,
            pan: (pan.x, pan.y),
            hovered: self.hovered,
            caret_visible: true,
            now,
            graph: self.scrolls.graph,
            input_scroll: self.scrolls.input,
        };
        let painted = screen::paint(&self.session, assets, moment, self.sidebar_width, &mut self.motions, text);
        self.sidebar_width = painted.sidebar_width;
        self.hits = painted.hits.clone();
        self.scrolls.painted(&self.hits, self.session.memory.open);
        painted
    }

    pub fn fit_pages(&mut self) -> Vec<Io> {
        self.fit_page()
    }

    fn act(&mut self, actions: impl IntoIterator<Item = Action>, now: f64) -> Vec<Io> {
        let mut effects: Vec<Effect> = Vec::new();
        for action in actions {
            if matches!(action, Action::Edit(_)) {
                self.scrolls.edited();
            }
            effects.extend(self.session.apply(action, now));
        }
        let (uploads, effects): (Vec<Effect>, Vec<Effect>) =
            effects.into_iter().partition(|effect| matches!(effect, Effect::Upload(_)));
        let mut sent = Vec::new();
        for effect in uploads {
            let Effect::Upload(paths) = effect else { continue };
            for upload in self.picked.uploads(paths) {
                match upload {
                    Ok(command) => sent.push(Io::Send(Box::new(command))),
                    Err(why) => sent.extend(self.act([Action::Server(ServerEvent::error(why))], now)),
                }
            }
        }
        let (effects, pages) = self.page_commands(effects);
        sent.extend(pages);
        sent.extend(io(effects, &self.session));
        sent
    }

    pub fn picked(&mut self, files: Vec<(String, Vec<u8>)>, now: f64) -> Vec<Io> {
        let known = self.picked.add(files);
        self.act([Action::AddReferences(known)], now)
    }

    pub fn event(&mut self, event: ServerEvent, locales: &[String], now: f64) -> Vec<Io> {
        match event {
            ServerEvent::Settings { settings } => {
                let settings = settings.map(Settings::from_value).unwrap_or_default();
                let saved = settings.language_saved.then_some(settings.preferences.language);
                let language = saved.or_else(|| Language::first_offered(locales)).unwrap_or_default();
                self.session.onboarded = settings.onboarded;
                let preferences = Preferences { language, ..settings.preferences };
                self.act(
                    [Action::RestoreRecents(settings.recent_workspaces), Action::RestorePreferences(preferences)],
                    now,
                )
            }
            ServerEvent::PageFrame(frame) => self.page_frame(frame),
            ServerEvent::Page { report: PageReport::Copied(text), .. } => vec![Io::Copy(text)],
            ServerEvent::Page { browser, report } => {
                self.pages.reported(browser, &report);
                self.act([Action::Server(ServerEvent::Page { browser, report })], now)
            }
            ServerEvent::Link(LinkState::Restored) => {
                let mut io: Vec<Io> = self.pages.refresh().into_iter().map(|c| Io::Send(Box::new(c))).collect();
                io.extend(self.act([Action::Server(ServerEvent::Link(LinkState::Restored))], now));
                io
            }
            event => self.act([Action::Server(event)], now),
        }
    }

    pub fn set_modifiers(&mut self, held: ModifiersState) {
        self.modifiers = held;
    }

    pub fn key(&mut self, command: KeyCommand, now: f64) -> Vec<Io> {
        self.key_for_page(command, None, now)
    }

    pub fn key_for_page(&mut self, command: KeyCommand, page: Option<KeyInput>, now: f64) -> Vec<Io> {
        if self.for_page(&command) {
            return self.key_on_page(command, page);
        }
        match route(&self.session, &self.hits, for_platform(command, self.session.platform)) {
            Route::Act(action) => self.act([action], now),
            Route::Paste(into) => vec![Io::Paste(into)],
            Route::Copy { text, then } => {
                let mut io = vec![Io::Copy(text)];
                io.extend(self.act(then, now));
                io
            }
            Route::Zoom(factor) => {
                self.camera.zoom_at(factor, Point::default());
                Vec::new()
            }
            Route::ResetView => {
                self.camera.reset();
                Vec::new()
            }
            Route::Nothing => Vec::new(),
        }
    }

    pub fn pasted(&mut self, into: fn(String) -> Action, text: String, now: f64) -> Vec<Io> {
        self.act([into(text)], now)
    }

    pub fn moved(&mut self, at: Point, now: f64) -> Vec<Io> {
        self.pointer.position = at;
        self.hovered = self.hits.at(at, self.camera.to_canvas(at));
        let actions = dragged(&self.session, &self.hits, &self.pointer, self.camera.to_canvas(at));
        let mut io = self.act(actions, now);
        io.extend(self.page_moved());
        io
    }

    pub fn press(&mut self, now: f64) -> Vec<Io> {
        let at = self.pointer.position;
        let target = self.hits.at(at, self.camera.to_canvas(at));
        let press = self.pointer.press(target, now);
        let shift = self.modifiers.shift_key();
        let mut io = match pressed(&self.session, &self.hits, press, target, self.camera.to_canvas(at), shift) {
            Pressed::Act(actions) => self.act(actions, now),
            Pressed::Recentre => {
                self.camera.reset();
                Vec::new()
            }
            Pressed::Window(_) => Vec::new(),
        };
        io.extend(self.page_press());
        io
    }

    pub fn release(&mut self) -> Vec<Io> {
        self.pointer.sliding_effort = false;
        self.pointer.selecting_text = false;
        self.page_release()
    }

    pub fn wheel(&mut self, dx: f32, dy: f32, zooming: bool, now: f64) -> Vec<Io> {
        if !zooming && let Some(io) = self.page_wheel(dx, dy) {
            return io;
        }
        let at = self.pointer.position;
        let canvas_at = self.camera.to_canvas(at);
        let wheel = route_wheel(&self.session, &self.hits, at, canvas_at, (dx, dy), zooming, self.hovered);
        let scrolled = self.scrolls.apply(wheel, (dx, dy), at);
        if let Some(factor) = scrolled.zoom {
            self.camera.zoom_at(factor, at);
        }
        if let Some((dx, dy)) = scrolled.pan {
            self.camera.pan_by(dx, dy);
        }
        self.act(scrolled.actions, now)
    }

    pub fn hovering_button(&self) -> bool {
        self.hovered.is_some_and(|target| target.is_button())
    }

    pub fn hovering_text(&self) -> bool {
        self.hovered == Some(Target::Input)
    }
}

#[cfg(test)]
mod tests;
