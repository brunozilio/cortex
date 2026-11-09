//! The Settings screen, over the whole window like the other screens: the
//! shared header (back, title), a column of sections at the left and the
//! chosen pane centred in what is left: its title, a caption and its
//! groups of settings as cards. Painted in window coordinates; every
//! region is a screen hit so what is underneath is inert.

mod ai;
mod ai_local;
mod card_body;
mod card_link;
mod collaboration;
mod field;
mod governance;
mod integrations;
mod logins;
mod organization;
mod profiles;
pub mod rows;
mod server;
mod voice;

use superfik_i18n::Text;
use superfik_render::{Quad, Rect};
use superfik_ui::components::{CardControl, CardRow, FormCard, Leading, Row, ScreenHeader, TextField};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, Plane, icons};

use super::target::{HitMap, SettingsTarget, Target};
use crate::state::{Session, SettingsPane};

pub struct SettingsScreenView<'a> {
    pub session: &'a Session,
    pub hovered: Option<Target>,
    pub viewport: (f32, f32),
    pub browser_icons: &'a std::collections::HashMap<superfik_browser::import::BrowserKind, superfik_render::ImageId>,
    pub now: f64,
    pub caret_visible: bool,
}

impl SettingsScreenView<'_> {
    fn is(&self, target: SettingsTarget) -> bool {
        self.hovered == Some(Target::Settings(target))
    }

    pub fn paint(&self, p: &mut Painter, hits: &mut HitMap) {
        if !self.session.settings.open {
            return;
        }
        let strings = self.session.strings();
        let window = Rect::new(0.0, 0.0, self.viewport.0, self.viewport.1);
        p.on_plane(Plane::Canvas, |p| {
            p.quad(Quad::new(window, 0.0, p.palette().canvas));
            hits.add_screen(window, Target::Settings(SettingsTarget::Surface));
            let back = strings.t(Text::ReviewBack);
            let header = ScreenHeader {
                top_inset: self.session.platform.titlebar_height(),
                back: &back,
                back_hovered: self.is(SettingsTarget::Back),
            }
            .paint(p, window);
            hits.add_screen(header.back, Target::Settings(SettingsTarget::Back));
            ScreenHeader::title(p, &header, &strings.t(Text::SettingsTitle), header.actions_right);
            let body = ScreenHeader::body(window, self.session.platform.titlebar_height());
            let nav = Rect::new(body.x, body.y, metrics::SETTINGS_NAV_WIDTH.min(body.w), body.h);
            self.paint_nav(p, nav, hits);
            let left = nav.right() + metrics::SCREEN_CONTENT_INSET;
            let room = (body.right() - metrics::SCREEN_CONTENT_INSET - left).max(0.0);
            let width = room.min(metrics::SETTINGS_CONTENT_WIDTH);
            let column = Rect::new((left + (room - width) * 0.5).round(), body.y, width, body.h);
            let visible = Rect::new(nav.right(), body.y, window.right() - nav.right(), window.bottom() - body.y);
            p.clipped(visible, |p| self.paint_pane(p, column, visible, hits));
            super::notice::paint_on_screen(p, self.session, column, self.hovered, hits);
        });
    }

    fn paint_nav(&self, p: &mut Painter, nav: Rect, hits: &mut HitMap) {
        let strings = self.session.strings();
        let mut y = nav.y + metrics::SETTINGS_TOP;
        for pane in SettingsPane::offered(self.session.platform) {
            let rect = Rect::new(nav.x, y, nav.w, metrics::SETTINGS_NAV_ROW);
            let shown = pane == self.session.settings.pane;
            let target = SettingsTarget::Pane(pane);
            let name = strings.t(rows::pane_name(pane));
            let tint = if shown { p.palette().icon } else { p.palette().icon_muted };
            if shown {
                p.quad(Quad::new(rect, metrics::ROW_RADIUS, p.palette().timeline_hover));
            }
            let row = Row::new(&name).icon(pane_icon(pane), Some(tint)).style(typography::MENU).bare();
            row.hovered(self.is(target) && !shown).paint(p, rect);
            hits.add_screen(rect, Target::Settings(target));
            y += metrics::SETTINGS_NAV_ROW + metrics::SETTINGS_NAV_GAP;
        }
    }

    fn paint_pane(&self, p: &mut Painter, column: Rect, visible: Rect, hits: &mut HitMap) {
        let strings = self.session.strings();
        let c = p.palette();
        let pane = self.session.settings.pane;
        let scroll = self.session.plugins.panel.scroll;
        let top = column.y + metrics::SETTINGS_TOP;
        let mut y = top - scroll;
        let title = p.label(&strings.t(rows::pane_name(pane)), typography::STORE_DISPLAY, column.x, y + 16.0, c.text);
        y += title.height.max(32.0);
        let caption = p.layout(&strings.t(rows::pane_caption(pane)), typography::STORE_BODY, Some(column.w));
        p.text(&caption, superfik_render::Point::new(column.x, y), c.text_secondary);
        y += caption.height + metrics::SETTINGS_INTRO_GAP;
        if pane.lists_plugins() {
            let bottom = self.paint_plugins(p, column, y, visible, hits);
            let content = bottom + scroll - top;
            hits.settings_scroll = Some((visible, (content - (visible.bottom() - top)).max(0.0)));
            return;
        }
        let hovered = match self.hovered {
            Some(Target::Settings(SettingsTarget::Choose(setting))) => Some(setting),
            _ => None,
        };
        let groups = match pane {
            SettingsPane::Browser => {
                let session = self.session;
                let import = session.platform.import_logins;
                let mut groups =
                    logins::groups(&session.logins, session.preferences.logins, import, &strings, self.now);
                groups.extend(rows::groups(pane, self.session.preferences, &strings));
                groups
            }
            SettingsPane::Voice => {
                voice::groups(self.session.preferences, self.session.voice_field_edit.as_ref(), &strings)
            }
            SettingsPane::Server => {
                let session = self.session;
                let admin = session.manages_organization();
                let mut groups = server::connection(session, &strings);
                groups.extend(server::administration(session, &strings));
                groups.extend(organization::groups(&session.organization, admin, &strings));
                let on_server = session.organization.joined();
                if on_server {
                    groups.extend(collaboration::teams(&session.organization, admin, &strings));
                }
                if admin && on_server {
                    groups.push(governance::policy(&session.organization, &session.models, &strings));
                    groups.extend(governance::usage(&session.organization, &strings));
                }
                groups
            }
            SettingsPane::General if self.session.offers_lock() => {
                let mut groups = rows::groups(pane, self.session.preferences, &strings);
                if let Some(method) = self.session.platform.system_unlock {
                    groups.push(rows::lock(self.session.preferences, &strings, method.name()));
                }
                groups
            }
            _ => rows::groups(pane, self.session.preferences, &strings),
        };
        for group in groups {
            let labels: Vec<Vec<&str>> = group
                .lines
                .iter()
                .map(|line| match &line.control {
                    rows::Control::Choice { options, .. } => options.iter().map(|(name, _)| name.as_str()).collect(),
                    rows::Control::Switch { .. }
                    | rows::Control::Action { .. }
                    | rows::Control::Buttons(_)
                    | rows::Control::Field { .. }
                    | rows::Control::None => Vec::new(),
                })
                .collect();
            let buttons: Vec<Vec<(&str, superfik_render::Icon)>> = group
                .lines
                .iter()
                .map(|line| match &line.control {
                    rows::Control::Buttons(buttons) => {
                        buttons.iter().map(|(label, icon, _)| (label.as_str(), *icon)).collect()
                    }
                    _ => Vec::new(),
                })
                .collect();
            let cards: Vec<CardRow<'_>> = group
                .lines
                .iter()
                .zip(labels.iter().zip(&buttons))
                .map(|(line, (labels, buttons))| CardRow {
                    leading: line.browser.map(|kind| match self.browser_icons.get(&kind) {
                        Some(image) => Leading::Image(*image),
                        None => Leading::Glyph(icons::GLOBE),
                    }),
                    title: &line.title,
                    detail: line.detail.as_deref(),
                    control: match &line.control {
                        rows::Control::Switch { on, .. } => CardControl::Switch { on: *on, enabled: true },
                        rows::Control::Choice { selected, enabled, .. } => {
                            CardControl::Choice { labels, selected: *selected, enabled: *enabled }
                        }
                        rows::Control::Action { labels, icon, run, progress } => CardControl::Button {
                            labels: labels.each_ref().map(String::as_str),
                            icon: *icon,
                            enabled: run.is_some(),
                            progress: *progress,
                        },
                        rows::Control::Buttons(_) => CardControl::Buttons(buttons),
                        rows::Control::Field { edit, focused, field } => CardControl::Field(TextField {
                            edit,
                            placeholder: field.placeholder(),
                            focused: *focused,
                            caret_visible: *focused && self.caret_visible,
                            multiline: false,
                            secret: field.secret(),
                        }),
                        rows::Control::None => CardControl::None,
                    },
                })
                .collect();
            let hovered = hovered
                .and_then(|setting| {
                    group.lines.iter().enumerate().find_map(|(i, line)| Some((i, line.control.part_of(setting)?)))
                })
                .or_else(|| {
                    let Some(Target::Settings(SettingsTarget::Run(hovered))) = self.hovered else { return None };
                    group.lines.iter().enumerate().find_map(|(row, line)| match &line.control {
                        rows::Control::Action { run: Some(run), .. } if *run == hovered => Some((row, 0)),
                        rows::Control::Buttons(buttons) => {
                            buttons.iter().position(|(.., run)| *run == hovered).map(|part| (row, part))
                        }
                        _ => None,
                    })
                });
            let card = FormCard { heading: group.heading.as_deref(), rows: &cards, hovered };
            let height = card.height();
            let parts = card.paint(p, Rect::new(column.x, y, column.w, height));
            for (line, rects) in group.lines.iter().zip(parts) {
                for (index, rect) in rects.into_iter().enumerate() {
                    let target = match &line.control {
                        rows::Control::Action { run: Some(run), .. } => SettingsTarget::Run(*run),
                        rows::Control::Buttons(buttons) => match buttons.get(index) {
                            Some((.., run)) => SettingsTarget::Run(*run),
                            None => continue,
                        },
                        rows::Control::Field { field, .. } => SettingsTarget::Field(*field),
                        control => match control.setting(index) {
                            Some(setting) => SettingsTarget::Choose(setting),
                            None => continue,
                        },
                    };
                    hits.add_screen(rect.intersect(&visible), Target::Settings(target));
                }
            }
            y += height + metrics::FORM_GROUP_GAP;
        }
        if pane == SettingsPane::Browser {
            y = self.paint_browser_profiles(p, column, y, visible, hits);
        }
        let content = y + scroll - top;
        hits.settings_scroll = Some((visible, (content - (visible.bottom() - top)).max(0.0)));
    }
}

fn pane_icon(pane: SettingsPane) -> superfik_render::Icon {
    match pane {
        SettingsPane::General => icons::GEAR,
        SettingsPane::Sound => icons::SPEAKER,
        SettingsPane::Ai => icons::BRAIN,
        SettingsPane::Voice => icons::MIC,
        SettingsPane::Browser => icons::GLOBE,
        SettingsPane::Integrations => icons::PLUG,
        SettingsPane::Vaults => icons::SHIELD,
        SettingsPane::Server => icons::PLUG,
    }
}
