//! Presentation: turns the session plus animation state into a scene and
//! a hit map. Nothing here mutates the session.

pub mod agent_sheet;
pub mod ask;
pub mod browser_overlay;
pub mod camera;
pub mod composer;
pub mod focus;
pub mod get_started;
pub mod intro_score;
pub mod layout;
pub mod lock;
pub mod memory;
pub mod motions;
pub mod nodes;
pub mod notice;
pub mod plan;
pub mod popovers;
pub mod pr_sheet;
pub mod review;
pub mod routines;
pub mod routing;
pub mod screen;
pub mod settings;
pub mod settings_screen;
pub mod sidebar;
pub mod spacing;
pub mod steps;
pub mod target;
pub mod tooltip;
pub mod transcript;
pub mod update;
pub mod viewer;
mod visual_hits;

use std::collections::HashMap;
use std::path::PathBuf;

use superfik_render::{ImageId, Point, Rect, StepPath};
use superfik_ui::Painter;
use superfik_ui::components::{Connector, Flow, Tile};
use superfik_ui::painter::Plane;
use superfik_ui::theme::colors::{self, Palette};

use self::ask::AskView;
use self::composer::ComposerView;
use self::layout::{CanvasLayout, Column, NodeKey};
use self::motions::{Entrance, Motions, NodeMotion, WordsScope};
use self::nodes::NodeCtx;
use self::popovers::PopoverView;
use self::routing::Wire;
use self::target::{HitMap, Target};
use self::transcript::TranscriptProps;
use crate::state::{Appearance, Carousel, ContextNode, Session};
use superfik_core::AgentStatus;

pub fn palette(session: &Session) -> &'static Palette {
    match session.appearance() {
        Appearance::Light => &colors::LIGHT,
        Appearance::Dark => &colors::DARK,
    }
}

const FLY_INSET: (f32, f32) = (4.0, 42.0);
const TRANSCRIPT_CLEARANCE: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadedImage {
    pub id: ImageId,
    pub aspect: f32,
}

pub struct Frame<'a> {
    pub session: &'a Session,
    pub layout: &'a CanvasLayout,
    pub images: &'a HashMap<PathBuf, LoadedImage>,
    pub pages: &'a HashMap<superfik_core::BrowserId, superfik_ui::components::PageImage>,
    pub overlaid: bool,
    pub hovered: Option<Target>,
    pub caret_visible: bool,
    pub input_scroll: (f32, bool),
    pub sidebar_edge: f32,
    pub now: f64,
}

impl Frame<'_> {
    pub fn paint(&self, p: &mut Painter, motions: &mut Motions) -> HitMap {
        let mut hits = HitMap::default();
        let s = self.session;
        let c = s.conversation();
        let emote = composer::emote_state(c, s.notice.is_some(), self.now);
        motions.sync_emote(emote, self.now);
        let ask = s.pending_ask().map(|ask| AskView {
            session: s,
            ask,
            composer: self.layout.composer,
            hovered: self.hovered,
            emote_from: motions.emote_from(self.now),
            now: self.now,
        });
        let lift = ask.as_ref().map_or(0.0, |ask| ask.extra_height(p).max(0.0));
        let props = TranscriptProps {
            turns: &c.turns,
            live_since: c.run.as_ref().map(|r| r.started_at),
            images: self.images,
            x: self.layout.transcript_x,
            bottom: self.layout.transcript_bottom - lift,
            fly_from: Some(Point::new(self.layout.composer.x + FLY_INSET.0, self.layout.composer.y + FLY_INSET.1)),
            words: WordsScope { conversation: c.id, agent: None },
            now: self.now,
            root: s.root.as_deref(),
            steps_open: &(0..c.turns.len()).map(|i| s.steps_opened_at(i)).collect::<Vec<_>>(),
            strings: s.strings(),
            retry: true,
            hovered: self.hovered,
            utc_offset: s.clock.utc_offset_secs,
            open_outputs: &c.open_outputs,
            comments: s.comments_on(c.id),
            sends: true,
        };
        motions.sync_transcript_height(transcript::height(p, &props));
        let shift = motions.transcript_shift();
        let mut top = self.layout.composer;
        top.y -= lift;
        let clip = transcript_clip(top);
        let beside = sidebar::beside(self.layout.composer, self.sidebar_edge);
        let mut clickable = Vec::new();
        let status_lines = p.on_plane(Plane::Backdrop, |p| {
            p.faded(beside, |p| p.clipped(clip, |p| transcript::paint(p, &props, motions, shift, &mut clickable)))
        });
        for (index, rect) in status_lines.into_iter().enumerate() {
            if c.turns[index].response.steps() > 0 && rect.bottom() <= clip.bottom() {
                hits.add(rect, Target::Steps(index));
            }
        }
        for (target, rect) in clickable {
            if rect.bottom() <= clip.bottom() {
                hits.add(rect, target);
            }
        }

        let focused = plan::focused_nodes(s);
        for key in [NodeKey::Skills, NodeKey::Connections, NodeKey::Commands] {
            motions.sync_variant(key, u8::from(focused.contains(&key)), self.now);
        }
        motions.sync_focus(&focused, self.now);
        for key in [NodeKey::Skills, NodeKey::Connections, NodeKey::Commands] {
            let carousel = plan::carousel_of(key);
            let page = if focused.contains(&key) { s.pick_page(carousel) } else { s.page(carousel) };
            motions.sync_page(key, page, self.now);
        }
        self.paint_nodes(p, motions, &mut hits);
        let permission_chip = p.faded(beside, |p| match &ask {
            Some(ask) => {
                ask.paint(p, &mut hits);
                None
            }
            None => Some(self.paint_composer(p, &mut hits).permission),
        });
        PopoverView {
            session: s,
            motions,
            composer: self.layout.composer,
            permission_chip,
            hovered: self.hovered,
            now: self.now,
        }
        .paint(p, &mut hits);
        notice::paint(p, s, self.layout.composer, self.hovered, &mut hits);
        hits
    }

    fn paint_composer(&self, p: &mut Painter, hits: &mut HitMap) -> superfik_ui::components::ComposerHits {
        ComposerView {
            session: self.session,
            origin: Point::new(self.layout.composer.x, self.layout.composer.y),
            hovered: self.hovered,
            caret_visible: self.caret_visible && self.session.notes.editing.is_none(),
            scroll: self.input_scroll.0,
            follow_caret: self.input_scroll.1,
            now: self.now,
        }
        .paint(p, hits)
    }

    fn paint_nodes(&self, p: &mut Painter, motions: &Motions, hits: &mut HitMap) {
        let mut nodes: Vec<(NodeKey, &NodeMotion)> = motions.nodes().map(|(k, m)| (*k, m)).collect();
        nodes.sort_by_key(|(k, _)| order(*k));
        let wires: Vec<Wire> =
            nodes.iter().map(|(key, m)| Wire { key: *key, column: m.column, rect: m.rect() }).collect();
        let mut paths: HashMap<NodeKey, StepPath> = routing::routes(self.layout.composer, &wires).into_iter().collect();
        for (key, motion) in nodes {
            let opacity = motions.focus_opacity(key, self.now) * sidebar::beside(motion.rect(), self.sidebar_edge);
            if opacity <= 0.0 {
                paths.remove(&key);
                continue;
            }
            p.faded(opacity, |p| self.paint_node_with_wire(p, key, motion, paths.remove(&key), hits));
        }
    }

    fn paint_node_with_wire(
        &self,
        p: &mut Painter,
        key: NodeKey,
        motion: &NodeMotion,
        path: Option<StepPath>,
        hits: &mut HitMap,
    ) {
        let entrance = Entrance::at(motion.presence.progress(self.now));
        let full = motion.rect();
        let busy = self.is_busy(key);
        let flow = busy.then_some(Flow { time: self.now as f32, intensity: 1.0 });
        if let Some(path) = path {
            Connector { path, reach: entrance.connector, flow }.paint(p);
        }
        let rect = grow(full, motion.column, entrance.surface);
        if entrance.surface <= 0.0 {
            return;
        }
        let entrance = Entrance { content: entrance.content.min(motion.swap_progress(self.now)), ..entrance };
        if entrance.content <= 0.0 || !motion.presence.is_visible() {
            p.surface_rect(
                rect,
                superfik_ui::theme::metrics::NODE_RADIUS.min(rect.h * 0.5),
                superfik_ui::theme::metrics::GOO,
            );
            return;
        }
        hits.add(rect, Target::Node(key));
        let mut ctx = NodeCtx {
            rect,
            content: entrance.content,
            busy,
            time: self.now as f32,
            hovered: self.hovered,
            page_slide: motion.page_slide(self.now),
            strings: self.session.strings(),
            hits,
        };
        self.paint_node(p, key, &mut ctx);
    }

    fn is_busy(&self, key: NodeKey) -> bool {
        let s = self.session;
        let context = match key {
            NodeKey::Repository | NodeKey::PullRequest => ContextNode::Repository,
            NodeKey::Skills => ContextNode::Skills,
            NodeKey::Connections => ContextNode::Connections,
            NodeKey::Commands | NodeKey::Notes => return false,
            NodeKey::References => ContextNode::References,
            NodeKey::Research => ContextNode::Research,
            NodeKey::Context => return s.conversation().upkeep.is_some(),
            NodeKey::Browser => return s.browser().is_some_and(|b| b.loading),
            NodeKey::Memory => {
                return self.now - s.memory.learned_at < superfik_ui::theme::motion::MEMORY_LEARNED;
            }
            NodeKey::Agent(i) => {
                let c = s.conversation();
                return c.is_running()
                    && c.agents.get(i).is_some_and(|a| matches!(a.status, AgentStatus::Running { .. }));
            }
        };
        s.is_node_active(context, self.now)
    }

    fn paint_node(&self, p: &mut Painter, key: NodeKey, ctx: &mut NodeCtx) {
        let s = self.session;
        let c = s.conversation();
        match key {
            NodeKey::Repository => {
                let activity = plan::repository_activity(s);
                let mut props = plan::repository_props(s);
                props.activity = activity.as_deref();
                nodes::repository::paint(p, ctx, &props);
            }
            NodeKey::PullRequest => {
                if let Some(pr) = &c.pull_request {
                    nodes::pull_request::paint(p, ctx, pr, s.pending_review_comments());
                }
            }
            NodeKey::Skills | NodeKey::Connections | NodeKey::Commands | NodeKey::Notes
                if s.picking().is_some() && (key != NodeKey::Notes || !plan::picking(s, key).is_empty()) =>
            {
                let carousel = plan::carousel_of(key);
                let (page, pages) = (s.pick_page(carousel), s.pick_pages(carousel));
                let cursor = s.pick_cursor.min(s.picks().len().saturating_sub(1));
                nodes::skills::paint_picker(p, ctx, &plan::picking(s, key), cursor, (carousel, page, pages));
            }
            NodeKey::Skills => nodes::skills::paint(p, ctx, &s.selected_skills(), s.page(Carousel::Skills)),
            NodeKey::Connections => nodes::connections::paint(p, ctx, &s.plugin_rows(), s.page(Carousel::Plugins)),
            NodeKey::Commands => nodes::commands::paint(p, ctx, s.page(Carousel::Commands)),
            NodeKey::References => {
                let tiles: Vec<Tile> =
                    c.references.iter().map(|path| nodes::references::tile(path, self.images)).collect();
                nodes::references::paint(p, ctx, &tiles, c.expanded_reference, context_used(s));
            }
            NodeKey::Notes => {
                plan::with_notes_props(s, self.caret_visible, |props| nodes::notes::paint(p, ctx, props));
            }
            NodeKey::Research => nodes::research::paint(p, ctx, &c.research_items),
            NodeKey::Memory => {
                let props = plan::memory_props(s, self.caret_visible, self.now);
                nodes::memory::paint(p, ctx, &props);
            }
            NodeKey::Context => {
                if let Some(props) = plan::context_props(s) {
                    nodes::context::paint(p, ctx, &props);
                }
            }
            NodeKey::Agent(i) => {
                if let Some(agent) = c.agents.get(i) {
                    nodes::agent::paint(p, ctx, agent, c.open_agent == Some(i));
                }
            }
            NodeKey::Browser => {
                if let Some(card) = &c.browser {
                    let page = self.pages.get(&card.id).copied();
                    use crate::state::PopoverKind;
                    let logins_open =
                        matches!(s.popover, Some(PopoverKind::Logins | PopoverKind::SaveLogin | PopoverKind::Passkeys));
                    nodes::browser::paint(p, ctx, card, page, self.caret_visible, self.overlaid, logins_open);
                }
            }
        }
    }
}

fn transcript_clip(composer: Rect) -> Rect {
    let all = Rect::EVERYTHING;
    Rect::new(all.x, all.y, all.w, composer.y - TRANSCRIPT_CLEARANCE - all.y)
}

fn context_used(session: &Session) -> bool {
    !session.conversation().turns.is_empty() && !session.is_gathering()
}

fn order(key: NodeKey) -> usize {
    match key {
        NodeKey::Repository => 0,
        NodeKey::PullRequest => 1,
        NodeKey::Skills => 2,
        NodeKey::Connections => 3,
        NodeKey::Commands => 4,
        NodeKey::References => 5,
        NodeKey::Notes => 6,
        NodeKey::Memory => 7,
        NodeKey::Research => 8,
        NodeKey::Context => 9,
        NodeKey::Browser => 10,
        NodeKey::Agent(i) => 11 + i,
    }
}

fn grow(full: Rect, column: Column, t: f32) -> Rect {
    if t >= 1.0 {
        return full;
    }
    let cy = full.center().y;
    let pill = match column {
        Column::Left | Column::Beside => Rect::new(full.right() - 44.0, cy - 12.0, 44.0, 24.0),
        Column::Right => Rect::new(full.x, cy - 12.0, 44.0, 24.0),
        Column::Below => Rect::new(full.x + 66.0, full.y, 50.0, 24.0),
    };
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    Rect::new(lerp(pill.x, full.x), lerp(pill.y, full.y), lerp(pill.w, full.w), lerp(pill.h, full.h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_reads_as_used_once_the_run_moves_on() {
        let (session, _) = crate::scenarios::by_name("reference").expect("scenario");
        assert!(context_used(&session));
        assert!(!session.is_node_active(crate::state::ContextNode::Repository, 0.0));
    }

    #[test]
    fn conversation_is_clipped_only_at_the_composer() {
        let composer = Rect::new(699.0, 490.0, 440.0, 190.0);
        let clip = transcript_clip(composer);
        assert_eq!(clip.bottom(), composer.y - TRANSCRIPT_CLEARANCE);
        assert!(clip.y < -100_000.0, "open at the top: the camera pans up to older turns");
        assert!(clip.contains(Point::new(composer.x, -50_000.0)));
        assert!(!clip.contains(Point::new(composer.x, composer.y)));
    }

    #[test]
    fn growth_starts_as_pill_near_composer_and_ends_full() {
        let full = Rect::new(100.0, 100.0, 216.0, 150.0);
        assert_eq!(grow(full, Column::Left, 1.0), full);
        let pill = grow(full, Column::Left, 0.0);
        assert_eq!(pill.right(), full.right());
        assert_eq!(pill.h, 24.0);
        let right = grow(full, Column::Right, 0.0);
        assert_eq!(right.x, full.x);
    }
}
