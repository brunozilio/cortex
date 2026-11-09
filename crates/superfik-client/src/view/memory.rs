//! The memory explorer: the Memory node expanded over the canvas area, with
//! the graph on the left and the ranked list on the right. Painted in
//! window coordinates on the overlay plane; its regions are screen hits.

use superfik_core::{Memory, MemoryKind};
use superfik_i18n::{Strings, Text};
use superfik_motion::remap;
use superfik_render::{Point, Quad, Rect};
use superfik_ui::components::{Button, GraphView, Knob, ScrollView, Segmented, TextField};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, Plane, icons};

use super::motions::Motions;
use super::nodes::memory::{self as node, MemoryProps};
use super::target::{HitMap, MemoryTarget, Target};
use crate::state::MemoryField;

const DRAFT: f32 = Segmented::HEIGHT + 8.0 + metrics::MEMORY_SEARCH;

pub struct ExplorerView<'a> {
    pub props: MemoryProps<'a>,
    pub strings: Strings,
    pub hovered: Option<Target>,
    pub area: Rect,
    pub view: GraphView,
    pub shareable: bool,
}

impl ExplorerView<'_> {
    pub fn paint(&self, p: &mut Painter, motions: &mut Motions, node: Option<Rect>, hits: &mut HitMap) {
        let state = self.props.state;
        let now = self.props.now;
        motions.sync_explorer(state.open, node, now);
        let Some(explorer) = motions.explorer(now) else { return };
        let t = explorer.progress(now);
        let panel = self.area.inset(metrics::MEMORY_EXPLORER_MARGIN);
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        let from = explorer.from;
        let rect =
            Rect::new(lerp(from.x, panel.x), lerp(from.y, panel.y), lerp(from.w, panel.w), lerp(from.h, panel.h));
        let content = remap(t, 0.6, 1.0);
        let mut inert = HitMap::default();
        let hits = if explorer.presence.is_visible() && state.open {
            hits.add_screen(self.area, Target::Memory(MemoryTarget::Close));
            hits.add_screen(panel, Target::Memory(MemoryTarget::Surface));
            hits
        } else {
            &mut inert
        };
        p.on_plane(Plane::Canvas, |p| p.quad(Quad::new(self.area, 0.0, p.palette().scrim.faded(t))));
        p.on_plane(Plane::Overlay, |p| {
            p.surface_rect(rect, metrics::NODE_RADIUS, metrics::GOO);
            if content <= 0.0 {
                return;
            }
            let blur = (1.0 - content) * superfik_ui::theme::motion::BLUR;
            p.faded(content, |p| p.blurred(blur, |p| self.paint_content(p, panel, hits)));
        });
    }

    fn paint_content(&self, p: &mut Painter, panel: Rect, hits: &mut HitMap) {
        let state = self.props.state;
        let strings = self.strings;
        let pad = metrics::NODE_PADDING * 1.6;
        let header = Rect::new(panel.x, panel.y, panel.w, metrics::NODE_HEADER + 8.0);
        let cy = header.center().y;
        p.icon_at(icons::BRAIN, Point::new(header.x + pad + 8.0, cy), 16.0, Some(p.palette().memory));
        let title = p.label(&strings.t(Text::Memory), typography::TITLE, header.x + pad + 24.0, cy, p.palette().text);
        if self.props.searching() {
            let activated = strings.t(Text::MemoryActivated(self.props.activated()));
            let x = header.x + pad + 24.0 + title.width + 10.0;
            p.label(&activated, typography::CAPTION, x, cy, p.palette().text_subtle);
        }
        let close_hovered = self.hovered == Some(Target::Memory(MemoryTarget::Close));
        let close = Knob::close(Point::new(header.right() - pad - 6.0, cy), close_hovered).paint(p);
        hits.add_screen(close.inset(-6.0), Target::Memory(MemoryTarget::Close));
        self.paint_actions(p, Rect::new(header.x, header.y, close.x - 6.0 - header.x - 12.0, header.h), hits);

        let body =
            Rect::new(panel.x + pad, header.bottom(), panel.w - pad * 2.0, panel.bottom() - header.bottom() - pad);
        let graph_w = (body.w - metrics::MEMORY_EXPLORER_GAP) * metrics::MEMORY_EXPLORER_GRAPH;
        let graph = Rect::new(body.x, body.y, graph_w, body.h);
        hits.memory_graph = Some(graph);
        hits.add_screen(graph, Target::Memory(MemoryTarget::Graph));
        let dots = node::paint_well(p, graph, &self.props, self.view, strings);
        let inset = metrics::MEMORY_GRAPH_PADDING;
        node::legend(p, Point::new(graph.x + inset, graph.bottom() - inset - metrics::LEGEND_ROW * 0.5), strings);
        for (memory, rect) in state.memories.iter().zip(dots) {
            hits.add_screen(rect, Target::Memory(MemoryTarget::Item(memory.id)));
        }
        let gap = metrics::MEMORY_EXPLORER_GAP;
        let column = Rect::new(graph.right() + gap, body.y, body.right() - graph.right() - gap, body.h);
        let search = Rect::new(column.x, column.y, column.w, metrics::MEMORY_SEARCH);
        let placeholder = strings.t(Text::SearchMemory);
        TextField {
            edit: &state.query,
            placeholder: &placeholder,
            focused: state.focus == Some(MemoryField::Search),
            caret_visible: self.props.caret_visible,
            multiline: false,
            secret: false,
        }
        .paint(p, search);
        hits.add_screen(search, Target::Memory(MemoryTarget::Search));
        let top = search.bottom() + metrics::ROW_GAP * 2.0;
        let list = Rect::new(column.x, top, column.w, column.bottom() - DRAFT - gap - top);
        self.paint_list(p, list, hits);
        self.paint_draft(p, Rect::new(column.x, list.bottom() + gap, column.w, DRAFT), hits);
    }

    fn paint_actions(&self, p: &mut Painter, header: Rect, hits: &mut HitMap) {
        let state = self.props.state;
        let height = Segmented::HEIGHT;
        let mut right = header.right();
        let label =
            self.strings.t(if state.optimizing.is_some() { Text::OptimizingMemory } else { Text::OptimizeMemory });
        let mut buttons = vec![(label, MemoryTarget::Optimize, state.optimizing.is_none() && state.memories.len() > 1)];
        if state.can_undo {
            buttons.push((self.strings.t(Text::UndoOptimize), MemoryTarget::Undo, true));
        }
        for (label, target, enabled) in buttons {
            let button = Button {
                label: &label,
                primary: false,
                hovered: enabled && self.hovered == Some(Target::Memory(target)),
                enabled,
            };
            let width = button.width(p);
            let rect = Rect::new(right - width, header.center().y - height * 0.5, width, height);
            let painted = button.paint(p, rect);
            if enabled {
                hits.add_screen(painted, Target::Memory(target));
            }
            right = rect.x - metrics::ROW_GAP;
        }
    }

    fn paint_list(&self, p: &mut Painter, list: Rect, hits: &mut HitMap) {
        let state = self.props.state;
        let width = list.w - ScrollView::THUMB_WIDTH - 10.0;
        let order = state.listed();
        let heights: Vec<f32> = order.iter().map(|&i| self.item_height(p, &state.memories[i], width)).collect();
        let total: f32 = heights.iter().sum::<f32>() + metrics::MEMORY_ITEM_GAP * order.len().saturating_sub(1) as f32;
        let scroll = ScrollView { content_height: total, offset: state.scroll };
        let (entries, scrolled) = scroll.paint(p, list, |p, origin| {
            let mut y = origin.y;
            let mut entries = Vec::with_capacity(order.len());
            for (&i, height) in order.iter().zip(&heights) {
                let rect = Rect::new(origin.x, y, width, *height);
                if rect.bottom() >= list.y && rect.y <= list.bottom() {
                    entries.push(self.paint_item(p, rect, &state.memories[i]));
                }
                y += height + metrics::MEMORY_ITEM_GAP;
            }
            entries
        });
        for (id, rect, controls) in entries {
            hits.add_screen(rect.intersect(&list), Target::Memory(MemoryTarget::Item(id)));
            for (control, target) in controls.into_iter().filter(|(r, _)| list.contains(r.center())) {
                hits.add_screen(control, Target::Memory(target));
            }
        }
        hits.memory_list = Some((list, scrolled.max_offset));
        if order.is_empty() {
            let text = if state.memories.is_empty() { Text::MemoryEmpty } else { Text::MemoryNoMatch };
            p.label(&self.strings.t(text), typography::MENU, list.x + 4.0, list.y + 14.0, p.palette().text_subtle);
        }
    }

    fn paint_item(
        &self,
        p: &mut Painter,
        rect: Rect,
        memory: &Memory,
    ) -> (superfik_core::MemoryId, Rect, Vec<(Rect, MemoryTarget)>) {
        let palette = p.palette();
        let state = self.props.state;
        let id = memory.id;
        let selected = state.selected == Some(id);
        let hovered = matches!(
            self.hovered,
            Some(Target::Memory(
                MemoryTarget::Item(h) | MemoryTarget::Forget(h) | MemoryTarget::Revise(h) | MemoryTarget::Share(h)
            )) if h == id
        );
        let fill = if selected {
            palette.control_active
        } else if hovered {
            palette.hover
        } else {
            palette.well
        };
        p.quad(Quad::new(rect, metrics::WELL_RADIUS, fill));
        let pad = metrics::MEMORY_ITEM_PADDING;
        let size = metrics::LEGEND_SWATCH;
        let tone = node::tone(&palette.memory_tones, memory.kind);
        p.quad(Quad::new(Rect::new(rect.x + pad, rect.y + pad + 6.0, size, size), size * 0.5, tone));
        let text_x = rect.x + pad + size + 8.0;
        let text_w = rect.right() - pad - text_x - KNOBS_ROOM;
        let mut controls = Vec::new();
        match state.revising.as_ref().filter(|r| r.id == id) {
            Some(revision) => {
                let width = rect.right() - pad - text_x;
                let field =
                    Rect::new(text_x - 4.0, rect.y + pad - 4.0, width + 4.0, editor_height(p, &revision.edit, width));
                TextField {
                    edit: &revision.edit,
                    placeholder: "",
                    focused: state.focus == Some(MemoryField::Revision),
                    caret_visible: self.props.caret_visible,
                    multiline: true,
                    secret: false,
                }
                .paint(p, field);
                controls.push((field, MemoryTarget::Revision));
                let labels: Vec<String> = MemoryKind::ALL.iter().map(|k| node::kind_label(self.strings, *k)).collect();
                let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
                let selected = MemoryKind::ALL.iter().position(|k| *k == revision.kind).unwrap_or(0);
                let hovered = MemoryKind::ALL
                    .iter()
                    .position(|k| self.hovered == Some(Target::Memory(MemoryTarget::RevisionKind(*k))));
                let origin = Point::new(field.x, field.bottom() + metrics::ROW_GAP);
                let segments = Segmented { labels: &labels, selected, hovered }.paint(p, origin);
                for (kind, segment) in MemoryKind::ALL.iter().zip(segments) {
                    controls.push((segment, MemoryTarget::RevisionKind(*kind)));
                }
            }
            None => {
                let layout = p.layout(&memory.text, typography::MENU, Some(text_w));
                p.text(&layout, Point::new(text_x, rect.y + pad), palette.text_body);
            }
        }
        if (hovered || selected) && state.revising.is_none() {
            let cy = rect.y + pad + 8.0;
            let forget_at = Point::new(rect.right() - pad - 6.0, cy);
            let revise_at = Point::new(forget_at.x - Knob::RADIUS * 2.0 - 6.0, cy);
            let revise_hovered = self.hovered == Some(Target::Memory(MemoryTarget::Revise(id)));
            let revise = Knob { glyph: icons::PENCIL, ..Knob::close(revise_at, revise_hovered) }.paint(p);
            controls.push((revise, MemoryTarget::Revise(id)));
            if self.shareable {
                let share_at = Point::new(revise_at.x - Knob::RADIUS * 2.0 - 6.0, cy);
                let share_hovered = self.hovered == Some(Target::Memory(MemoryTarget::Share(id)));
                let share = Knob { glyph: icons::PERSON, ..Knob::close(share_at, share_hovered) }.paint(p);
                controls.push((share, MemoryTarget::Share(id)));
            }
            let forget_hovered = self.hovered == Some(Target::Memory(MemoryTarget::Forget(id)));
            controls.push((Knob::close(forget_at, forget_hovered).paint(p), MemoryTarget::Forget(id)));
        }
        (id, rect, controls)
    }

    fn item_height(&self, p: &mut Painter, memory: &Memory, width: f32) -> f32 {
        let pad = metrics::MEMORY_ITEM_PADDING;
        let text_x = pad + metrics::LEGEND_SWATCH + 8.0;
        let body = match self.props.state.revising.as_ref().filter(|r| r.id == memory.id) {
            Some(revision) => {
                editor_height(p, &revision.edit, width - pad - text_x) - 4.0 + metrics::ROW_GAP + Segmented::HEIGHT
            }
            None => p.layout(&memory.text, typography::MENU, Some(width - pad - text_x - KNOBS_ROOM)).height,
        };
        pad + body + pad
    }

    fn paint_draft(&self, p: &mut Painter, area: Rect, hits: &mut HitMap) {
        let state = self.props.state;
        let labels: Vec<String> = MemoryKind::ALL.iter().map(|k| node::kind_label(self.strings, *k)).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let hovered = MemoryKind::ALL.iter().position(|k| self.hovered == Some(Target::Memory(MemoryTarget::Kind(*k))));
        let selected = MemoryKind::ALL.iter().position(|k| *k == state.draft_kind).unwrap_or(0);
        let segments = Segmented { labels: &labels, selected, hovered }.paint(p, Point::new(area.x, area.y));
        for (kind, rect) in MemoryKind::ALL.iter().zip(segments) {
            hits.add_screen(rect, Target::Memory(MemoryTarget::Kind(*kind)));
        }
        let label = self.strings.t(Text::Remember);
        let button = Button {
            label: &label,
            primary: true,
            hovered: self.hovered == Some(Target::Memory(MemoryTarget::Remember)),
            enabled: !state.draft.text().trim().is_empty(),
        };
        let width = button.width(p);
        let rect = Rect::new(area.right() - width, area.y, width, Segmented::HEIGHT);
        hits.add_screen(button.paint(p, rect), Target::Memory(MemoryTarget::Remember));
        let field = Rect::new(area.x, area.y + Segmented::HEIGHT + 8.0, area.w, metrics::MEMORY_SEARCH);
        let placeholder = self.strings.t(Text::RememberPlaceholder);
        TextField {
            edit: &state.draft,
            placeholder: &placeholder,
            focused: state.focus == Some(MemoryField::Draft),
            caret_visible: self.props.caret_visible,
            multiline: false,
            secret: false,
        }
        .paint(p, field);
        hits.add_screen(field, Target::Memory(MemoryTarget::Draft));
    }
}

const KNOBS_ROOM: f32 = 38.0;

fn editor_height(p: &mut Painter, edit: &superfik_ui::TextEdit, width: f32) -> f32 {
    let text = if edit.text().is_empty() { " " } else { edit.text() };
    let inner = width - superfik_ui::components::text_field::INSET * 2.0;
    p.layout(text, typography::INPUT, Some(inner)).height + 14.0
}
