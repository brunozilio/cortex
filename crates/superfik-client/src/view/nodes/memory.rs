//! The Memory node: the memory graph alone. A click expands it into the
//! explorer, where the search and the legend live.

use superfik_core::{Memory, MemoryKind};
use superfik_i18n::{Strings, Text};
use superfik_render::{Color, Point, Quad, Rect};
use superfik_ui::Painter;
use superfik_ui::components::{GraphDot, GraphView, MemoryGraph, NodeFrame};
use superfik_ui::icons;
use superfik_ui::theme::colors::MemoryTones;
use superfik_ui::theme::{metrics, motion, typography};

use super::{NodeCtx, node_height, reveal};
use crate::state::MemoryState;
use crate::view::target::{MemoryTarget, Target};

#[derive(Debug, Clone, Copy)]
pub struct MemoryProps<'a> {
    pub state: &'a MemoryState,
    pub caret_visible: bool,
    pub now: f64,
}

impl MemoryProps<'_> {
    pub fn searching(&self) -> bool {
        self.state.searching()
    }

    pub fn activation(&self) -> &[f32] {
        &self.state.activation
    }

    pub fn activated(&self) -> usize {
        self.activation().iter().filter(|a| **a > 0.0).count()
    }
}

pub fn tone(tones: &MemoryTones, kind: MemoryKind) -> Color {
    match kind {
        MemoryKind::Episodic => tones.episodic,
        MemoryKind::Semantic => tones.semantic,
        MemoryKind::Procedural => tones.procedural,
    }
}

pub fn kind_label(strings: Strings, kind: MemoryKind) -> String {
    strings.t(match kind {
        MemoryKind::Episodic => Text::MemoryEpisodic,
        MemoryKind::Semantic => Text::MemorySemantic,
        MemoryKind::Procedural => Text::MemoryProcedural,
    })
}

pub fn dots(props: &MemoryProps, tones: &MemoryTones) -> Vec<GraphDot> {
    let state = props.state;
    let since = props.now - state.learned_at;
    let pulse = if since < motion::MEMORY_LEARNED {
        let fade = 1.0 - (since / motion::MEMORY_LEARNED) as f32;
        let wave = (since as f32 / motion::MEMORY_PULSE * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        fade * wave
    } else {
        0.0
    };
    state
        .memories
        .iter()
        .zip(props.activation().iter().chain(std::iter::repeat(&0.0)))
        .map(|(memory, light): (&Memory, &f32)| GraphDot {
            seed: memory.id.0,
            tone: tone(tones, memory.kind),
            radius: metrics::MEMORY_DOT[memory.kind as usize],
            light: match state.selected {
                Some(id) => f32::from(u8::from(id == memory.id)),
                None => *light,
            },
            grow: if state.selected == Some(memory.id) {
                1.0
            } else if state.is_learned(memory.id) {
                pulse
            } else {
                0.0
            },
        })
        .collect()
}

pub fn height() -> f32 {
    node_height(metrics::MEMORY_GRAPH_HEIGHT)
}

pub fn paint(p: &mut Painter, ctx: &mut NodeCtx, props: &MemoryProps) {
    let strings = ctx.strings;
    let state = props.state;
    let title = strings.t(Text::Memory);
    let count = state.memories.len().to_string();
    let mut frame = NodeFrame::new(ctx.rect, icons::BRAIN, Some(p.palette().memory), &title);
    frame.count = Some(&count);
    frame.busy = ctx.busy;
    frame.content = ctx.content;
    frame.time = ctx.time;
    frame.paint(p);
    ctx.hits.memory_node = Some(ctx.rect);
    let body = ctx.body();
    let well = Rect::new(body.x, body.y, body.w, metrics::MEMORY_GRAPH_HEIGHT);
    let dot_hits = reveal(p, ctx.content, |p| paint_well(p, well, props, GraphView::default(), strings));
    ctx.hits.add(well, Target::Memory(MemoryTarget::Open));
    for (memory, rect) in state.memories.iter().zip(dot_hits) {
        ctx.hits.add(rect, Target::Memory(MemoryTarget::Item(memory.id)));
    }
}

pub fn paint_well(p: &mut Painter, well: Rect, props: &MemoryProps, view: GraphView, strings: Strings) -> Vec<Rect> {
    let palette = p.palette();
    p.quad(Quad::new(well, metrics::WELL_RADIUS, palette.well));
    let empty = match (props.state.memories.is_empty(), props.searching() && props.activated() == 0) {
        (true, _) => Some(Text::MemoryEmpty),
        (false, true) => Some(Text::MemoryNoMatch),
        _ => None,
    };
    let dots = dots(props, &palette.memory_tones);
    let graph = MemoryGraph {
        dots: &dots,
        links: &props.state.links,
        searching: props.searching() || props.state.selected.is_some(),
        tones: palette.memory_tones,
        view,
    };
    let hits = graph.paint(p, well);
    if let Some(text) = empty {
        let layout = p.layout(&strings.t(text), typography::CAPTION, None);
        let at = Point::new(well.center().x - layout.width * 0.5, well.bottom() - layout.height - 8.0);
        p.text(&layout, at, palette.text_subtle);
    }
    hits
}

pub fn legend(p: &mut Painter, origin: Point, strings: Strings) {
    let palette = p.palette();
    let size = metrics::LEGEND_SWATCH;
    let mut x = origin.x;
    for kind in MemoryKind::ALL {
        let swatch = Rect::new(x + 3.0, origin.y - size * 0.5, size, size);
        p.quad(Quad::new(swatch, size * 0.5, tone(&palette.memory_tones, kind)));
        let name = kind_label(strings, kind);
        let label = p.label(&name, typography::LABEL, swatch.right() + 7.0, origin.y, palette.text_label);
        x = swatch.right() + 7.0 + label.width + metrics::MEMORY_LEGEND_GAP;
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{Author, MemoryId};

    use super::*;

    fn memory(id: u32, kind: MemoryKind) -> Memory {
        Memory {
            id: MemoryId(id),
            kind,
            text: format!("m{id}"),
            tags: vec![],
            created: 0,
            origin: None,
            author: Author::Agent,
        }
    }

    #[test]
    fn dots_follow_kind_selection_and_learning() {
        let mut state = MemoryState {
            memories: vec![memory(0, MemoryKind::Episodic), memory(4, MemoryKind::Semantic)],
            ..MemoryState::default()
        };
        state.selected = Some(MemoryId(4));
        state.learned = vec![MemoryId(0)];
        state.learned_at = 10.0;
        state.activation = vec![0.0, 1.0];
        let props = MemoryProps { state: &state, caret_visible: false, now: 10.3 };
        let tones = superfik_ui::theme::colors::DARK.memory_tones;
        let dots = dots(&props, &tones);
        assert_eq!((dots[0].seed, dots[0].tone), (0, tones.episodic));
        assert_eq!((dots[1].seed, dots[1].grow, dots[1].light), (4, 1.0, 1.0));
        assert!(dots[0].grow > 0.0, "freshly learned memories pulse");
        let later = MemoryProps { now: 10.0 + motion::MEMORY_LEARNED + 1.0, ..props };
        assert_eq!(super::dots(&later, &tones)[0].grow, 0.0);
    }

    #[test]
    fn the_node_holds_only_the_graph() {
        assert_eq!(height(), node_height(metrics::MEMORY_GRAPH_HEIGHT));
    }
}
