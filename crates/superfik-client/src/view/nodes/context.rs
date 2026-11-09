//! Context node: how full the conversation's context window is, what
//! fills it, and the row that compacts it.

use superfik_core::{Compaction, ContextSlice, ContextUsage, compact_tokens};
use superfik_i18n::{Strings, Text};
use superfik_render::{Color, Point, Quad, Rect};
use superfik_ui::components::{NodeFrame, Row, Trailing, UsageGrid};
use superfik_ui::theme::colors::{ContextTones, Palette};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

use super::{NodeCtx, node_height, reveal};
use crate::view::target::Target;

#[derive(Debug, Clone, Copy)]
pub struct ContextProps<'a> {
    pub usage: Option<&'a ContextUsage>,
    pub measuring: bool,
    pub compacting: bool,
    pub can_compact: bool,
    pub compaction: Option<Compaction>,
}

const TOTALS: f32 = 30.0;

fn well_height() -> f32 {
    let inner = metrics::NODE_WIDTH - metrics::NODE_PADDING * 2.0 - metrics::USAGE_WELL_PADDING * 2.0;
    UsageGrid::height(inner) + metrics::USAGE_WELL_PADDING * 2.0
}

fn legend_lines(props: &ContextProps) -> usize {
    props.usage.map_or(0, |u| u.shares.iter().filter(|s| s.tokens > 0).count())
}

pub fn height(props: &ContextProps) -> f32 {
    let legend = legend_lines(props) as f32 * metrics::LEGEND_ROW;
    let gap = if legend > 0.0 { metrics::ROW_GAP } else { 0.0 };
    node_height(well_height() + TOTALS + legend + gap + metrics::ROW_HEIGHT)
}

pub fn tone(tones: &ContextTones, slice: &ContextSlice) -> Color {
    match slice {
        ContextSlice::SystemPrompt => tones.system,
        ContextSlice::Tools => tones.tools,
        ContextSlice::Mcp => tones.mcp,
        ContextSlice::Agents => tones.agents,
        ContextSlice::Memory => tones.memory,
        ContextSlice::Skills => tones.skills,
        ContextSlice::Messages => tones.messages,
        ContextSlice::Other(_) => tones.other,
    }
}

pub fn paint(p: &mut Painter, ctx: &mut NodeCtx, props: &ContextProps) {
    let strings = ctx.strings;
    let title = strings.t(Text::Context);
    let percent = props.usage.map(|u| format!("{}%", u.percent()));
    let mut frame = NodeFrame::new(ctx.rect, icons::GAUGE, Some(p.palette().context), &title);
    frame.count = percent.as_deref();
    frame.busy = ctx.busy;
    frame.content = ctx.content;
    frame.time = ctx.time;
    frame.paint(p);
    let body = ctx.body();
    let hovered = ctx.is_hovered(Target::CompactContext);
    let compact = reveal(p, ctx.content, |p| {
        let mut y = body.y;
        let well = Rect::new(body.x, y, body.w, well_height());
        paint_grid(p, well, props.usage);
        y = well.bottom();
        paint_totals(p, Rect::new(body.x, y, body.w, TOTALS), props.usage, props.measuring, strings);
        y += TOTALS;
        if let Some(usage) = props.usage {
            for share in usage.shares.iter().filter(|s| s.tokens > 0) {
                let line = Rect::new(body.x, y, body.w, metrics::LEGEND_ROW);
                let color = tone(&p.palette().context_tones, &share.slice);
                legend_line(p, line, color, strings.context_slice(&share.slice), share.tokens);
                y += metrics::LEGEND_ROW;
            }
            if legend_lines(props) > 0 {
                y += metrics::ROW_GAP;
            }
        }
        let row = Rect::new(body.x, y, body.w, metrics::ROW_HEIGHT);
        paint_compact(p, row, props, hovered, strings);
        row
    });
    if props.can_compact {
        ctx.hits.add(compact, Target::CompactContext);
    }
}

fn paint_grid(p: &mut Painter, well: Rect, usage: Option<&ContextUsage>) {
    let palette: &Palette = p.palette();
    let tones = palette.context_tones;
    p.quad(Quad::new(well, metrics::WELL_RADIUS, palette.well));
    let segments: Vec<(u64, Color)> =
        usage.map(|u| u.shares.iter().map(|s| (s.tokens, tone(&tones, &s.slice))).collect()).unwrap_or_default();
    let grid = UsageGrid {
        segments: &segments,
        capacity: usage.map_or(0, |u| u.window),
        reserved: usage.map_or(0, |u| u.reserved),
        free: tones.free,
        reserved_ring: tones.reserved,
    };
    grid.paint(p, well.inset(metrics::USAGE_WELL_PADDING));
}

fn paint_totals(p: &mut Painter, line: Rect, usage: Option<&ContextUsage>, measuring: bool, strings: Strings) {
    let cy = line.center().y;
    let x = line.x + 2.0;
    let Some(usage) = usage else {
        let text = strings.t(if measuring { Text::ContextMeasuring } else { Text::ContextEmpty });
        p.label(&text, typography::SUBTITLE, x, cy, p.palette().text_subtle);
        return;
    };
    let totals = format!("{} / {}", compact_tokens(usage.used), compact_tokens(usage.window));
    p.label(&totals, typography::TITLE, x, cy, p.palette().text);
    let free = strings.t(Text::ContextFree(&compact_tokens(usage.free())));
    right_label(p, &free, typography::CAPTION, line.right() - 2.0, cy, p.palette().text_subtle);
}

fn legend_line(p: &mut Painter, line: Rect, color: Color, name: &str, tokens: u64) {
    let cy = line.center().y;
    let size = metrics::LEGEND_SWATCH;
    p.quad(Quad::new(Rect::new(line.x + 3.0, cy - size * 0.5, size, size), size * 0.5, color));
    let amount = compact_tokens(tokens);
    let width = right_label(p, &amount, typography::CAPTION, line.right() - 2.0, cy, p.palette().text_subtle);
    let x = line.x + 3.0 + size + 8.0;
    p.label_fit(name, typography::LABEL, x, cy, line.right() - 10.0 - width - x, p.palette().text_label);
}

fn paint_compact(p: &mut Painter, row: Rect, props: &ContextProps, hovered: bool, strings: Strings) {
    let label = strings.t(if props.compacting { Text::Compacting } else { Text::CompactContext });
    let freed = props.compaction.map(|c| format!("−{}", compact_tokens(c.freed())));
    let trailing = freed.as_deref().map_or(Trailing::None, Trailing::Count);
    Row::new(&label)
        .icon(icons::LAYERS, Some(p.palette().context))
        .trailing(trailing)
        .color(p.palette().text_label)
        .muted(!props.can_compact && !props.compacting)
        .hovered(hovered && props.can_compact)
        .paint(p, row);
}

fn right_label(
    p: &mut Painter,
    text: &str,
    style: superfik_render::TextStyle,
    right: f32,
    cy: f32,
    color: Color,
) -> f32 {
    let layout = p.layout(text, style, None);
    let width = layout.width;
    p.text(&layout, Point::new(right - width, (cy - layout.height * 0.5).round()), color);
    width
}

#[cfg(test)]
mod tests {
    use superfik_core::ContextShare;

    use super::*;

    fn props(usage: Option<&ContextUsage>) -> ContextProps<'_> {
        ContextProps { usage, measuring: false, compacting: false, can_compact: true, compaction: None }
    }

    #[test]
    fn the_legend_grows_the_node_one_line_per_share() {
        let share = |slice, tokens| ContextShare { slice, tokens };
        let usage = ContextUsage {
            used: 10_000,
            window: 200_000,
            reserved: 33_000,
            shares: vec![
                share(ContextSlice::SystemPrompt, 4_000),
                share(ContextSlice::Messages, 6_000),
                share(ContextSlice::Memory, 0),
            ],
        };
        let measuring = height(&props(None));
        let measured = height(&props(Some(&usage)));
        assert_eq!(measured - measuring, 2.0 * metrics::LEGEND_ROW + metrics::ROW_GAP);
    }
}
