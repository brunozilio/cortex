//! Skills node: the workspace skills the agent may consult (all of them
//! until some are taken out), with a "+" to bring removed ones back.

use superfik_core::{Skill, SkillIcon};
use superfik_i18n::Text;
use superfik_render::{Icon, Rect};
use superfik_ui::components::{EarSide, Menu, MenuItem, NodeFrame, Pager, Row, Trailing};
use superfik_ui::theme::colors::Palette;
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

use super::{NodeCtx, node_height, paged, reveal};
use crate::state::{Carousel, Pick};
use crate::view::target::Target;

const ROW: f32 = 34.0;

pub fn icon(c: &Palette, icon: SkillIcon) -> (Icon, Option<superfik_render::Color>) {
    match icon {
        SkillIcon::Cube => (icons::CUBE, Some(c.skill_cube)),
        SkillIcon::Bolt => (icons::BOLT, Some(c.skill_bolt)),
        SkillIcon::Sparkle => (icons::SPARKLE, Some(c.sparkle)),
        SkillIcon::Book => (icons::BOOK, Some(c.reference)),
    }
}

pub fn picker_height(count: usize) -> f32 {
    let pager = if count > metrics::PAGE_ROWS { metrics::PAGER_HEIGHT } else { 0.0 };
    Menu::height(count.clamp(1, metrics::PAGE_ROWS)) + pager
}

pub fn pick_icon(c: &Palette, pick: &Pick) -> (Icon, Option<superfik_render::Color>) {
    match pick {
        Pick::Skill(skill) => icon(c, skill.icon),
        Pick::Connection(connection) => super::connections::plugin_glyph(c, &connection.icon),
        Pick::Plugin(plugin) => super::connections::plugin_glyph(c, &plugin.icon),
        Pick::Command(_) => (icons::BOLT, Some(c.skill_bolt)),
        Pick::Note(_) => (icons::NOTE, Some(c.notes)),
    }
}

pub fn paint_picker(
    p: &mut Painter,
    ctx: &mut NodeCtx,
    all: &[(usize, Pick)],
    cursor: usize,
    (carousel, page, pages): (Carousel, usize, usize),
) {
    p.surface_rect(ctx.rect, metrics::NODE_RADIUS, metrics::GOO);
    let picks: Vec<(usize, Pick)> =
        all.iter().skip(page * metrics::PAGE_ROWS).take(metrics::PAGE_ROWS).copied().collect();
    let picks = &picks[..];
    let labels: Vec<String> = picks.iter().map(|(_, pick)| pick.label()).collect();
    let items: Vec<MenuItem> = picks
        .iter()
        .zip(&labels)
        .map(|((_, pick), label)| MenuItem { icon: Some(pick_icon(p.palette(), pick)), ..MenuItem::new(label) })
        .collect();
    let hovered = picks
        .iter()
        .position(|(i, _)| ctx.is_hovered(Target::SkillItem(*i)))
        .or_else(|| picks.iter().position(|(i, _)| *i == cursor));
    let rows = reveal(p, ctx.content, |p| Menu { items: &items, hovered }.paint(p, ctx.rect));
    for ((index, _), row) in picks.iter().zip(rows) {
        ctx.hits.add(row, Target::SkillItem(*index));
    }
    if pages < 2 {
        return;
    }
    let bar = Rect::new(
        ctx.rect.x + Menu::PADDING,
        ctx.rect.y + Menu::PADDING + metrics::PAGE_ROWS.min(all.len()) as f32 * Menu::ROW,
        ctx.rect.w - Menu::PADDING * 2.0,
        metrics::PAGER_HEIGHT,
    );
    let pager = Pager {
        page,
        pages,
        hovered_prev: ctx.is_hovered(Target::Page(carousel, -1)),
        hovered_next: ctx.is_hovered(Target::Page(carousel, 1)),
    }
    .paint(p, bar);
    for (rect, by) in [(pager.prev, -1), (pager.next, 1)] {
        if let Some(rect) = rect {
            ctx.hits.add(rect, Target::Page(carousel, by));
        }
    }
}

pub fn height(count: usize) -> f32 {
    node_height(paged::body_height(count.max(1), ROW))
}

pub fn paint(p: &mut Painter, ctx: &mut NodeCtx, skills: &[Skill], page: usize) {
    let count = skills.len().to_string();
    let title = ctx.strings.t(Text::Skills);
    let mut frame = NodeFrame::new(ctx.rect, icons::LAYERS, Some(p.palette().skill), &title);
    frame.count = Some(&count);
    frame.busy = ctx.busy;
    frame.ear = Some(EarSide::Left);
    frame.ear_glyph = icons::PLUS;
    frame.ear_hovered = ctx.is_hovered(Target::AddSkill);
    frame.content = ctx.content;
    frame.time = ctx.time;
    let hits = frame.paint(p);
    if let Some(knob) = hits.knob {
        ctx.hits.add(knob, Target::AddSkill);
    }
    let body = ctx.body();
    let none = ctx.strings.t(Text::NoSkills);
    reveal(p, ctx.content, |p| {
        if skills.is_empty() {
            Row::new(&none).muted(true).paint(p, Rect::new(body.x, body.y, body.w, ROW));
        }
        paged::paint(p, ctx, Carousel::Skills, (skills.len(), ROW, page), |p, ctx, i, row| {
            paint_row(p, ctx, &skills[i], i, row);
        });
    });
}

fn paint_row(p: &mut Painter, ctx: &mut NodeCtx, skill: &Skill, index: usize, row: Rect) {
    let (glyph, tint) = icon(p.palette(), skill.icon);
    let size = if skill.icon == SkillIcon::Bolt { metrics::SKILL_BOLT } else { metrics::SKILL_ICON };
    Row::new(&skill.title)
        .icon(glyph, tint)
        .icon_size(size, metrics::SKILL_ICON + metrics::SKILL_ICON_GAP - size)
        .style(typography::SKILL)
        .color(p.palette().text)
        .trailing(Trailing::Remove { hovered: ctx.is_hovered(Target::RemoveSkill(index)) })
        .paint(p, row);
    ctx.hits.add(Row::knob_rect(row), Target::RemoveSkill(index));
}
