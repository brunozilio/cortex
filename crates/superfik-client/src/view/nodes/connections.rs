//! Plugins node: every installed plugin, with a select of the account the
//! session uses. Plugins in use are lit, the others dimmed; a plugin with no
//! account yet offers to connect one. Accounts are also picked with `#`.

use superfik_i18n::Text;
use superfik_render::{Color, Icon, Rect};
use superfik_ui::components::{NodeFrame, Row, Trailing};
use superfik_ui::theme::colors::Palette;
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

use super::{NodeCtx, node_height, paged, reveal};
use crate::state::{Carousel, PluginRow, account_label};
use crate::view::target::Target;

pub fn plugin_glyph(c: &Palette, key: &str) -> (Icon, Option<Color>) {
    let brand = superfik_ui::brands::brand(key);
    match brand.mark {
        Some(mark) => (mark, (!brand.own_colours).then_some(brand.color.mix(c.text, 0.35))),
        None => (icons::PLUG, Some(brand.color.mix(c.text, 0.2))),
    }
}

pub fn height(count: usize) -> f32 {
    node_height(paged::body_height(count, metrics::ROW_HEIGHT))
}

pub fn row_centre(node: Rect, index: usize) -> f32 {
    paged::row_top(node.y + metrics::NODE_HEADER, index, metrics::ROW_HEIGHT) + metrics::ROW_HEIGHT * 0.5
}

pub fn paint(p: &mut Painter, ctx: &mut NodeCtx, rows: &[PluginRow], page: usize) {
    let count = rows.iter().filter(|row| row.used.is_some()).count().to_string();
    let title = ctx.strings.t(Text::Connections);
    let not_used = ctx.strings.t(Text::AccountNotUsed);
    let connect = ctx.strings.t(Text::Connect);
    let mut frame = NodeFrame::new(ctx.rect, icons::PLUG, Some(p.palette().plugin), &title);
    frame.count = Some(&count);
    frame.busy = ctx.busy;
    frame.content = ctx.content;
    frame.time = ctx.time;
    frame.paint(p);
    reveal(p, ctx.content, |p| {
        paged::paint(p, ctx, Carousel::Plugins, (rows.len(), metrics::ROW_HEIGHT, page), |p, ctx, i, rect| {
            let row = &rows[i];
            let used = row.used.is_some();
            let value = match (row.used, row.accounts.is_empty()) {
                (Some(connection), _) => account_label(connection),
                (None, true) => connect.as_str(),
                (None, false) => not_used.as_str(),
            };
            let (glyph, tint) = plugin_glyph(p.palette(), row.icon);
            let color = if used { p.palette().text_label } else { p.palette().text_subtle };
            let tint = if used { tint } else { tint.map(|t| t.mix(p.palette().text_subtle, 0.5)) };
            let target = Target::PluginRow(i);
            Row::new(row.name)
                .icon(glyph, tint)
                .style(typography::SKILL)
                .color(color)
                .trailing(Trailing::Select(value))
                .hovered(ctx.is_hovered(target))
                .paint(p, rect);
            ctx.hits.add(rect, target);
        });
    });
}

#[cfg(test)]
mod tests {
    use superfik_ui::theme::colors;

    use super::*;

    #[test]
    fn brands_show_their_mark_and_others_a_plug() {
        assert_eq!(plugin_glyph(&colors::DARK, "github").0.name, "brand-github");
        assert_eq!(plugin_glyph(&colors::DARK, "github").1, None);
        assert_eq!(plugin_glyph(&colors::LIGHT, "instagram").1, None);
        assert_eq!(plugin_glyph(&colors::DARK, "unheard-of").0.name, icons::PLUG.name);
        assert_eq!(height(2), node_height(super::super::rows_height(2, metrics::ROW_HEIGHT, metrics::ROW_GAP)));
    }
}
