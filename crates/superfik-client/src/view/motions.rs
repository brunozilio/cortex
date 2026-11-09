//! Animation state that outlives a frame: node presence and position
//! springs, popover and viewer presence, word reveal times and the
//! conversation's growth glide.

use std::collections::HashMap;

use superfik_core::ConversationId;
use superfik_motion::{Easing, Presence, Spring, SpringConfig, remap};
use superfik_render::Rect;
use superfik_ui::emote::EmoteState;
use superfik_ui::theme::motion;

use super::layout::{Column, NodeKey, Slot};
use crate::state::PopoverKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WordsScope {
    pub conversation: ConversationId,
    pub agent: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct NodeMotion {
    pub presence: Presence,
    pub column: Column,
    variant: u8,
    swapped_at: f64,
    page: usize,
    paged_at: f64,
    page_direction: f32,
    x: Spring,
    y: Spring,
    h: Spring,
    w: f32,
}

impl NodeMotion {
    fn new(slot: &Slot, now: f64) -> Self {
        let mut presence = Presence::new(false, motion::NODE_ENTER, motion::NODE_EXIT);
        presence.set(true, now);
        let rect = slot.rect;
        Self {
            presence,
            column: slot.column,
            variant: 0,
            swapped_at: f64::NEG_INFINITY,
            page: 0,
            paged_at: f64::NEG_INFINITY,
            page_direction: 0.0,
            x: Spring::new(rect.x, SpringConfig::LAYOUT),
            y: Spring::new(rect.y, SpringConfig::LAYOUT),
            h: Spring::new(rect.h, SpringConfig::LAYOUT),
            w: rect.w,
        }
    }

    pub fn swap_progress(&self, now: f64) -> f32 {
        Easing::OUT.apply(((now - self.swapped_at) as f32 / motion::CONTENT_SWAP).clamp(0.0, 1.0))
    }

    pub fn page_slide(&self, now: f64) -> f32 {
        let t = Easing::OUT.apply(((now - self.paged_at) as f32 / motion::PAGE_SLIDE).clamp(0.0, 1.0));
        self.page_direction * (1.0 - t)
    }

    pub fn rect(&self) -> Rect {
        Rect::new(self.x.value, self.y.value, self.w, self.h.value)
    }

    fn settled(&self) -> bool {
        self.x.is_settled() && self.y.is_settled() && self.h.is_settled()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entrance {
    pub connector: f32,
    pub surface: f32,
    pub content: f32,
}

impl Entrance {
    pub fn at(progress: f32) -> Self {
        Self {
            connector: Easing::OUT.apply(remap(progress, 0.0, 0.4)),
            surface: Easing::CubicBezier(0.3, 1.2, 0.4, 1.0).apply(remap(progress, 0.25, 0.8)),
            content: Easing::OUT.apply(remap(progress, 0.6, 1.0)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimelineMotion {
    y: Spring,
    trail_from: f32,
    moved_at: f64,
}

impl TimelineMotion {
    fn trail_faded(&self, now: f64) -> f32 {
        let travelled = ((now - self.moved_at) as f32 - motion::TIMELINE_TRAVEL).max(0.0);
        (travelled / motion::TIMELINE_TRAIL).clamp(0.0, 1.0)
    }

    pub fn marker(&self, now: f64) -> superfik_ui::components::TimelineMarker {
        superfik_ui::components::TimelineMarker {
            y: self.y.value,
            trail_from: self.trail_from,
            trail: 1.0 - Easing::OUT.apply(self.trail_faded(now)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewerMotion {
    pub presence: Presence,
    pub index: usize,
    pub from: Rect,
}

impl ViewerMotion {
    pub fn progress(&self, now: f64) -> f32 {
        motion::ENTER.apply(self.presence.progress(now))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SheetKind {
    PullRequest,
    Agent,
    PublishReview,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExplorerMotion {
    pub presence: Presence,
    pub from: Rect,
}

impl ExplorerMotion {
    pub fn progress(&self, now: f64) -> f32 {
        motion::ENTER.apply(self.presence.progress(now))
    }
}

#[derive(Debug, Default)]
pub struct Motions {
    nodes: HashMap<NodeKey, NodeMotion>,
    timeline: Option<TimelineMotion>,
    popovers: HashMap<PopoverKind, Presence>,
    sheets: HashMap<SheetKind, Presence>,
    viewer: Option<ViewerMotion>,
    explorer: Option<ExplorerMotion>,
    expanded_browser: Option<ExplorerMotion>,
    get_started: Option<Presence>,
    words: HashMap<(WordsScope, usize), Vec<f64>>,
    transcript_height: Option<f32>,
    shift: Option<Spring>,
    emote: Option<EmoteState>,
    emote_from: Option<(EmoteState, f64)>,
    focus: super::focus::Focus,
}

impl Motions {
    pub fn sync_nodes(&mut self, slots: &[Slot], now: f64) {
        for slot in slots {
            let motion = self.nodes.entry(slot.key).or_insert_with(|| NodeMotion::new(slot, now));
            motion.presence.set(true, now);
            motion.column = slot.column;
            motion.x.set_target(slot.rect.x);
            motion.y.set_target(slot.rect.y);
            motion.h.set_target(slot.rect.h);
            motion.w = slot.rect.w;
        }
        for (key, motion) in &mut self.nodes {
            if !slots.iter().any(|s| s.key == *key) {
                motion.presence.set(false, now);
            }
        }
        self.nodes.retain(|_, m| m.presence.is_mounted(now));
    }

    pub fn nodes(&self) -> impl Iterator<Item = (&NodeKey, &NodeMotion)> {
        self.nodes.iter()
    }

    pub fn node(&self, key: NodeKey) -> Option<&NodeMotion> {
        self.nodes.get(&key)
    }

    pub fn sync_variant(&mut self, key: NodeKey, variant: u8, now: f64) {
        if let Some(motion) = self.nodes.get_mut(&key)
            && motion.variant != variant
        {
            motion.variant = variant;
            motion.swapped_at = now;
        }
    }

    pub fn sync_page(&mut self, key: NodeKey, page: usize, now: f64) {
        if let Some(motion) = self.nodes.get_mut(&key)
            && motion.page != page
        {
            motion.page_direction = if page > motion.page { 1.0 } else { -1.0 };
            motion.page = page;
            motion.paged_at = now;
        }
    }

    pub fn sync_focus(&mut self, keys: &[NodeKey], now: f64) {
        self.focus.sync(keys, now);
    }

    pub fn focus_opacity(&self, key: NodeKey, now: f64) -> f32 {
        self.focus.opacity(key, now)
    }

    pub fn sync_popover(&mut self, open: Option<PopoverKind>, now: f64) {
        if let Some(kind) = open {
            self.popovers
                .entry(kind)
                .or_insert_with(|| Presence::new(false, motion::POPOVER_OPEN, motion::POPOVER_CLOSE));
        }
        for (kind, presence) in &mut self.popovers {
            presence.set(Some(*kind) == open, now);
        }
        self.popovers.retain(|_, p| p.is_mounted(now));
    }

    pub fn sync_emote(&mut self, state: EmoteState, now: f64) {
        if let Some(current) = self.emote.replace(state)
            && current != state
        {
            self.emote_from = Some((current, now));
        }
    }

    pub fn emote_from(&self, now: f64) -> Option<(EmoteState, f32)> {
        let (state, at) = self.emote_from?;
        let k = ((now - at) as f32 / motion::EMOTE_MORPH).clamp(0.0, 1.0);
        Some((state, k))
    }

    pub fn popovers(&self) -> impl Iterator<Item = (&PopoverKind, &Presence)> {
        self.popovers.iter()
    }

    pub fn sync_viewer(&mut self, open: Option<usize>, from: Option<Rect>, now: f64) {
        match (open, &mut self.viewer) {
            (Some(index), Some(viewer)) if viewer.index == index => {
                viewer.presence.set(true, now);
                if let Some(from) = from {
                    viewer.from = from;
                }
            }
            (Some(index), _) => {
                let mut presence = Presence::new(false, motion::VIEWER_OPEN, motion::VIEWER_CLOSE);
                presence.set(true, now);
                let from = from.or_else(|| self.viewer.map(|v| v.from)).unwrap_or_default();
                self.viewer = Some(ViewerMotion { presence, index, from });
            }
            (None, Some(viewer)) => {
                viewer.presence.set(false, now);
                if let Some(from) = from {
                    viewer.from = from;
                }
                if !viewer.presence.is_mounted(now) {
                    self.viewer = None;
                }
            }
            (None, None) => {}
        }
    }

    pub fn sync_explorer(&mut self, open: bool, from: Option<Rect>, now: f64) {
        match &mut self.explorer {
            Some(explorer) => {
                explorer.presence.set(open, now);
                if let Some(from) = from {
                    explorer.from = from;
                }
                if !open && !explorer.presence.is_mounted(now) {
                    self.explorer = None;
                }
            }
            None if open => {
                let mut presence = Presence::new(false, motion::MEMORY_OPEN, motion::MEMORY_CLOSE);
                presence.set(true, now);
                self.explorer = Some(ExplorerMotion { presence, from: from.unwrap_or_default() });
            }
            None => {}
        }
    }

    pub fn sync_expanded_browser(&mut self, open: bool, from: Option<Rect>, now: f64) {
        match &mut self.expanded_browser {
            Some(grow) => {
                grow.presence.set(open, now);
                if let Some(from) = from {
                    grow.from = from;
                }
                if !open && !grow.presence.is_mounted(now) {
                    self.expanded_browser = None;
                }
            }
            None if open => {
                let mut presence = Presence::new(false, motion::BROWSER_EXPAND, motion::BROWSER_COLLAPSE);
                presence.set(true, now);
                self.expanded_browser = Some(ExplorerMotion { presence, from: from.unwrap_or_default() });
            }
            None => {}
        }
    }

    pub fn expanded_browser(&self, now: f64) -> Option<ExplorerMotion> {
        self.expanded_browser.filter(|e| e.presence.is_mounted(now))
    }

    pub fn sync_sheet(&mut self, kind: SheetKind, open: bool, now: f64) {
        match self.sheets.get_mut(&kind) {
            Some(presence) => {
                presence.set(open, now);
                if !open && !presence.is_mounted(now) {
                    self.sheets.remove(&kind);
                }
            }
            None if open => {
                let mut presence = Presence::new(false, motion::POPOVER_OPEN, motion::POPOVER_CLOSE);
                presence.set(true, now);
                self.sheets.insert(kind, presence);
            }
            None => {}
        }
    }

    pub fn sync_get_started(&mut self, open: bool, now: f64) {
        match &mut self.get_started {
            Some(presence) => {
                presence.set(open, now);
                if !presence.is_mounted(now) {
                    self.get_started = None;
                }
            }
            None if open => {
                let mut presence = Presence::new(false, motion::GET_STARTED_OPEN, motion::GET_STARTED_CLOSE);
                presence.set(true, now);
                self.get_started = Some(presence);
            }
            None => {}
        }
    }

    pub fn get_started(&self, now: f64) -> Option<Presence> {
        self.get_started.filter(|p| p.is_mounted(now))
    }

    pub fn sheet(&self, kind: SheetKind, now: f64) -> Option<Presence> {
        self.sheets.get(&kind).copied().filter(|s| s.is_mounted(now))
    }

    pub fn explorer(&self, now: f64) -> Option<ExplorerMotion> {
        self.explorer.filter(|e| e.presence.is_mounted(now))
    }

    pub fn viewer(&self, now: f64) -> Option<ViewerMotion> {
        self.viewer.filter(|v| v.presence.is_mounted(now))
    }

    pub fn sync_words(&mut self, scope: WordsScope, turn: usize, word_count: usize, now: f64) {
        let times = self.words.entry((scope, turn)).or_insert_with(|| vec![f64::NEG_INFINITY; word_count]);
        let mut next = times.last().map_or(now, |last| (last + motion::WORD_STAGGER as f64).max(now - 0.2));
        while times.len() < word_count {
            next = next.max(now - 0.2);
            times.push(next);
            next += motion::WORD_STAGGER as f64;
        }
    }

    pub fn word(&self, scope: WordsScope, turn: usize, index: usize, now: f64) -> f32 {
        self.words
            .get(&(scope, turn))
            .and_then(|times| times.get(index))
            .map_or(1.0, |start| Easing::OUT.apply(((now - start) as f32 / motion::WORD_REVEAL).clamp(0.0, 1.0)))
    }

    pub fn sync_timeline(&mut self, y: Option<f32>, now: f64) {
        let Some(y) = y else {
            self.timeline = None;
            return;
        };
        match &mut self.timeline {
            Some(timeline) => {
                if (timeline.y.target - y).abs() > 0.5 {
                    timeline.trail_from = timeline.y.value;
                    timeline.moved_at = now;
                    timeline.y.set_target(y);
                }
            }
            None => {
                self.timeline = Some(TimelineMotion {
                    y: Spring::new(y, SpringConfig::LAYOUT),
                    trail_from: y,
                    moved_at: f64::NEG_INFINITY,
                });
            }
        }
    }

    pub fn timeline_marker(&self, now: f64) -> Option<superfik_ui::components::TimelineMarker> {
        self.timeline.as_ref().map(|t| t.marker(now))
    }

    pub fn sync_transcript_height(&mut self, height: f32) {
        if let Some(previous) = self.transcript_height.replace(height) {
            let delta = height - previous;
            if delta.abs() > 0.5 {
                let shift = self.shift.get_or_insert_with(|| Spring::new(0.0, SpringConfig::LAYOUT));
                shift.value += delta;
                shift.set_target(0.0);
            }
        }
    }

    pub fn transcript_shift(&self) -> f32 {
        self.shift.as_ref().map_or(0.0, |s| s.value)
    }

    pub fn step(&mut self, dt: f32) {
        for motion in self.nodes.values_mut() {
            motion.x.step(dt);
            motion.y.step(dt);
            motion.h.step(dt);
        }
        if let Some(shift) = &mut self.shift {
            shift.step(dt);
        }
        if let Some(timeline) = &mut self.timeline {
            timeline.y.step(dt);
        }
    }

    pub fn is_animating(&self, now: f64) -> bool {
        self.nodes.values().any(|m| {
            m.presence.is_animating(now) || !m.settled() || m.swap_progress(now) < 1.0 || m.page_slide(now) != 0.0
        }) || self.popovers.values().any(|p| p.is_animating(now))
            || self.focus.is_animating(now)
            || self.viewer.is_some_and(|v| v.presence.is_animating(now))
            || self.expanded_browser.is_some_and(|b| b.presence.is_animating(now))
            || self.get_started.is_some_and(|p| p.is_animating(now))
            || self.emote_from(now).is_some_and(|(_, k)| k < 1.0)
            || self.shift.as_ref().is_some_and(|s| !s.is_settled())
            || self.timeline.as_ref().is_some_and(|t| !t.y.is_settled() || t.trail_faded(now) < 1.0)
            || self.words.values().any(|w| w.last().is_some_and(|t| now - t < motion::WORD_REVEAL as f64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(key: NodeKey, y: f32) -> Slot {
        Slot { key, column: Column::Left, rect: Rect::new(10.0, y, 216.0, 100.0) }
    }

    #[test]
    fn nodes_enter_and_exit() {
        let mut motions = Motions::default();
        motions.sync_nodes(&[slot(NodeKey::Skills, 0.0)], 0.0);
        let node = motions.node(NodeKey::Skills).expect("node");
        assert!(node.presence.is_animating(0.1));
        motions.sync_nodes(&[], 1.0);
        assert!(motions.node(NodeKey::Skills).is_some(), "still exiting");
        motions.sync_nodes(&[], 3.0);
        assert!(motions.node(NodeKey::Skills).is_none());
    }

    #[test]
    fn nodes_spring_toward_new_targets() {
        let mut motions = Motions::default();
        motions.sync_nodes(&[slot(NodeKey::Skills, 0.0)], 0.0);
        motions.sync_nodes(&[slot(NodeKey::Skills, 100.0)], 0.0);
        motions.step(0.05);
        let y = motions.node(NodeKey::Skills).expect("node").rect().y;
        assert!(y > 0.0 && y < 100.0);
        for _ in 0..200 {
            motions.step(1.0 / 60.0);
        }
        assert!((motions.node(NodeKey::Skills).expect("node").rect().y - 100.0).abs() < 0.1);
    }

    #[test]
    fn flipping_a_page_slides_it_in_from_that_side() {
        let mut motions = Motions::default();
        motions.sync_nodes(&[slot(NodeKey::Skills, 0.0)], 0.0);
        let slide = |m: &Motions, now| m.node(NodeKey::Skills).expect("node").page_slide(now);
        assert_eq!(slide(&motions, 0.0), 0.0);
        motions.sync_page(NodeKey::Skills, 1, 1.0);
        assert_eq!(slide(&motions, 1.0), 1.0, "forward comes in from the right");
        assert!(motions.is_animating(1.1));
        assert_eq!(slide(&motions, 2.0), 0.0);
        motions.sync_page(NodeKey::Skills, 0, 3.0);
        assert_eq!(slide(&motions, 3.0), -1.0);
    }

    #[test]
    fn swapping_content_restarts_its_reveal() {
        let mut motions = Motions::default();
        motions.sync_nodes(&[slot(NodeKey::Skills, 0.0)], 0.0);
        let node = |m: &Motions| m.node(NodeKey::Skills).expect("node").swap_progress(5.0);
        assert_eq!(node(&motions), 1.0);
        motions.sync_variant(NodeKey::Skills, 1, 5.0);
        assert_eq!(node(&motions), 0.0);
    }

    #[test]
    fn growing_conversation_glides_into_place() {
        let mut motions = Motions::default();
        motions.sync_transcript_height(100.0);
        assert_eq!(motions.transcript_shift(), 0.0);
        motions.sync_transcript_height(160.0);
        assert_eq!(motions.transcript_shift(), 60.0, "starts where it was");
        for _ in 0..300 {
            motions.step(1.0 / 60.0);
        }
        assert!(motions.transcript_shift().abs() < 0.5);
    }

    #[test]
    fn words_are_staggered_and_resolve() {
        let mut motions = Motions::default();
        let scope = scope();
        motions.sync_words(scope, 0, 0, 9.0);
        motions.sync_words(scope, 0, 3, 10.0);
        assert_eq!(motions.word(scope, 0, 0, 10.0), 0.0);
        assert!(motions.word(scope, 0, 2, 10.0 + 0.05) < motions.word(scope, 0, 0, 10.0 + 0.05));
        assert_eq!(motions.word(scope, 0, 2, 20.0), 1.0);
        assert_eq!(motions.word(scope, 5, 0, 0.0), 1.0, "unknown turns are fully shown");
    }

    fn scope() -> WordsScope {
        WordsScope { conversation: ConversationId::new(), agent: None }
    }

    #[test]
    fn a_turn_seen_for_the_first_time_is_shown_at_once() {
        let mut motions = Motions::default();
        let scope = scope();
        motions.sync_words(scope, 0, 40, 10.0);
        assert_eq!(motions.word(scope, 0, 0, 10.0), 1.0);
        assert_eq!(motions.word(scope, 0, 39, 10.0), 1.0);
        motions.sync_words(scope, 0, 42, 10.5);
        let fresh = motions.word(scope, 0, 41, 10.5);
        assert!(fresh < 1.0, "words that arrive after still resolve");
    }

    #[test]
    fn sessions_do_not_share_word_reveals() {
        let mut motions = Motions::default();
        let (a, b) = (scope(), scope());
        motions.sync_words(a, 0, 3, 10.0);
        motions.sync_words(b, 0, 50, 20.0);
        assert_eq!(motions.word(b, 0, 10, 20.0), 1.0, "b's words are not a's new arrivals");
    }

    #[test]
    fn entrance_beats_are_ordered() {
        let early = Entrance::at(0.3);
        assert!(early.connector > early.surface);
        assert_eq!(early.content, 0.0);
        assert_eq!(Entrance::at(1.0), Entrance { connector: 1.0, surface: 1.0, content: 1.0 });
    }

    #[test]
    fn timeline_dot_appears_in_place_then_travels_with_a_trail() {
        let mut motions = Motions::default();
        motions.sync_timeline(Some(100.0), 1.0);
        let marker = motions.timeline_marker(1.0).expect("marker");
        assert_eq!((marker.y, marker.trail), (100.0, 0.0), "no trail on first appearance");
        motions.sync_timeline(Some(180.0), 2.0);
        motions.step(0.05);
        let marker = motions.timeline_marker(2.05).expect("marker");
        assert!(marker.y > 100.0 && marker.y < 180.0);
        assert_eq!(marker.trail_from, 100.0);
        assert_eq!(marker.trail, 1.0);
        assert!(motions.is_animating(2.05));
        for _ in 0..300 {
            motions.step(1.0 / 60.0);
        }
        assert!((motions.timeline_marker(9.0).expect("marker").y - 180.0).abs() < 0.1);
        assert_eq!(motions.timeline_marker(9.0).expect("marker").trail, 0.0);
        assert!(!motions.is_animating(9.0));
        motions.sync_timeline(None, 9.0);
        assert!(motions.timeline_marker(9.0).is_none());
    }

    #[test]
    fn viewer_grows_from_its_tile_and_shrinks_back_to_it() {
        let mut motions = Motions::default();
        assert!(motions.viewer(0.0).is_none());
        let tile = Rect::new(40.0, 300.0, 60.0, 60.0);
        motions.sync_viewer(Some(1), Some(tile), 0.0);
        let viewer = motions.viewer(0.0).expect("viewer");
        assert_eq!((viewer.index, viewer.from, viewer.progress(0.0)), (1, tile, 0.0));
        assert!(motions.is_animating(0.1));
        assert_eq!(motions.viewer(5.0).expect("viewer").progress(5.0), 1.0);
        let moved = tile.translate(0.0, 50.0);
        motions.sync_viewer(Some(1), Some(moved), 5.0);
        motions.sync_viewer(None, None, 5.0);
        let closing = motions.viewer(5.1).expect("still closing");
        assert_eq!(closing.from, moved);
        assert!(closing.progress(5.1) < 1.0 && closing.progress(5.1) > 0.0);
        motions.sync_viewer(None, None, 9.0);
        assert!(motions.viewer(9.0).is_none());
        assert!(!motions.is_animating(9.0));
    }

    #[test]
    fn viewer_keeps_its_origin_while_the_tile_is_off_screen() {
        let mut motions = Motions::default();
        let tile = Rect::new(40.0, 300.0, 60.0, 60.0);
        motions.sync_viewer(Some(0), Some(tile), 0.0);
        motions.sync_viewer(Some(0), None, 1.0);
        assert_eq!(motions.viewer(1.0).expect("viewer").from, tile);
    }

    #[test]
    fn popover_switches_presence() {
        let mut motions = Motions::default();
        motions.sync_popover(Some(PopoverKind::Model), 0.0);
        motions.sync_popover(Some(PopoverKind::Branch), 1.0);
        let open: Vec<_> = motions.popovers().filter(|(_, p)| p.is_visible()).map(|(k, _)| *k).collect();
        assert_eq!(open, vec![PopoverKind::Branch]);
    }
}
