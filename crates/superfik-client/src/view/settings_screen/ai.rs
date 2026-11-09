//! AI accounts grouped by brand, with a choice of subscription or API key.

use superfik_core::AuthKind;
use superfik_i18n::Text;
use superfik_render::{Fill, Point, Quad, Rect};
use superfik_ui::brands::brand;
use superfik_ui::components::BrandTile;
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

use super::SettingsScreenView;
use super::card_body::{Item, items};
use crate::view::target::{HitMap, PluginsTarget};

impl SettingsScreenView<'_> {
    pub(super) fn paint_ai(&self, p: &mut Painter, column: Rect, mut y: f32, visible: Rect, hits: &mut HitMap) -> f32 {
        let strings = self.session.strings();
        let c = p.palette();
        let pad = metrics::FORM_ROW_PADDING;
        let head = metrics::FORM_ROW_DETAILED;
        for group in self.session.plugins.ai_groups() {
            let expanded =
                group.plugins.iter().any(|plugin| self.session.plugins.panel.selected.as_ref() == Some(&plugin.id));
            let mut accounts = Vec::new();
            let mut details = Vec::new();
            if expanded {
                for plugin in &group.plugins {
                    for connection in self.session.plugins.connections_of(&plugin.id) {
                        accounts.push((*plugin, Item::Account(connection)));
                    }
                    details.extend(
                        items(self, plugin)
                            .into_iter()
                            .filter(|item| matches!(item, Item::Sheet(_) | Item::Pending))
                            .map(|item| (*plugin, item)),
                    );
                }
            }
            let choosing = expanded
                && (accounts.is_empty()
                    || self.session.plugins.panel.ai_adding
                    || self.session.plugins.panel.sheet.is_some());
            let footer = expanded && !choosing && self.session.plugins.panel.pending.is_none();
            let choice_height = if choosing { metrics::CHIP_HEIGHT + metrics::ROW_GAP } else { 0.0 };
            let body_height: f32 = accounts.iter().chain(&details).map(|(plugin, item)| item.height(plugin)).sum();
            let footer_height =
                if footer { Item::Actions { add: true, remove: false }.height(group.plugins[0]) } else { 0.0 };
            let height = head + if expanded { 14.0 + choice_height + body_height + footer_height + pad } else { 0.0 };
            let card = Rect::new(column.x, y, column.w, height);
            p.quad(
                Quad::new(card, metrics::FORM_CARD_RADIUS, c.surface_top)
                    .fill(Fill { top: c.surface_top, bottom: c.surface_bottom })
                    .border(c.divider, 1.0),
            );
            let header = Rect::new(card.x, y, card.w, head);
            let first = group.plugins[0];
            let Some(index) = self.session.plugins.list.iter().position(|plugin| plugin.id == first.id) else {
                continue;
            };
            let target = if expanded { PluginsTarget::Fold } else { PluginsTarget::Open(index) };
            if self.is_plugin(target) {
                p.quad(Quad::new(header, metrics::FORM_CARD_RADIUS, c.hover));
            }
            self.plugin_hit(hits, header, visible, target);
            let tile = Rect::new(card.x + pad, header.center().y - 18.0, 36.0, 36.0);
            BrandTile { brand: brand(group.icon), name: group.name }.paint(p, tile);
            let x = tile.right() + metrics::FORM_ICON_GAP;
            let width = (card.right() - pad * 2.0 - metrics::ICON_LARGE - x).max(0.0);
            p.label_fit(group.name, typography::MENU, x, header.center().y - 9.0, width, c.text);
            p.label_fit(
                &strings.t(Text::AiConnectionMethod),
                typography::SUBTITLE,
                x,
                header.center().y + 9.0,
                width,
                c.text_secondary,
            );
            p.icon_at(
                if expanded { icons::CARET_UP } else { icons::CHEVRON_DOWN },
                Point::new(card.right() - pad - metrics::ICON_LARGE * 0.5, header.center().y),
                metrics::ICON_LARGE,
                Some(c.icon_muted),
            );
            if expanded {
                p.quad(Quad::new(Rect::new(card.x + pad, y + head, card.w - pad * 2.0, 1.0), 0.0, c.divider));
                let mut at = Point::new(card.x + pad, y + head + 14.0);
                for (plugin, item) in &accounts {
                    self.paint_item(p, item, plugin, at, card.w - pad * 2.0, visible, hits);
                    at.y += item.height(plugin);
                }
                let mut button_x = at.x;
                for plugin in group.plugins.iter().filter(|_| choosing) {
                    let Some(index) = self.session.plugins.list.iter().position(|listed| listed.id == plugin.id) else {
                        continue;
                    };
                    let label =
                        strings.t(if plugin.auth == AuthKind::OAuth { Text::AiSubscription } else { Text::AiApiKey });
                    let active = self.session.plugins.panel.selected.as_ref() == Some(&plugin.id)
                        && self.session.plugins.panel.sheet.is_some();
                    let rect = self.plugin_button(
                        p,
                        &label,
                        active,
                        PluginsTarget::AiConnect(index),
                        Point::new(button_x, at.y),
                        visible,
                        hits,
                    );
                    button_x = rect.right() + 8.0;
                }
                at.y += choice_height;
                for (plugin, item) in details {
                    self.paint_item(p, &item, plugin, at, card.w - pad * 2.0, visible, hits);
                    at.y += item.height(plugin);
                }
                if footer {
                    self.paint_item(
                        p,
                        &Item::Actions { add: true, remove: false },
                        group.plugins[0],
                        at,
                        card.w - pad * 2.0,
                        visible,
                        hits,
                    );
                }
            }
            y += height + metrics::ROW_GAP;
        }
        y += self.paint_local_models(p, column, y, visible, hits);
        y + 24.0
    }
}
