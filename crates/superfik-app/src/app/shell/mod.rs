//! The winit application: owns the window, renderer, session and backend,
//! and drives the dispatch → render loop.

use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_protocol::ClientCommand;
use superfik_render::{Color, Point, Rect, Renderer, Scene, TextSystem, gpu::GpuContext};
use superfik_ui::Painter;
use superfik_ui::painter::LAYERS;
use superfik_ui::theme::{metrics, motion};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::window::{Window, WindowId};

use super::effects::EffectRunner;
use super::images::ImageCache;
use super::input::map_key;
use super::pages::Pages;
use super::pointer::Pointer;
use super::titlebar::DoubleClickAction;
use crate::state::{Action, BrowserFocus, EditKey, Effect, Session, Theme, WallClock};
use crate::view::camera::Camera;
use crate::view::intro_score::IntroScore;
use crate::view::layout::{CanvasLayout, NodeKey};
use crate::view::motions::Motions;
use crate::view::pr_sheet::SheetAnchor;
use crate::view::sidebar::{self, SidebarView};
use crate::view::spacing::NodeSpacing;
use crate::view::target::{HitMap, Target};
use crate::view::tooltip::{ElidedLabels, TooltipHover};
use crate::view::viewer::ViewerView;
use crate::view::{Frame, plan};
use superfik_haptics::Haptics;

mod canvas;
mod gestures;
mod pacer;
mod page_input;
#[cfg(target_os = "linux")]
mod quit_signals;
mod talk;
mod user_settings;
mod wheel;

#[derive(Debug, Clone, Copy)]
pub enum Wake {
    Backend,
    OpenSettings,
    #[cfg(target_os = "linux")]
    Quit,
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(snapshot) = super::snapshot::parse(&args) {
        return super::snapshot::run(&snapshot);
    }
    let cache_root = superfik_backend::local_data_dir().unwrap_or_else(std::env::temp_dir).join("browser");
    let mut pages = Pages::new(superfik_browser::system(cache_root.clone()), cache_root);
    pages.warm_up();
    let mut event_loop = EventLoop::<Wake>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    #[cfg(target_os = "linux")]
    quit_signals::install(proxy.clone());
    let mut app = App::new(proxy, pages)?;
    loop {
        let timeout = app.pages.is_active().then_some(PAGE_PUMP);
        if let PumpStatus::Exit(_) = event_loop.pump_app_events(timeout, &mut app) {
            break;
        }
        app.pump_pages();
    }
    Ok(())
}

const REVEAL_SETTLE: f64 = 1.0;

const PAGE_PUMP: Duration = Duration::from_millis(4);

struct Gpu {
    window: Arc<Window>,
    renderer: Renderer,
}

struct App {
    gpu: Option<Gpu>,
    clipboard: super::clipboard::Clipboard,
    text: TextSystem,
    session: Session,
    motions: Motions,
    effects: EffectRunner,
    images: ImageCache,
    hits: HitMap,
    attached: Vec<(Option<NodeKey>, Rect)>,
    pointer: Pointer,
    spacing: NodeSpacing,
    base: CanvasLayout,
    camera: Camera,
    haptics: Haptics,
    pages: Pages,
    browser_icons: super::browser_icons::BrowserIcons,
    page_pressed: Option<winit::event::MouseButton>,
    over_page: bool,
    shown_cursor: Option<winit::window::CursorIcon>,
    revealed: Option<(superfik_core::BrowserId, f64)>,
    dock: super::dock::DockEmote,
    proxy: EventLoopProxy<Wake>,
    menu: Option<super::menu::Menu>,
    sidebar_scroll: f32,
    scrolls: superfik_client::input::scrolls::Scrolls,
    page_swipe: super::page_swipe::PageSwipe,
    sidebar: sidebar::SidebarExtent,
    hovered: Option<Target>,
    tooltip: TooltipHover,
    intro_score: IntroScore,
    modifiers: ModifiersState,
    push_to_talk: superfik_client::input::push_to_talk::PushToTalk,
    start: Instant,
    last_frame: f64,
    last_edit: f64,
    focused: bool,
    chrome: Option<Theme>,
    was_live: bool,
    pacer: pacer::Pacer,
}

const BLINK: f64 = 0.53;

impl App {
    fn new(proxy: EventLoopProxy<Wake>, pages: Pages) -> Result<Self, Box<dyn std::error::Error>> {
        let clipboard = pages.clipboard();
        let backend_proxy = proxy.clone();
        let wake: superfik_backend::Wake = Arc::new(move || {
            let _ = backend_proxy.send_event(Wake::Backend);
        });
        let connected = super::connect::backend(wake.clone())?;
        let mut effects = EffectRunner::new(connected.backend, superfik_sound::Sounds::open(Default::default()))
            .clipboard(clipboard.clone());
        let settings = user_settings::at_launch(&mut effects, super::settings::take_legacy);
        let preferences = user_settings::preferences(&settings);
        effects.sound_prefs(preferences.sound);
        let reason = superfik_i18n::Strings::new(preferences.language).t(superfik_i18n::Text::UnlockReason);
        let unlocker = super::unlock::Unlocker::new(superfik_unlock::system(), reason, wake.clone());
        let updater = super::update::UpdateRunner::detect(wake.clone());
        let effects = effects
            .voice(super::voice::device(wake.clone()))
            .joining(connected.device, wake)
            .unlocker(unlocker)
            .updater(updater);
        let mut app = Self {
            gpu: None,
            clipboard,
            text: TextSystem::new(),
            session: Session { platform: super::platform(), ..Session::launching() },
            motions: Motions::default(),
            effects,
            images: ImageCache::default(),
            hits: HitMap::default(),
            pointer: Pointer::new(DoubleClickAction::from_system()).with_titlebar(super::platform().titlebar_height()),
            spacing: NodeSpacing::default(),
            base: plan::plan(&Session::default(), superfik_render::Rect::default()),
            camera: Camera::default(),
            pages,
            browser_icons: super::browser_icons::BrowserIcons::default(),
            page_pressed: None,
            over_page: false,
            shown_cursor: None,
            revealed: None,
            haptics: Haptics::trackpad(
                preferences.haptics,
                superfik_ui::theme::motion::HAPTIC_GAP,
                superfik_ui::theme::motion::HAPTIC_BURST,
            ),
            dock: super::dock::DockEmote::new(
                superfik_dock::system(),
                super::launch::dock_emote_enabled(std::env::var("SUPERFIK_DOCK").ok().as_deref()),
            ),
            proxy,
            menu: None,
            sidebar_scroll: 0.0,
            scrolls: Default::default(),
            page_swipe: super::page_swipe::PageSwipe::default(),
            attached: Vec::new(),
            sidebar: sidebar::SidebarExtent {
                width: superfik_ui::theme::metrics::SIDEBAR_MIN_WIDTH,
                content_height: 0.0,
                selected_y: None,
            },
            hovered: None,
            tooltip: TooltipHover::default(),
            intro_score: IntroScore::default(),
            modifiers: ModifiersState::empty(),
            push_to_talk: Default::default(),
            start: Instant::now(),
            last_frame: 0.0,
            last_edit: 0.0,
            focused: true,
            chrome: None,
            was_live: false,
            pacer: pacer::Pacer::default(),
        };
        app.session.clock = wall_clock();
        app.execute(vec![
            Effect::Command(Box::new(ClientCommand::ListModels)),
            Effect::Command(Box::new(ClientCommand::ListGitHubRepositories)),
            Effect::Command(Box::new(ClientCommand::ListPlugins)),
        ]);
        app.apply_settings(settings.clone());
        app.dispatch(Action::Managed(super::managed::load()));
        if !cfg!(target_os = "windows") {
            app.dispatch(Action::Lock(crate::state::LockAction::Launched));
        }
        app.effects.resume_server();
        let home = std::env::home_dir();
        let app_dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(std::path::Path::to_path_buf));
        let root = super::launch::startup_workspace(
            std::env::args_os().nth(1).map(std::path::PathBuf::from),
            std::env::current_dir().ok(),
            home.as_deref(),
            app_dir.as_deref(),
            settings.last_workspace,
        );
        if let Some(root) = root.filter(|r| r.is_dir()) {
            app.dispatch(Action::OpenWorkspace(root));
        }
        let home = std::env::home_dir();
        let sources = home.map(|h| superfik_browser::import::sources(&h)).unwrap_or_default();
        let kinds: Vec<_> = sources.iter().map(|s| s.kind).collect();
        let icons = superfik_backend::local_data_dir().unwrap_or_else(std::env::temp_dir).join("browser-icons");
        app.browser_icons = super::browser_icons::BrowserIcons::start(kinds.clone(), icons);
        app.dispatch(Action::BrowserSources(kinds));
        if let Ok(address) = std::env::var("SUPERFIK_BROWSER") {
            app.dispatch(Action::OpenBrowser);
            app.dispatch(Action::Edit(EditKey::Insert(address)));
            app.dispatch(Action::Submit);
        }
        Ok(app)
    }

    fn shortcut_modifiers(&self) -> winit::keyboard::ModifiersState {
        super::input::for_primary(self.modifiers, self.session.platform.primary)
    }

    fn now(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn dispatch(&mut self, action: Action) {
        if matches!(action, Action::Edit(_)) {
            self.last_edit = self.now();
            self.scrolls.edited();
        }
        let opening = matches!(action, Action::OpenWorkspace(_));
        let active = self.session.active;
        let phase = self.session.voice.phase;
        let label = action_label(&action);
        let effects = self.session.apply(action, self.now());
        if self.session.voice.phase != phase {
            tracing::info!(from = ?phase, to = ?self.session.voice.phase, action = label, "voice phase");
        }
        self.haptics.set_enabled(self.session.preferences.haptics);
        if opening || self.session.active != active {
            self.switched();
        }
        self.execute(effects);
        self.sync_chrome();
        if let Some(menu) = &mut self.menu {
            menu.sync(self.session.strings().t(superfik_i18n::Text::SettingsMenu));
        }
        self.request_redraw();
    }

    fn sync_chrome(&mut self) {
        let theme = self.session.preferences.theme;
        let Some(gpu) = self.gpu.as_ref().filter(|_| self.chrome != Some(theme)) else { return };
        self.chrome = Some(theme);
        gpu.window.set_theme(super::window::chrome(theme));
        if let Some(os) = gpu.window.theme().filter(|_| theme == Theme::System) {
            self.session.apply(Action::SystemAppearance(super::window::appearance(os)), self.now());
        }
    }

    fn execute(&mut self, effects: Vec<Effect>) {
        let scale = self.gpu.as_ref().map_or(1.0, |g| g.window.scale_factor() as f32);
        let (pages, mut effects): (Vec<Effect>, Vec<Effect>) =
            effects.into_iter().partition(|e| matches!(e, Effect::Browser(_) | Effect::SaveLogin { .. }));
        for effect in pages {
            match effect {
                Effect::Browser(call) => self.pages.send(call, scale),
                Effect::SaveLogin { browser, account } => {
                    effects.extend(self.pages.save_login(browser, account).map(|c| Effect::Command(Box::new(c))));
                }
                _ => {}
            }
        }
        self.pages.follow_logins(self.session.preferences.logins);
        let (cookies, effects): (Vec<Effect>, Vec<Effect>) =
            effects.into_iter().partition(|e| matches!(e, Effect::ImportLogins(_) | Effect::ClearCookies));
        for effect in cookies {
            match effect {
                Effect::ImportLogins(kind) => self.pages.import_logins(kind),
                _ => self.pages.clear_cookies(),
            }
        }
        if effects.iter().any(Effect::is_dialog) {
            self.pages.suspend();
        }
        for action in self.effects.run(effects, &self.session) {
            self.dispatch(action);
        }
    }

    fn pump_pages(&mut self) {
        let Some(gpu) = &mut self.gpu else { return };
        let (actions, painted) = self.pages.pump(&mut gpu.renderer);
        for action in actions {
            self.dispatch(action);
        }
        self.show_cursor();
        if painted {
            self.request_redraw();
        }
    }

    fn request_redraw(&self) {
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    fn drain_backend(&mut self) {
        if let Some(joined) = self.effects.poll_join() {
            self.dispatch(joined);
        }
        for unlock in self.effects.poll_unlock() {
            self.dispatch(unlock);
        }
        for update in self.effects.poll_update() {
            self.dispatch(update);
        }
        for voice in self.effects.poll_voice() {
            self.dispatch(voice);
        }
        while let Some(event) = self.effects.next_event() {
            if let superfik_protocol::ServerEvent::Settings { settings } = event {
                self.apply_settings(super::settings::Settings::from_value(settings.unwrap_or_default()));
                continue;
            }
            match super::login_fill::route(event) {
                super::login_fill::Routed::Page(fill) => {
                    let scale = self.gpu.as_ref().map_or(1.0, |gpu| gpu.window.scale_factor() as f32);
                    self.pages.send(fill, scale);
                }
                super::login_fill::Routed::Session(event) => self.dispatch(Action::Server(event)),
            }
        }
    }

    fn render(&mut self) {
        let now = self.start.elapsed().as_secs_f64();
        let dt = (now - self.last_frame).clamp(0.0, 0.1) as f32;
        let Some(gpu) = &mut self.gpu else { return };
        let previous = std::mem::replace(&mut self.last_frame, now);
        self.pacer.observe(now - previous);
        let scale = gpu.window.scale_factor() as f32;
        let size = gpu.window.inner_size();
        let viewport = (size.width as f32 / scale, size.height as f32 / scale);

        self.motions.step(dt);
        self.camera.step(dt);
        let mut layout = plan::plan(&self.session, sidebar::canvas_area(viewport, self.sidebar.width));
        self.base.clone_from(&layout);
        let column = self.camera.rect_to_canvas(Rect::new(0.0, 0.0, self.sidebar.width, viewport.1));
        if plan::focused_nodes(&self.session).is_empty() {
            let (extras, avoid) = gestures::collision_sets(&self.attached, self.base.composer);
            self.spacing.place_new(&self.base.slots, &extras, &[column], &avoid);
            let composer = self.base.composer.right() + metrics::NODE_CLEARANCE;
            let right = self.spacing.right_edge(&self.base.slots, &[]).map_or(composer, |r| r.max(composer));
            self.camera.fence(right, self.sidebar.width + metrics::SIDEBAR_KEEP);
            self.camera.make_room(self.spacing.panel_edge(&self.base.slots, &extras), self.sidebar.width);
            self.spacing.apply(&mut layout.slots);
        }
        self.motions.sync_nodes(&layout.slots, now);
        let card = self.session.browser().map(|card| card.id);
        if card != self.revealed.map(|(id, _)| id) {
            self.revealed = card.map(|id| (id, now));
        }
        let settling = self.revealed.is_some_and(|(_, since)| now - since < REVEAL_SETTLE);
        if settling
            && !self.camera.is_animating()
            && let Some(slot) = layout.slots.iter().find(|s| s.key == NodeKey::Browser)
        {
            let area = sidebar::canvas_area(viewport, self.sidebar.width);
            let others = layout.slots.iter().filter(|s| s.key != NodeKey::Browser);
            let keep = others.fold(layout.composer, |all, s| all.union(&s.rect));
            self.camera.reveal(slot.rect, keep, area, superfik_ui::theme::metrics::BROWSER_REVEAL_MARGIN);
        }
        self.motions.sync_popover(self.session.popover, now);
        let emote = crate::view::composer::emote_state(self.session.conversation(), self.session.notice.is_some(), now);
        self.motions.sync_emote(emote, now);
        self.dock.sync(emote, self.motions.emote_from(now), now);
        let conversation = self.session.conversation();
        let references =
            conversation.references.iter().chain(conversation.turns.iter().flat_map(|t| t.user.images.iter()));
        self.images.ensure(&mut gpu.renderer, references.map(|p| (p.as_path(), conversation.preview(p))));

        let palette = crate::view::palette(&self.session);
        let mut scene = Scene::new(palette.canvas, LAYERS);
        let zoomed = scale * self.camera.zoom;
        let mut painter = Painter::new(&mut scene, &mut self.text, zoomed, palette);
        painter.set_offset(self.camera.offset());
        let caret_visible = ((now - self.last_edit) % (BLINK * 2.0)) < BLINK;
        let frame = Frame {
            session: &self.session,
            layout: &layout,
            images: self.images.ids(),
            pages: self.pages.ids(),
            overlaid: self.session.browser().is_some_and(|card| card.expanded)
                || self.motions.expanded_browser(now).is_some(),
            hovered: self.hovered,
            caret_visible,
            input_scroll: (self.scrolls.input.offset, self.scrolls.input.follows_caret),
            sidebar_edge: self.camera.to_canvas(Point::new(self.sidebar.width, 0.0)).x,
            now,
        };
        self.hits = frame.paint(&mut painter, &mut self.motions);
        if let Some((id, page)) = self.hits.browser_page {
            self.pages.fit(id, page, scale);
        }
        let mut elided = ElidedLabels::default();
        let pan = self.camera.pan;
        let canvas_labels = painter.take_elided().into_iter().map(|mut label| {
            label.rect = self.camera.to_screen(label.rect.translate(-pan.x, -pan.y));
            label
        });
        elided.add(canvas_labels.collect(), 0);
        if let Some((scroll, _)) = self.hits.input_scroll {
            self.scrolls.input.offset = scroll;
        }
        self.attached = self
            .hits
            .attached
            .iter()
            .filter_map(|(key, panel)| {
                let from = match key {
                    Some(key) => self.motions.node(*key)?.rect(),
                    None => layout.composer,
                };
                Some((*key, panel.translate(-from.x, -from.y)))
            })
            .collect();
        let shortcuts = super::input::for_primary(self.modifiers, self.session.platform.primary).super_key();
        let mut hud = Scene::new(Color::TRANSPARENT, LAYERS);
        let mut painter = Painter::new(&mut hud, &mut self.text, scale, palette);
        let sidebar = SidebarView {
            session: &self.session,
            hovered: self.hovered,
            scroll: self.sidebar_scroll,
            caret_visible,
            now,
            shortcuts,
        };
        if !self.session.review_open() {
            self.sidebar = sidebar.paint(&mut painter, viewport, &mut self.motions, &mut self.hits);
        }
        let canvas = sidebar::canvas_area(viewport, self.sidebar.width);
        if !self.session.review_open() && sidebar::off_screen(self.camera.to_screen(self.base.composer), canvas) {
            let text = self.session.strings().t(superfik_i18n::Text::Recentre);
            let button = superfik_ui::components::Button {
                label: &text,
                primary: false,
                hovered: self.hovered == Some(Target::RecentreComposer),
                enabled: true,
            };
            let width = button.width(&mut painter);
            let y = canvas.bottom()
                - superfik_ui::theme::metrics::RECENTRE_MARGIN
                - superfik_ui::theme::metrics::CHIP_HEIGHT;
            let rect =
                Rect::new(canvas.x + (canvas.w - width) * 0.5, y, width, superfik_ui::theme::metrics::CHIP_HEIGHT);
            button.paint(&mut painter, rect);
            self.hits.add_screen(rect, Target::RecentreComposer);
        }
        elided.add(painter.take_elided(), self.hits.screen_len());
        if let Some(y) = self.sidebar.selected_y {
            let height = (viewport.1 - superfik_ui::theme::metrics::SETTINGS_FOOTER).max(0.0);
            let scroll = sidebar::reveal(self.sidebar_scroll, y, height, self.sidebar.content_height - height);
            if scroll != self.sidebar_scroll {
                self.sidebar_scroll = scroll;
                gpu.window.request_redraw();
            }
        }
        let mut overlay = Scene::new(Color::TRANSPARENT, LAYERS);
        let mut painter = Painter::new(&mut overlay, &mut self.text, scale, palette);
        let tile = self.hits.viewed_tile.map(|rect| self.camera.to_screen(rect));
        let conversation = self.session.conversation();
        if let Some(path) = conversation.expanded_reference.and_then(|i| conversation.references.get(i)) {
            self.images.ensure_full(&mut gpu.renderer, path, conversation.preview(path));
        }
        let viewer = ViewerView {
            session: &self.session,
            images: self.images.ids(),
            full: self.images.full_ids(),
            hovered: self.hovered,
            viewport,
            now,
        };
        viewer.paint(&mut painter, &mut self.motions, tile, &mut self.hits);
        let expanded = crate::view::browser_overlay::ExpandedBrowserView {
            session: &self.session,
            pages: self.pages.ids(),
            hovered: self.hovered,
            viewport,
            caret_visible,
            now,
        };
        let card = self.hits.browser_card.map(|rect| self.camera.to_screen(rect));
        expanded.paint(&mut painter, &mut self.motions, card, &mut self.hits);
        if let Some((id, page)) = self.hits.expanded_page {
            self.pages.fit(id, page, scale);
        }
        let review = crate::view::review::ReviewView {
            session: &self.session,
            hovered: self.hovered,
            now,
            area: Rect::new(0.0, 0.0, viewport.0, viewport.1),
            caret_visible,
        };
        review.paint(&mut painter, &mut self.motions, &mut self.hits);
        self.browser_icons.poll(&mut gpu.renderer);
        let routines = crate::view::routines::RoutinesScreenView {
            session: &self.session,
            hovered: self.hovered,
            viewport,
            now,
            caret_visible,
        };
        routines.paint(&mut painter, &mut self.hits);
        let settings = crate::view::settings_screen::SettingsScreenView {
            session: &self.session,
            hovered: self.hovered,
            viewport,
            browser_icons: self.browser_icons.ids(),
            now,
            caret_visible,
        };
        settings.paint(&mut painter, &mut self.hits);
        let explorer = crate::view::memory::ExplorerView {
            props: crate::view::plan::memory_props(&self.session, caret_visible, now),
            strings: self.session.strings(),
            hovered: self.hovered,
            area: sidebar::canvas_area(viewport, self.sidebar.width),
            view: self.scrolls.graph,
            shareable: self.session.may_share_memory(),
        };
        if !self.session.memory.open {
            self.scrolls.graph = superfik_ui::components::GraphView::default();
        }
        let node = self.hits.memory_node.map(|rect| self.camera.to_screen(rect));
        explorer.paint(&mut painter, &mut self.motions, node, &mut self.hits);
        let sheet = crate::view::pr_sheet::PullRequestSheetView {
            session: &self.session,
            hovered: self.hovered,
            viewport,
            caret_visible,
            now,
        };
        let repository = self.hits.repository_node;
        let from = repository.map(|rect| SheetAnchor::from_canvas(rect, |r| self.camera.to_screen(r)));
        if let (Some(panel), Some(node)) =
            (sheet.paint(&mut painter, &mut self.motions, from, &mut self.hits), repository)
        {
            let panel = self.camera.rect_to_canvas(panel);
            self.attached.push((Some(NodeKey::Repository), panel.translate(-node.x, -node.y)));
        }
        let agent_sheet =
            crate::view::agent_sheet::AgentSheetView { session: &self.session, hovered: self.hovered, viewport, now };
        let card = self.hits.open_agent_node.map(|rect| self.camera.to_screen(rect));
        agent_sheet.paint(&mut painter, &mut self.motions, card, &mut self.hits);
        let get_started =
            crate::view::get_started::GetStartedView { session: &self.session, hovered: self.hovered, viewport, now };
        let opening = get_started.paint(&mut painter, &mut self.motions, &mut self.hits);
        crate::view::lock::LockView { session: &self.session, hovered: self.hovered, viewport, now }
            .paint(&mut painter, &mut self.hits);
        crate::view::update::UpdateView { session: &self.session, hovered: self.hovered, viewport, now }
            .paint(&mut painter, &mut self.hits);
        let intro_cue = self.intro_score.advance(opening);
        elided.add(painter.take_elided(), self.hits.screen_len());
        self.tooltip.sync(elided.under(self.pointer.position, &self.hits), now);
        self.tooltip.paint(&mut painter, self.pointer.position, viewport, now);
        let painted = self.start.elapsed().as_secs_f64();
        let passes = [(&scene, zoomed), (&hud, scale), (&overlay, scale)];
        if let Err(error) = gpu.renderer.render_passes(&passes, &mut self.text, now as f32) {
            tracing::error!(%error, "render failed");
        }
        let presented = self.start.elapsed().as_secs_f64();
        tracing::trace!(
            frame_ms = ((now - previous) * 1000.0) as u32,
            paint_ms = ((painted - now) * 1000.0) as u32,
            gpu_ms = ((presented - painted) * 1000.0) as u32,
            "frame"
        );
        if let Some(cue) = intro_cue {
            self.execute(vec![Effect::Sound(cue)]);
        }
    }

    fn install_menu(&mut self) {
        let title = self.session.strings().t(superfik_i18n::Text::SettingsMenu);
        let proxy = self.proxy.clone();
        let menu = superfik_menu::system(
            &title,
            Box::new(move || {
                let _ = proxy.send_event(Wake::OpenSettings);
            }),
        );
        self.menu = Some(super::menu::Menu::new(menu, title));
    }

    fn schedule(&mut self, event_loop: &ActiveEventLoop) {
        let now = self.now();
        let live = self.session.any_running()
            || self.motions.is_animating(now)
            || self.camera.is_animating()
            || self.session.logins.is_animating(now)
            || self.session.browser().is_some_and(|card| card.loading)
            || self.session.model_changed.as_ref().is_some_and(|(_, at)| now - at < motion::MORPH as f64 + 0.1);
        let settle = self.was_live && !live;
        self.was_live = live;
        if live || settle || self.focused {
            if let Some(at) = self.pacer.next_frame(self.last_frame).filter(|&at| at > now) {
                let at = self.start + Duration::from_secs_f64(at);
                event_loop.set_control_flow(ControlFlow::WaitUntil(at));
                return;
            }
            self.request_redraw();
            event_loop.set_control_flow(ControlFlow::Poll);
            return;
        }
        let blink = BLINK - ((now - self.last_edit) % BLINK);
        let mut wait = motion::IDLE_FRAME.min(blink).max(0.004);
        if self.pages.is_active() {
            wait = wait.min(PAGE_PUMP.as_secs_f64());
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_secs_f64(wait)));
    }
}

impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let window = match event_loop.create_window(super::window::attributes()) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                tracing::error!(%error, "could not create window");
                event_loop.exit();
                return;
            }
        };
        #[cfg(target_os = "windows")]
        super::window::fit_monitor(&window);
        self.clipboard.attach(window.clone());
        window.set_ime_allowed(true);
        #[cfg(target_os = "windows")]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = window.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
            {
                self.effects.unlock_parent_window(handle.hwnd.get());
            }
            self.dispatch(Action::Lock(crate::state::LockAction::Launched));
        }
        self.pacer.set_refresh(window.current_monitor().and_then(|monitor| monitor.refresh_rate_millihertz()));
        self.install_menu();
        let size = window.inner_size();
        let open: superfik_render::gpu::OpenDevice = superfik_browser::open_device;
        match GpuContext::opening(window.clone(), size.width, size.height, Some(open)) {
            Ok(context) => {
                self.gpu = Some(Gpu { window, renderer: Renderer::new(context) });
                self.sync_chrome();
            }
            Err(error) => {
                tracing::error!(%error, "could not initialise the GPU");
                event_loop.exit();
            }
        }
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            self.request_redraw();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Wake) {
        match event {
            Wake::Backend => self.drain_backend(),
            Wake::OpenSettings => self.dispatch(Action::OpenSettings),
            #[cfg(target_os = "linux")]
            Wake::Quit => event_loop.exit(),
        }
        if self.effects.update_quitting() {
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.renderer.resize(size.width, size.height);
                }
                self.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.render();
                self.update_hover();
            }
            WindowEvent::ThemeChanged(os) if self.session.preferences.theme == Theme::System => {
                self.dispatch(Action::SystemAppearance(super::window::appearance(os)));
            }
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                if !focused {
                    self.talk_blur();
                }
                self.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let before = self.shortcut_modifiers().super_key();
                self.modifiers = modifiers.state();
                if self.shortcut_modifiers().super_key() != before {
                    self.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.talk_key(&event);
                let command = super::input::for_platform(
                    map_key(&event.logical_key, event.text.as_deref(), self.shortcut_modifiers()),
                    self.session.platform,
                );
                if self.session.browser_focus() == Some(BrowserFocus::Page)
                    && !super::controller::app_owns_key(&command)
                {
                    self.page_key(&event);
                } else if event.state == ElementState::Pressed {
                    self.key(command);
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) if self.session.browser_focus() == Some(BrowserFocus::Page) => {
                self.page_text(text);
            }
            WindowEvent::Ime(Ime::Commit(text)) => self.dispatch(Action::Edit(EditKey::Insert(text))),
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.gpu.as_ref().map_or(1.0, |g| g.window.scale_factor());
                self.pointer.position = Point::new((position.x / scale) as f32, (position.y / scale) as f32);
                self.pointer_moved();
                self.page_pointer_moved();
                self.update_hover();
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                self.talk_other();
                self.press();
                self.page_press(MouseButton::Left);
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } => self.page_press(button),
            WindowEvent::MouseInput { state: ElementState::Released, button, .. } => {
                if button == MouseButton::Left {
                    self.pointer.sliding_effort = false;
                    self.pointer.selecting_text = false;
                }
                self.page_release(button);
            }
            WindowEvent::MouseWheel { delta, phase, .. } => self.wheel(delta, phase),
            WindowEvent::PinchGesture { .. }
                if self.session.settings.open || self.session.routines.open || self.hits.expanded_page.is_some() => {}
            WindowEvent::PinchGesture { delta, .. } => {
                if let Some(graph) = self.memory_graph_under_pointer() {
                    self.scrolls.graph.zoom_at(1.0 + delta as f32, self.pointer.position, graph);
                } else {
                    self.camera.zoom_at(1.0 + delta as f32, self.pointer.position);
                }
                self.update_hover();
                self.request_redraw();
            }
            WindowEvent::DroppedFile(path) => {
                let action =
                    if path.is_dir() { Action::OpenWorkspace(path) } else { Action::AddReferences(vec![path]) };
                self.dispatch(action);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.talk_tick();
        self.schedule(event_loop);
    }
}

fn action_label(action: &Action) -> String {
    let full = format!("{action:?}");
    let end = full.find(|c: char| !(c.is_alphanumeric() || c == '(' || c == '_')).unwrap_or(full.len());
    full[..end].to_owned()
}

fn wall_clock() -> WallClock {
    let now = chrono::Local::now();
    WallClock { unix_at_zero: now.timestamp(), utc_offset_secs: now.offset().local_minus_utc() }
}
