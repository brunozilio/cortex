//! The review screen: covers the canvas area (the sidebar stays) with a
//! header, the changed files on the left and the diff on the right, plus
//! the publish sheet. Painted in window coordinates on the overlay plane;
//! every region is a screen hit so the canvas underneath is inert.

mod diff;
mod header;
mod publish;

use superfik_i18n::Text;
use superfik_render::{Quad, Rect};
use superfik_ui::components::{FileGlyph, FileList, FileRow, ScrollView};
use superfik_ui::{Painter, Plane};

use super::target::{HitMap, ReviewTarget, Target};
use crate::state::Session;

const FILES_WIDTH: f32 = 260.0;
const PADDING: f32 = 12.0;

pub struct ReviewView<'a> {
    pub session: &'a Session,
    pub hovered: Option<Target>,
    pub now: f64,
    pub area: Rect,
    pub caret_visible: bool,
}

impl ReviewView<'_> {
    pub fn paint(&self, p: &mut Painter, motions: &mut super::motions::Motions, hits: &mut HitMap) {
        let Some(review) = self.session.review.as_ref().filter(|r| r.open) else { return };
        let strings = self.session.strings();
        let area = self.area;
        p.on_plane(Plane::Canvas, |p| {
            p.quad(Quad::new(area, 0.0, p.palette().canvas));
            let top = self.session.platform.titlebar_height();
            let below_titlebar = Rect::new(area.x, area.y + top, area.w, area.h - top);
            hits.add_screen(below_titlebar, Target::Review(ReviewTarget::Surface));
            let header = header::paint(p, self.session, review, &strings, area, self.hovered, hits);
            let body = Rect::new(area.x, header.bottom(), area.w, area.bottom() - header.bottom());
            let files_pane = Rect::new(body.x + PADDING, body.y, FILES_WIDTH, body.h - PADDING);
            let diff_pane = Rect::new(
                files_pane.right() + PADDING,
                body.y,
                body.right() - files_pane.right() - PADDING * 2.0,
                body.h - PADDING,
            );
            hits.review_panes = Some((files_pane, diff_pane));
            match review.detail.as_ref() {
                Some(detail) if !detail.files.is_empty() => {
                    let rows: Vec<FileRow> = detail
                        .files
                        .iter()
                        .enumerate()
                        .map(|(i, file)| FileRow {
                            path: file.path(),
                            glyph: match file.status {
                                superfik_core::FileStatus::Added => FileGlyph::Added,
                                superfik_core::FileStatus::Removed => FileGlyph::Removed,
                                superfik_core::FileStatus::Modified => FileGlyph::Modified,
                                superfik_core::FileStatus::Renamed => FileGlyph::Renamed,
                            },
                            additions: file.additions(),
                            deletions: file.deletions(),
                            comments: review.comments_on(file.path()).count(),
                            selected: i == review.selected_file,
                            hovered: self.hovered == Some(Target::Review(ReviewTarget::File(i))),
                        })
                        .collect();
                    let list = FileList { rows: &rows };
                    let scroll =
                        ScrollView { content_height: FileList::height(rows.len()), offset: review.files_scroll };
                    let (row_rects, _) = scroll.paint(p, files_pane, |p, origin| {
                        list.paint(
                            p,
                            Rect::new(origin.x, origin.y, files_pane.w - ScrollView::THUMB_WIDTH - 8.0, f32::MAX),
                        )
                    });
                    for (i, rect) in row_rects.into_iter().enumerate() {
                        hits.add_screen(rect.intersect(&files_pane), Target::Review(ReviewTarget::File(i)));
                    }
                    if let Some(file) = detail.files.get(review.selected_file) {
                        diff::paint(
                            p,
                            self.session,
                            review,
                            file,
                            &strings,
                            diff_pane,
                            self.hovered,
                            self.caret_visible,
                            hits,
                        );
                    }
                }
                Some(_) => {
                    p.label(
                        &strings.t(Text::ReviewNoFiles),
                        superfik_ui::theme::typography::MENU,
                        files_pane.x + 8.0,
                        files_pane.y + 20.0,
                        p.palette().text_subtle,
                    );
                }
                None => {
                    p.label(
                        &strings.t(Text::LoadingPullRequest(review.number)),
                        superfik_ui::theme::typography::MENU,
                        files_pane.x + 8.0,
                        files_pane.y + 20.0,
                        p.palette().text_subtle,
                    );
                }
            }
        });
        publish::paint(
            p,
            self.session,
            review,
            &strings,
            area,
            motions,
            self.now,
            self.hovered,
            self.caret_visible,
            hits,
        );
    }
}
