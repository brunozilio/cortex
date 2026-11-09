//! One open page: the CEF producer behind it, its latest frame, what it
//! last reported, and the calls and input it takes.

use welding::macos_cef::MacosCefProducer;
use welding::{CefSurfaceEvent, CefSurfaceProducer, FocusDirection, HostWgpuContext, ImportedTexture};

use super::convert::{self, Held};
use super::{agent, log, log_ok};
use crate::{Call, EditCommand, KeyInput, KeyPhase, Modifiers, PageEvent, PageInput, PointerButton};

const EDIT_SCRIPT: u64 = u64::MAX;

pub(super) struct Page {
    pub(super) producer: MacosCefProducer,
    pub(super) frame: Option<ImportedTexture>,
    held: Held,
    pub(super) history: (bool, bool),
    loads: convert::Loads,
    pub(super) loading: bool,
    pub(super) load_revision: u64,
    pub(super) title: String,
    pub(super) url: String,
    pub(super) failure: Option<String>,
    pub(super) size: (u32, u32),
    copying: Option<welding::WebRequestId>,
    next_edit_script: u64,
    pub(super) jobs: agent::Jobs,
    pub(super) automation: super::automation::Automation,
    pub(super) files: super::files::FileArea,
    pub(super) filling: Option<super::fill::Filling>,
    pub(super) fill_answers: Vec<superfik_core::autofill::FillOutcome>,
    pub(super) cdp: crate::devtools::Cdp,
    pub(super) devtools_ready: bool,
    pub(super) passkeys: super::passkeys::Passkeys,
    guarded: bool,
}

impl Page {
    pub(super) fn new(producer: MacosCefProducer, size: (u32, u32), files: super::files::FileArea) -> Self {
        Self {
            producer,
            frame: None,
            held: Held::default(),
            history: (false, false),
            loads: convert::Loads::default(),
            loading: false,
            load_revision: 0,
            title: String::new(),
            url: String::new(),
            failure: None,
            size,
            copying: None,
            next_edit_script: EDIT_SCRIPT,
            jobs: agent::Jobs::default(),
            automation: super::automation::Automation::default(),
            files,
            filling: None,
            fill_answers: Vec::new(),
            cdp: crate::devtools::Cdp::default(),
            devtools_ready: false,
            passkeys: super::passkeys::Passkeys::default(),
            guarded: false,
        }
    }

    pub(super) fn guarded(mut self, guarded: bool) -> Self {
        self.guarded = guarded;
        self
    }

    pub(super) fn may_go(&self, url: &str) -> Result<(), String> {
        if !self.guarded {
            return Ok(());
        }
        crate::address::server_may_open(url).map_err(|refused| refused.to_string())
    }

    fn go(&mut self, url: &str) {
        match self.may_go(url) {
            Ok(()) => log(self.producer.navigate_to_url(url)),
            Err(error) => tracing::warn!(%error, "a page was kept from an address"),
        }
    }

    pub(super) fn apply(&mut self, call: Call) {
        if let Call::Navigate(_, url) = &call {
            return self.go(url);
        }
        let p = &mut self.producer;
        match call {
            Call::Navigate(..) => {}
            Call::Back(_) => log(p.go_back()),
            Call::Forward(_) => log(p.go_forward()),
            Call::Reload(_) => log(p.reload()),
            Call::Stop(_) => log(p.stop()),
            Call::Resize { size, scale, .. } => {
                self.size = size;
                log(p.set_scale_factor(scale));
                log(p.resize(size.into()));
            }
            Call::Focus(_, true) => log(p.move_focus(FocusDirection::Forward)),
            Call::Focus(_, false) => {}
            Call::Input(_, input) => self.input(&input),
            Call::Agent { request, op, .. } => self.queue(request, op),
            Call::FillLogin { url, fill, .. } => self.start_fill(url, fill),
            Call::SetCookies { cookies, .. } => {
                for cookie in &cookies {
                    log(p.set_cookie(&cookie.url(), &convert::cookie(cookie)));
                }
            }
            Call::ClearCookies(_) => log(p.delete_cookies(None, None)),
            Call::AnswerPasskey { request, reply, .. } => self.answer_passkey(request, &reply),
            Call::Open { .. } | Call::Close(_) => {}
        }
    }

    pub(super) fn input(&mut self, input: &PageInput) {
        let p = &mut self.producer;
        match input {
            PageInput::Key(key) => convert::keys(key).into_iter().for_each(|e| log(p.send_keyboard_input(e))),
            PageInput::Text(text) => convert::text(text).into_iter().for_each(|e| log(p.send_keyboard_input(e))),
            PageInput::Edit(command) => {
                let id = welding::WebRequestId::new(self.next_edit_script);
                self.next_edit_script = self.next_edit_script.wrapping_sub(1);
                if log_ok(p.request_script_result(id, &crate::agent::edit_script(command)))
                    && matches!(command, EditCommand::Copy | EditCommand::Cut)
                {
                    self.copying = Some(id);
                }
            }
            pointer => {
                if let Some(event) = convert::mouse(pointer, &mut self.held) {
                    log(p.send_mouse_input(event));
                }
            }
        }
    }

    pub(super) fn click(&mut self, x: f32, y: f32) {
        let scale = self.producer.scale_factor();
        let (x, y) = ((x * scale).round() as i32, (y * scale).round() as i32);
        let modifiers = Modifiers::default();
        self.input(&PageInput::Move { x, y, modifiers });
        self.input(&PageInput::Press { x, y, button: PointerButton::Left, modifiers });
        self.input(&PageInput::Release { x, y, button: PointerButton::Left, modifiers });
    }

    pub(super) fn type_text(&mut self, text: &str, submit: bool) {
        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                self.press_named("Enter");
            }
            if !line.is_empty() {
                self.input(&PageInput::Text(line.to_owned()));
            }
        }
        if submit {
            self.press_named("Enter");
        }
    }

    pub(super) fn press_named(&mut self, name: &str) {
        if let Some(down) = crate::agent::named_key(name) {
            self.press(down);
        }
    }

    pub(super) fn press(&mut self, down: KeyInput) {
        let up = KeyInput { phase: KeyPhase::Up, text: None, ..down.clone() };
        self.input(&PageInput::Key(down));
        self.input(&PageInput::Key(up));
    }

    pub(super) fn drain(&mut self, context: &HostWgpuContext, events: &mut Vec<PageEvent>) {
        match self.producer.acquire_frame(context) {
            Ok(Some(frame)) => self.frame = Some(frame),
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "could not take the page's frame"),
        }
        let now = std::time::Instant::now();
        while let Some(event) = self.producer.poll_web_event() {
            match event {
                CefSurfaceEvent::Navigation(navigation) => {
                    self.files.observe(&navigation);
                    if let welding::NavigationEvent::NewWindowRequested { url, .. } = &navigation {
                        let url = url.clone();
                        self.go(&url);
                    }
                    for event in self.loads.event(navigation) {
                        let event = self.kept_on_the_web(event);
                        self.note(&event);
                        events.push(event);
                    }
                }
                CefSurfaceEvent::ScriptCompleted { id, result } if self.copying == Some(id) => {
                    self.copying = None;
                    if let Ok(json) = result {
                        events.push(PageEvent::Copied(crate::agent::readable(&json)));
                    }
                }
                CefSurfaceEvent::ScriptCompleted { .. } => {}
                _ => {}
            }
        }
        if let Some(shape) = self.producer.poll_cursor_shape() {
            events.push(PageEvent::Cursor(convert::cursor(shape)));
        }
        let history = (self.producer.can_go_back(), self.producer.can_go_forward());
        if history != self.history {
            self.history = history;
            events.push(PageEvent::History { back: history.0, forward: history.1 });
        }
        self.drain_devtools(events);
        if let Some(outcome) = self.fill_overdue(now) {
            self.fill_answers.push(outcome);
        }
        events.extend(self.fill_answers.drain(..).map(PageEvent::LoginFilled));
        for (request, answer) in self.advance(now) {
            events.push(PageEvent::Answered { request, answer });
        }
    }

    fn kept_on_the_web(&mut self, event: PageEvent) -> PageEvent {
        let PageEvent::Address(url) = &event else { return event };
        let Err(error) = self.may_go(url) else { return event };
        log(self.producer.stop());
        log(self.producer.navigate_to_url("about:blank"));
        PageEvent::Failed { url: url.clone(), error }
    }

    fn note(&mut self, event: &PageEvent) {
        match event {
            PageEvent::Address(url) => self.url.clone_from(url),
            PageEvent::Title(title) => self.title.clone_from(title),
            PageEvent::Loading(loading) => {
                self.loading = *loading;
                if *loading {
                    self.load_revision = self.load_revision.saturating_add(1);
                    self.failure = None;
                }
            }
            PageEvent::Failed { error, .. } => self.failure = Some(error.clone()),
            PageEvent::Crashed => self.failure = Some("the page crashed".to_owned()),
            PageEvent::History { .. }
            | PageEvent::Answered { .. }
            | PageEvent::Copied(_)
            | PageEvent::Cursor(_)
            | PageEvent::LoginFilled(_)
            | PageEvent::LoginSubmitted(_)
            | PageEvent::PasskeyRequested(_) => {}
        }
    }
}
