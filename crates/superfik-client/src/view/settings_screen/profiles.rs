//! Laboratory profiles on the Browser pane, painted entirely from reducer
//! state as form cards: the profiles (actual values, each profile with its
//! summary, a new one) or, while one is edited, its editor.

mod editor;
mod summary;

use super::SettingsScreenView;
use crate::view::target::{HitMap, ProfilesTarget, SettingsTarget, Target};
use superfik_i18n::Text;
use superfik_render::{Icon, Point, Rect};
use superfik_ui::components::{CardControl, CardRow, FormCard, Leading, Progress};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

struct Line<'a> {
    row: CardRow<'a>,
    targets: Vec<ProfilesTarget>,
}

impl SettingsScreenView<'_> {
    pub(super) fn paint_browser_profiles(
        &self,
        p: &mut Painter,
        column: Rect,
        y: f32,
        visible: Rect,
        hits: &mut HitMap,
    ) -> f32 {
        let state = &self.session.browser_profiles;
        let mut y = match &state.editor {
            Some(editor) => self.paint_profile_editor(p, column, y, visible, hits, editor),
            None => self.paint_profile_list(p, column, y, visible, hits),
        };
        let t = self.session.strings();
        let restart = state.restart_required.then(|| (t.t(Text::BrowserProfileRestart), p.palette().text_secondary));
        let error = state.error.clone().map(|error| (error, p.palette().text));
        for (text, color) in restart.into_iter().chain(error) {
            let layout = p.layout(&text, typography::CAPTION, Some(column.w));
            p.text(&layout, Point::new(column.x + 2.0, y), color);
            y += layout.height + metrics::ROW_GAP;
        }
        y + metrics::FORM_GROUP_GAP
    }

    fn paint_profile_list(&self, p: &mut Painter, column: Rect, y: f32, visible: Rect, hits: &mut HitMap) -> f32 {
        let t = self.session.strings();
        let state = &self.session.browser_profiles;
        let assigned = state.book.assignments.get(&self.session.conversation().id.to_string()).copied();
        let (real, caption, in_use) =
            (t.t(Text::BrowserProfileReal), t.t(Text::BrowserProfilesCaption), t.t(Text::BrowserProfileInUse));
        let (use_, edit, delete) =
            (t.t(Text::BrowserProfileUse), t.t(Text::BrowserProfileEdit), t.t(Text::BrowserProfileDelete));
        let (new, new_caption, create) =
            (t.t(Text::BrowserProfileNew), t.t(Text::BrowserProfileNewCaption), t.t(Text::BrowserProfileCreate));
        let details: Vec<String> = state
            .book
            .profiles
            .iter()
            .map(|profile| {
                let summary = summary::summary(&profile.settings, &t);
                if assigned == Some(profile.id) { format!("{in_use} · {summary}") } else { summary }
            })
            .collect();
        let with_use =
            [(use_.as_str(), icons::CHECK), (edit.as_str(), icons::PENCIL), (delete.as_str(), icons::DELETE)];
        let in_place = [(edit.as_str(), icons::PENCIL), (delete.as_str(), icons::DELETE)];

        let mut lines = Vec::with_capacity(state.book.profiles.len() + 2);
        lines.push(match assigned {
            None => Line { row: row(icons::CHECK_CIRCLE, &real, &in_use, CardControl::None), targets: Vec::new() },
            Some(_) => Line {
                row: row(icons::GLOBE, &real, &caption, button(&use_, icons::CHECK)),
                targets: vec![ProfilesTarget::Assign(None)],
            },
        });
        for (profile, detail) in state.book.profiles.iter().zip(&details) {
            let used = assigned == Some(profile.id);
            let (icon, buttons, mut targets) = if used {
                (icons::CHECK_CIRCLE, &in_place[..], Vec::new())
            } else {
                (icons::FLASK, &with_use[..], vec![ProfilesTarget::Assign(Some(profile.id))])
            };
            targets.extend([ProfilesTarget::Edit(profile.id), ProfilesTarget::Delete(profile.id)]);
            lines.push(Line { row: row(icon, &profile.name, detail, CardControl::Buttons(buttons)), targets });
        }
        lines.push(Line {
            row: row(icons::PLUS_CIRCLE, &new, &new_caption, button(&create, icons::PLUS)),
            targets: vec![ProfilesTarget::New],
        });
        let heading = t.t(Text::BrowserProfiles);
        self.profile_card(p, column, y, Some(&heading), &lines, visible, hits)
    }

    #[allow(clippy::too_many_arguments, reason = "stateless painting helper")]
    fn profile_card(
        &self,
        p: &mut Painter,
        column: Rect,
        y: f32,
        heading: Option<&str>,
        lines: &[Line<'_>],
        visible: Rect,
        hits: &mut HitMap,
    ) -> f32 {
        let hovered = match self.hovered {
            Some(Target::Settings(SettingsTarget::Profile(target))) => lines
                .iter()
                .enumerate()
                .find_map(|(i, line)| Some((i, line.targets.iter().position(|t| *t == target)?))),
            _ => None,
        };
        let rows: Vec<CardRow<'_>> = lines.iter().map(|line| line.row).collect();
        let card = FormCard { heading, rows: &rows, hovered };
        let height = card.height();
        let parts = card.paint(p, Rect::new(column.x, y, column.w, height));
        let state = &self.session.browser_profiles;
        if state.loaded && !state.saving {
            for (line, rects) in lines.iter().zip(parts) {
                for (rect, target) in rects.into_iter().zip(&line.targets) {
                    hits.add_screen(rect.intersect(&visible), Target::Settings(SettingsTarget::Profile(*target)));
                }
            }
        }
        y + height + metrics::FORM_GROUP_GAP
    }
}

fn row<'a>(icon: Icon, title: &'a str, detail: &'a str, control: CardControl<'a>) -> CardRow<'a> {
    CardRow { leading: Some(Leading::Glyph(icon)), title, detail: Some(detail), control }
}

fn button(label: &str, icon: Icon) -> CardControl<'_> {
    CardControl::Button { labels: [label; 3], icon, enabled: true, progress: Progress::Idle }
}
