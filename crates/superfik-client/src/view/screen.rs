//! One whole frame of the client for a shell that paints everything at
//! once: the canvas, then the sidebar, then the overlays (viewer, review,
//! settings, memory explorer, sheets, Get Started, sign-in), as the window
//! draws them. The snapshot and the web app paint through it.

use std::collections::HashMap;
use std::path::PathBuf;

use superfik_browser::import::BrowserKind;
use superfik_core::BrowserId;
use superfik_render::{Color, ImageId, Point, Rect, Scene, TextSystem};
use superfik_ui::Painter;
use superfik_ui::components::{GraphView, PageImage};
use superfik_ui::painter::LAYERS;

use super::motions::Motions;
use super::sidebar::{self, SidebarView};
use super::target::{HitMap, Target};
use super::viewer::ViewerView;
use super::{Frame, LoadedImage, plan};
use crate::input::scrolls::InputScroll;
use crate::state::Session;

#[derive(Debug, Clone, Copy)]
pub struct Assets<'a> {
    pub images: &'a HashMap<PathBuf, LoadedImage>,
    pub full_images: &'a HashMap<PathBuf, LoadedImage>,
    pub pages: &'a HashMap<BrowserId, PageImage>,
    pub browser_icons: &'a HashMap<BrowserKind, ImageId>,
}

#[derive(Debug, Clone, Copy)]
pub struct Moment {
    pub viewport: (f32, f32),
    pub scale: f32,
    pub pan: (f32, f32),
    pub hovered: Option<Target>,
    pub caret_visible: bool,
    pub now: f64,
    pub graph: GraphView,
    pub input_scroll: InputScroll,
}

#[derive(Debug)]
pub struct Painted {
    pub canvas: Scene,
    pub sidebar: Scene,
    pub overlay: Scene,
    pub hits: HitMap,
    pub sidebar_width: f32,
}

pub fn paint(
    session: &Session,
    assets: Assets<'_>,
    moment: Moment,
    sidebar_width: f32,
    motions: &mut Motions,
    text: &mut TextSystem,
) -> Painted {
    let Moment { viewport, scale, pan, hovered, caret_visible, now, graph, input_scroll } = moment;
    let palette = super::palette(session);
    let layout = plan::plan(session, sidebar::canvas_area(viewport, sidebar_width));
    motions.sync_nodes(&layout.slots, now);
    motions.sync_popover(session.popover, now);
    let shift = |rect: Rect| rect.translate(pan.0, pan.1);

    let mut canvas = Scene::new(palette.canvas, LAYERS);
    let mut painter = Painter::new(&mut canvas, text, scale, palette);
    painter.set_offset(Point::new(pan.0, pan.1));
    let frame = Frame {
        session,
        layout: &layout,
        images: assets.images,
        pages: assets.pages,
        overlaid: false,
        hovered,
        caret_visible,
        input_scroll: (input_scroll.offset, input_scroll.follows_caret),
        sidebar_edge: sidebar_width - pan.0,
        now,
    };
    let mut hits = frame.paint(&mut painter, motions);

    let mut sidebar_scene = Scene::new(Color::TRANSPARENT, LAYERS);
    let mut painter = Painter::new(&mut sidebar_scene, text, scale, palette);
    let mut width = sidebar_width;
    if !session.review_open() {
        let view = SidebarView { session, hovered, scroll: 0.0, caret_visible, now, shortcuts: false };
        width = view.paint(&mut painter, viewport, motions, &mut hits).width;
    }

    let mut overlay = Scene::new(Color::TRANSPARENT, LAYERS);
    let mut painter = Painter::new(&mut overlay, text, scale, palette);
    let tile = hits.viewed_tile.map(shift);
    let viewer = ViewerView { session, images: assets.images, full: assets.full_images, hovered, viewport, now };
    viewer.paint(&mut painter, motions, tile, &mut hits);
    let whole = Rect::new(0.0, 0.0, viewport.0, viewport.1);
    super::review::ReviewView { session, hovered, now, area: whole, caret_visible }.paint(
        &mut painter,
        motions,
        &mut hits,
    );
    super::routines::RoutinesScreenView { session, hovered, viewport, now, caret_visible }
        .paint(&mut painter, &mut hits);
    let settings = super::settings_screen::SettingsScreenView {
        session,
        hovered,
        viewport,
        browser_icons: assets.browser_icons,
        now,
        caret_visible,
    };
    settings.paint(&mut painter, &mut hits);
    let explorer = super::memory::ExplorerView {
        props: plan::memory_props(session, true, now),
        strings: session.strings(),
        hovered,
        area: sidebar::canvas_area(viewport, width),
        view: graph,
        shareable: session.may_share_memory(),
    };
    let node = hits.memory_node.map(shift);
    explorer.paint(&mut painter, motions, node, &mut hits);
    let sheet = super::pr_sheet::PullRequestSheetView { session, hovered, viewport, caret_visible, now };
    let from = hits.repository_node.map(|rect| super::pr_sheet::SheetAnchor::from_canvas(rect, shift));
    sheet.paint(&mut painter, motions, from, &mut hits);
    let card = hits.open_agent_node.map(shift);
    super::agent_sheet::AgentSheetView { session, hovered, viewport, now }.paint(
        &mut painter,
        motions,
        card,
        &mut hits,
    );
    let _ =
        super::get_started::GetStartedView { session, hovered, viewport, now }.paint(&mut painter, motions, &mut hits);
    super::lock::LockView { session, hovered, viewport, now }.paint(&mut painter, &mut hits);
    super::update::UpdateView { session, hovered, viewport, now }.paint(&mut painter, &mut hits);

    Painted { canvas, sidebar: sidebar_scene, overlay, hits, sidebar_width: width }
}
