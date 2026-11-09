//! Repository node: the workspace's repo card, its branch row, the
//! repositories attached to the conversation and the row that adds one.

use superfik_core::Repository;
use superfik_i18n::Text;
use superfik_render::{Point, Quad, Rect};
use superfik_ui::components::{EarSide, Knob, NodeFrame, Row, Trailing};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

use super::{NodeCtx, node_height, reveal, rows_height};
use crate::view::target::Target;

#[derive(Debug, Clone, Copy)]
pub struct RepositoryProps<'a> {
    pub repository: Option<&'a Repository>,
    pub branch: &'a str,
    pub pull_requests: usize,
    pub attached: &'a [Repository],
    pub activity: Option<&'a str>,
}

const CARD: f32 = 52.0;

pub fn pull_requests_row_centre(node: Rect) -> f32 {
    let body = node.y + metrics::NODE_HEADER;
    body + CARD + metrics::ROW_GAP + metrics::ROW_HEIGHT + metrics::ROW_GAP + metrics::ROW_HEIGHT * 0.5
}

pub fn height(props: &RepositoryProps) -> f32 {
    if props.repository.is_none() {
        return node_height(CARD);
    }
    let rows = rows_height(2 + props.attached.len(), metrics::ROW_HEIGHT, metrics::ROW_GAP);
    node_height(CARD + metrics::ROW_GAP + rows)
}

pub fn paint(p: &mut Painter, ctx: &mut NodeCtx, props: &RepositoryProps) {
    let strings = ctx.strings;
    let title = ctx.strings.t(Text::Repository);
    let mut frame = NodeFrame::new(ctx.rect, icons::REPOSITORY, Some(p.palette().repository), &title);
    frame.busy = ctx.busy;
    frame.content = ctx.content;
    frame.time = ctx.time;
    frame.ear = Some(EarSide::Left);
    frame.ear_glyph = icons::PLUS;
    frame.ear_hovered = ctx.is_hovered(Target::AddRepository);
    let hits = frame.paint(p);
    if let Some(knob) = hits.knob {
        ctx.hits.add(knob, Target::AddRepository);
    }
    ctx.hits.repository_node = Some(ctx.rect);
    let body = ctx.body();
    let card = Rect::new(body.x, body.y, body.w, CARD);
    reveal(p, ctx.content, |p| {
        p.quad(Quad::new(card, metrics::WELL_RADIUS, p.palette().well));
        let (title, subtitle) = match props.repository {
            Some(repo) => (repo.slug.clone(), props.activity.map_or_else(|| repo.head_caption(), str::to_owned)),
            None => (strings.t(Text::OpenRepositoryTitle), strings.t(Text::OpenRepositoryHint)),
        };
        let x = card.x + 12.0;
        p.label_fit(&title, typography::TITLE, x, card.y + 17.0, card.w - 30.0 - x + card.x, p.palette().text);
        p.label_fit(
            &subtitle,
            typography::SUBTITLE,
            x,
            card.y + 38.5,
            card.right() - 12.0 - x,
            p.palette().text_subtle,
        );
        let Some(repository) = props.repository else { return };
        let knob = Knob::close(Point::new(card.right() - 14.0, card.y + 17.0), ctx.is_hovered(Target::RemoveWorkspace))
            .paint(p);
        ctx.hits.add(knob, Target::RemoveWorkspace);
        let mut y = card.bottom() + metrics::ROW_GAP;
        let row = Rect::new(body.x, y, body.w, metrics::ROW_HEIGHT);
        Row::new(&repository.branch_label(props.branch))
            .icon(icons::BRANCH, Some(p.palette().branch))
            .icon_size(metrics::ICON, metrics::BRANCH_ICON_GAP)
            .trailing(Trailing::Caret)
            .color(p.palette().text_label)
            .hovered(ctx.is_hovered(Target::Branch))
            .paint(p, row);
        ctx.hits.add(row, Target::Branch);
        y += metrics::ROW_HEIGHT + metrics::ROW_GAP;
        let row = Rect::new(body.x, y, body.w, metrics::ROW_HEIGHT);
        let count = props.pull_requests.to_string();
        let trailing = if props.pull_requests > 0 { Trailing::Count(&count) } else { Trailing::Caret };
        Row::new(&strings.t(Text::PullRequests))
            .icon(icons::PULL_REQUEST, Some(p.palette().branch))
            .icon_size(metrics::ICON, metrics::BRANCH_ICON_GAP)
            .trailing(trailing)
            .color(p.palette().text_label)
            .hovered(ctx.is_hovered(Target::PullRequests))
            .paint(p, row);
        ctx.hits.add(row, Target::PullRequests);
        y += metrics::ROW_HEIGHT + metrics::ROW_GAP;
        for (index, attached) in props.attached.iter().enumerate() {
            let row = Rect::new(body.x, y, body.w, metrics::ROW_HEIGHT);
            let label = format!("{} · {}", attached.name(), attached.branch);
            Row::new(&label)
                .icon(icons::REPOSITORY, Some(p.palette().repository))
                .trailing(Trailing::Remove { hovered: ctx.is_hovered(Target::RemoveRepository(index)) })
                .color(p.palette().text_label)
                .paint(p, row);
            ctx.hits.add(Row::knob_rect(row), Target::RemoveRepository(index));
            y += metrics::ROW_HEIGHT + metrics::ROW_GAP;
        }
    });
}

#[cfg(test)]
mod tests {
    use superfik_render::{Color, Scene, TextSystem};
    use superfik_ui::painter::LAYERS;
    use superfik_ui::theme::colors;

    use super::*;
    use crate::view::target::HitMap;

    #[test]
    fn a_long_commit_line_is_shortened_and_noted_for_its_tooltip() {
        let repository = Repository {
            slug: "owner/a-repository-name-that-is-long-enough-to-cut".into(),
            head_short_sha: "a1f3c2e".into(),
            head_subject: "Make the commit subject far longer than the card can ever show".into(),
            ..Repository::default()
        };
        let props = RepositoryProps {
            repository: Some(&repository),
            branch: "main",
            pull_requests: 0,
            attached: &[],
            activity: None,
        };
        let mut scene = Scene::new(Color::TRANSPARENT, LAYERS);
        let mut text = TextSystem::new();
        let mut p = Painter::new(&mut scene, &mut text, 1.0, &colors::DARK);
        let mut hits = HitMap::default();
        let mut ctx = NodeCtx {
            rect: Rect::new(0.0, 0.0, 216.0, height(&props)),
            content: 1.0,
            busy: false,
            time: 0.0,
            hovered: None,
            page_slide: 0.0,
            strings: superfik_i18n::Strings::default(),
            hits: &mut hits,
        };
        paint(&mut p, &mut ctx, &props);
        let noted: Vec<String> = p.take_elided().into_iter().map(|label| label.text).collect();
        assert!(noted.contains(&repository.head_caption()), "{noted:?}");
        assert!(noted.contains(&repository.slug), "{noted:?}");
    }
}
