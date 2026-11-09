//! A laboratory profile being edited: a card per group (the profile,
//! hardware, screen, GPU, fingerprint, proxy) with each value typed in
//! place, then Cancel and Save.

use super::{Line, SettingsScreenView};
use crate::state::browser_profiles::{Editor, ProfileField, ProfileToggle};
use crate::view::target::{HitMap, ProfilesTarget, SettingsTarget, Target};
use superfik_core::browser_profiles::{ProxyScheme, WebRtc};
use superfik_i18n::Text;
use superfik_render::{Point, Rect};
use superfik_ui::Painter;
use superfik_ui::components::{Button, CardControl, CardRow, TextField};
use superfik_ui::theme::{metrics, typography};

const GROUPS: [(Option<Text<'static>>, &[ProfileField]); 6] = {
    use ProfileField::*;
    [
        (None, &[Name]),
        (Some(Text::BrowserProfileHardware), &[Cpu, Memory, Fonts]),
        (Some(Text::BrowserProfileScreen), &[Width, Height, Scale, Depth]),
        (Some(Text::BrowserProfileGpu), &[GpuVendor, GpuRenderer, GpuArchitecture, GpuDevice, GpuDescription]),
        (Some(Text::BrowserProfileFingerprint), &[Seed]),
        (Some(Text::BrowserProfileProxy), &[ProxyHost, ProxyPort, ProxyUsername, ProxyPassword]),
    ]
};

impl SettingsScreenView<'_> {
    #[allow(clippy::too_many_arguments, reason = "stateless painting helper")]
    pub(super) fn paint_profile_editor(
        &self,
        p: &mut Painter,
        column: Rect,
        mut y: f32,
        visible: Rect,
        hits: &mut HitMap,
        editor: &Editor,
    ) -> f32 {
        let t = self.session.strings();
        let c = p.palette();
        p.label(&t.t(Text::BrowserProfiles), typography::HEADER, column.x + 2.0, y + 12.0, c.text_secondary);
        y += metrics::FORM_HEADING;
        let hint = p.layout(&t.t(Text::BrowserProfileDefaultsHint), typography::CAPTION, Some(column.w));
        p.text(&hint, Point::new(column.x + 2.0, y), c.text_secondary);
        y += hint.height + metrics::FORM_FIELD_GAP;

        let settings = &editor.original.settings;
        let labels: Vec<Vec<String>> = GROUPS
            .iter()
            .map(|(_, fields)| fields.iter().map(|f| t.t(Text::BrowserProfileField(*f))).collect())
            .collect();
        let (actual, keep) = (t.t(Text::BrowserProfileActual), t.t(Text::BrowserProfilePasswordHint));
        let toggles = [
            Text::BrowserProfileEnabled,
            Text::BrowserProfileCanvas,
            Text::BrowserProfileAudio,
            Text::BrowserProfileHttps,
            Text::BrowserProfileWebRtc,
            Text::BrowserProfileSeedCaption,
        ]
        .map(|text| t.t(text));
        let webrtc_labels =
            [Text::BrowserProfileRtcSystem, Text::BrowserProfileRtcProxy, Text::BrowserProfileRtcDisabled]
                .map(|x| t.t(x));
        let webrtc_labels = webrtc_labels.each_ref().map(String::as_str);
        const WEBRTC: [WebRtc; 3] = [WebRtc::System, WebRtc::ProxyOnly, WebRtc::Disabled];
        let https = settings.proxy.as_ref().is_some_and(|proxy| proxy.scheme == ProxyScheme::Https);
        let headings: Vec<Option<String>> = GROUPS.iter().map(|(heading, _)| heading.map(|h| t.t(h))).collect();

        for (index, (_, fields)) in GROUPS.iter().enumerate() {
            let mut lines: Vec<Line<'_>> = fields
                .iter()
                .zip(&labels[index])
                .map(|(&field, label)| {
                    let password = field == ProfileField::ProxyPassword;
                    let input = TextField {
                        edit: editor.field(field),
                        placeholder: if password { &keep } else { &actual },
                        focused: editor.focus == field,
                        caret_visible: self.caret_visible,
                        multiline: false,
                        secret: password,
                    };
                    let detail = (field == ProfileField::Seed).then_some(toggles[5].as_str());
                    Line {
                        row: CardRow { leading: None, title: label, detail, control: CardControl::Field(input) },
                        targets: vec![ProfilesTarget::Field(field)],
                    }
                })
                .collect();
            match fields.first() {
                Some(ProfileField::Name) => lines.push(switch(&toggles[0], settings.enabled, ProfileToggle::Enabled)),
                Some(ProfileField::Seed) => {
                    lines.push(switch(&toggles[1], settings.canvas, ProfileToggle::Canvas));
                    lines.push(switch(&toggles[2], settings.audio, ProfileToggle::Audio));
                    lines.push(Line {
                        row: CardRow {
                            leading: None,
                            title: &toggles[4],
                            detail: None,
                            control: CardControl::Choice {
                                labels: &webrtc_labels,
                                selected: WEBRTC.iter().position(|w| *w == settings.webrtc).unwrap_or(0),
                                enabled: true,
                            },
                        },
                        targets: WEBRTC.map(ProfilesTarget::WebRtc).to_vec(),
                    });
                }
                Some(ProfileField::ProxyHost) => lines.push(switch(&toggles[3], https, ProfileToggle::Https)),
                _ => {}
            }
            y = self.profile_card(p, column, y, headings[index].as_deref(), &lines, visible, hits);
        }
        self.paint_profile_footer(p, column, y, visible, hits)
    }

    fn paint_profile_footer(&self, p: &mut Painter, column: Rect, y: f32, visible: Rect, hits: &mut HitMap) -> f32 {
        let t = self.session.strings();
        let state = &self.session.browser_profiles;
        let enabled = state.loaded && !state.saving;
        let mut right = column.right();
        for (text, target, primary) in [
            (Text::BrowserProfileSave, ProfilesTarget::Save, true),
            (Text::BrowserProfileCancel, ProfilesTarget::Cancel, false),
        ] {
            let label = t.t(text);
            let settings = SettingsTarget::Profile(target);
            let button = Button { label: &label, primary, hovered: enabled && self.is(settings), enabled };
            let width = button.width(p);
            let rect = button.paint(p, Rect::new(right - width, y, width, metrics::SETTINGS_BUTTON));
            if enabled {
                hits.add_screen(rect.intersect(&visible), Target::Settings(settings));
            }
            right = rect.x - metrics::FORM_SHEET_BUTTON_GAP;
        }
        y + metrics::SETTINGS_BUTTON + metrics::FORM_FIELD_GAP
    }
}

fn switch(title: &str, on: bool, toggle: ProfileToggle) -> Line<'_> {
    Line {
        row: CardRow { leading: None, title, detail: None, control: CardControl::Switch { on, enabled: true } },
        targets: vec![ProfilesTarget::Toggle(toggle)],
    }
}

#[cfg(test)]
mod tests {
    use super::GROUPS;
    use crate::state::browser_profiles::ORDER;

    #[test]
    fn the_cards_show_every_field_in_tab_order() {
        let shown: Vec<_> = GROUPS.iter().flat_map(|(_, fields)| fields.iter().copied()).collect();
        assert_eq!(shown, ORDER);
    }
}
