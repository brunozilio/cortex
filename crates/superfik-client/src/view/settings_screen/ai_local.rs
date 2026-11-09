//! Local model downloads alongside the cloud account cards.
use super::SettingsScreenView;
use crate::view::target::{HitMap, PluginsTarget};
use superfik_i18n::Text;
use superfik_protocol::DownloadState;
use superfik_render::{Point, Quad, Rect};
use superfik_ui::components::{Button, Row, TextField, Trailing};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

impl SettingsScreenView<'_> {
    pub(super) fn paint_local_models(
        &self,
        p: &mut Painter,
        column: Rect,
        y: f32,
        visible: Rect,
        hits: &mut HitMap,
    ) -> f32 {
        let strings = self.session.strings();
        let c = p.palette();
        let pad = metrics::FORM_ROW_PADDING;
        let open = self.session.plugins.panel.local_models;
        let choices = self.session.local_model_choices();
        let has_models = choices.iter().any(|(_, _, index)| index.is_some());
        let picker_height = metrics::ROW_HEIGHT
            + metrics::ROW_GAP
            + if self.session.plugins.panel.local_picker {
                choices.len() as f32 * (metrics::ROW_HEIGHT + metrics::ROW_GAP)
            } else {
                0.0
            };
        let downloads: Vec<_> =
            self.session.plugins.downloads.iter().filter(|(_, state)| **state != DownloadState::Done).collect();
        let line = 26.0;
        let head = metrics::FORM_ROW_DETAILED;
        let body = if open {
            14.0 + (usize::from(!has_models) + downloads.len()) as f32 * line
                + picker_height
                + 22.0
                + metrics::ROW_HEIGHT
                + metrics::ROW_GAP
                + metrics::CHIP_HEIGHT
                + pad
        } else {
            0.0
        };
        let card = Rect::new(column.x, y, column.w, head + body);
        p.quad(Quad::new(card, metrics::FORM_CARD_RADIUS, c.surface_top).border(c.divider, 1.0));
        let header = Rect::new(card.x, y, card.w, head);
        let target = PluginsTarget::LocalModels;
        if self.is_plugin(target) {
            p.quad(Quad::new(header, metrics::FORM_CARD_RADIUS, c.hover));
        }
        self.plugin_hit(hits, header, visible, target);
        p.icon_at(icons::BRAIN, Point::new(card.x + pad + 18.0, header.center().y), 28.0, Some(c.icon));
        let x = card.x + pad + 36.0 + metrics::FORM_ICON_GAP;
        p.label_fit(
            &strings.t(Text::AiLocalModels),
            typography::MENU,
            x,
            header.center().y - 9.0,
            card.w - (x - card.x) - pad * 2.0,
            c.text,
        );
        let caption = strings.t(Text::AiLocalCaption);
        p.label_fit(
            &caption,
            typography::SUBTITLE,
            x,
            header.center().y + 9.0,
            card.w - (x - card.x) - pad * 2.0,
            c.text_secondary,
        );
        p.icon_at(
            if open { icons::CARET_UP } else { icons::CHEVRON_DOWN },
            Point::new(card.right() - pad, header.center().y),
            metrics::ICON_LARGE,
            Some(c.icon_muted),
        );
        if open {
            let inner = card.w - 2.0 * pad;
            let x = card.x + pad;
            let mut at = y + head + 14.0;
            if !has_models {
                p.label(&strings.t(Text::AiLocalEmpty), typography::SUBTITLE, x, at + line * 0.5, c.text_subtle);
                at += line;
            }
            let selected = self.session.local_model_id();
            let label = choices
                .iter()
                .find(|(id, _, _)| selected.as_ref() == Some(id))
                .map(|(_, label, _)| label.clone())
                .unwrap_or_else(|| strings.t(Text::AiLocalSelect));
            let picker = Rect::new(x, at, inner, metrics::ROW_HEIGHT);
            Row::new(&label)
                .trailing(Trailing::Caret)
                .hovered(self.is_plugin(PluginsTarget::LocalModelPicker))
                .paint(p, picker);
            self.plugin_hit(hits, picker, visible, PluginsTarget::LocalModelPicker);
            at += metrics::ROW_HEIGHT + metrics::ROW_GAP;
            if self.session.plugins.panel.local_picker {
                for (index, (_, label, installed)) in choices.iter().enumerate() {
                    let rect = Rect::new(x, at, inner, metrics::ROW_HEIGHT);
                    Row::new(label)
                        .trailing(if installed.is_some() { Trailing::Check } else { Trailing::None })
                        .hovered(self.is_plugin(PluginsTarget::LocalModelChoice(index)))
                        .paint(p, rect);
                    self.plugin_hit(hits, rect, visible, PluginsTarget::LocalModelChoice(index));
                    at += metrics::ROW_HEIGHT + metrics::ROW_GAP;
                }
            }
            for (id, state) in downloads {
                let said = match state {
                    DownloadState::Started => strings.t(Text::AiLocalDownloading),
                    DownloadState::Preparing => strings.t(Text::AiLocalPreparing),
                    DownloadState::Fetching { done, total: Some(total) } => {
                        strings.t(Text::AiLocalFetching(*done, *total))
                    }
                    DownloadState::Fetching { done, total: None } => strings.t(Text::AiLocalFetched(*done)),
                    DownloadState::Failed(reason) => reason.clone(),
                    DownloadState::Done => continue,
                };
                let label = format!("{id} · {said}");
                p.label_fit(&label, typography::SUBTITLE, x, at + line * 0.5, inner, c.text_secondary);
                at += line;
            }
            p.label(&strings.t(Text::AiLocalModelId), typography::CAPTION, x, at + 9.0, c.text_subtle);
            at += 22.0;
            let field = Rect::new(x, at, inner, metrics::ROW_HEIGHT);
            TextField {
                edit: &self.session.plugins.local_model,
                placeholder: crate::state::LOCAL_SUGGESTIONS[0].0,
                focused: self.session.plugins.panel.local_focus == 0,
                caret_visible: self.caret_visible,
                multiline: false,
                secret: false,
            }
            .paint(p, field);
            self.plugin_hit(hits, field, visible, PluginsTarget::LocalModelField);
            at += metrics::ROW_HEIGHT + metrics::ROW_GAP;
            let label = strings.t(Text::AiLocalDownload);
            let enabled = self.session.local_model_downloadable();
            let button =
                Button { label: &label, primary: true, hovered: self.is_plugin(PluginsTarget::DownloadModel), enabled };
            let rect = Rect::new(x, at, button.width(p), metrics::CHIP_HEIGHT);
            if self.session.plugins.panel.local_focus == 1 {
                Button::focus_ring(p, rect);
            }
            button.paint(p, rect);
            if enabled {
                self.plugin_hit(hits, rect, visible, PluginsTarget::DownloadModel);
            }
        }
        card.h + metrics::ROW_GAP
    }
}
