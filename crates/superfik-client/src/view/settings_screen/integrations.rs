//! The Integrations and Vaults panes: shelves of plugins, each a card. A
//! card's header shows the plugin (mark, name, one line); an installed one
//! expands to its accounts, the sheet that adds one and the button that
//! removes the plugin. Heights are computed from the same items that are
//! painted, so a card is drawn once, at the size it needs.

use superfik_core::PluginSummary;
use superfik_i18n::{Strings, Text};
use superfik_render::{Fill, Point, Quad, Rect};
use superfik_ui::brands::brand;
use superfik_ui::components::{BrandTile, Button};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

use super::SettingsScreenView;
use super::card_body::items;
use crate::state::{SettingsPane, Shelf};
use crate::view::target::{HitMap, PluginsTarget, SettingsTarget, Target};

const SHELF_GAP: f32 = 24.0;
const TILE: f32 = 36.0;
const BODY_TOP: f32 = 14.0;

fn shelf_title(strings: &Strings, shelf: Shelf) -> String {
    strings.t(match shelf {
        Shelf::Installed => Text::StoreInstalled,
        Shelf::Category("development") => Text::CategoryDevelopment,
        Shelf::Category("productivity") => Text::CategoryProductivity,
        Shelf::Category("data") => Text::CategoryData,
        Shelf::Category("web") => Text::CategoryWeb,
        Shelf::Category(_) => Text::CategoryOther,
    })
}

impl SettingsScreenView<'_> {
    pub(super) fn is_plugin(&self, target: PluginsTarget) -> bool {
        self.is(SettingsTarget::Plugin(target))
    }

    fn focused_button(&self, target: PluginsTarget) -> bool {
        let Some(sheet) = &self.session.plugins.panel.sheet else { return false };
        match target {
            PluginsTarget::Connect => sheet.focus == sheet.connect_slot(),
            PluginsTarget::CancelSheet => sheet.focus == sheet.cancel_slot(),
            _ => false,
        }
    }

    pub(super) fn plugin_hit(&self, hits: &mut HitMap, rect: Rect, visible: Rect, target: PluginsTarget) {
        hits.add_screen(rect.intersect(&visible), Target::Settings(SettingsTarget::Plugin(target)));
    }

    #[allow(clippy::too_many_arguments, reason = "a view helper: everything it paints from, nothing stored")]
    pub(super) fn plugin_button(
        &self,
        p: &mut Painter,
        label: &str,
        primary: bool,
        target: PluginsTarget,
        at: Point,
        visible: Rect,
        hits: &mut HitMap,
    ) -> Rect {
        let b = Button { label, primary, hovered: self.is_plugin(target), enabled: true };
        let rect = Rect::new(at.x, at.y, b.width(p), metrics::CHIP_HEIGHT);
        if self.focused_button(target) {
            Button::focus_ring(p, rect);
        }
        let rect = b.paint(p, rect);
        self.plugin_hit(hits, rect, visible, target);
        rect
    }

    pub(super) fn paint_plugins(
        &self,
        p: &mut Painter,
        column: Rect,
        mut y: f32,
        visible: Rect,
        hits: &mut HitMap,
    ) -> f32 {
        let strings = self.session.strings();
        if self.session.settings.pane == SettingsPane::Ai {
            return self.paint_ai(p, column, y, visible, hits);
        }
        let vaults = self.session.settings.pane == SettingsPane::Vaults;
        for (shelf, plugins) in self.session.plugins.shelves(vaults) {
            let c = p.palette();
            p.label(
                &shelf_title(&strings, shelf),
                typography::HEADER,
                column.x + 2.0,
                y + metrics::FORM_HEADING * 0.4,
                c.text_secondary,
            );
            y += metrics::FORM_HEADING;
            for plugin in plugins {
                y += self.paint_card(p, column, y, plugin, visible, hits) + metrics::ROW_GAP;
            }
            y += SHELF_GAP - metrics::ROW_GAP;
        }
        if !vaults {
            let label = strings.t(Text::AddPluginFolder);
            let rect =
                self.plugin_button(p, &label, false, PluginsTarget::AddFolder, Point::new(column.x, y), visible, hits);
            y = rect.bottom() + SHELF_GAP;
        }
        y
    }

    fn paint_card(
        &self,
        p: &mut Painter,
        column: Rect,
        y: f32,
        plugin: &PluginSummary,
        visible: Rect,
        hits: &mut HitMap,
    ) -> f32 {
        let strings = self.session.strings();
        let c = p.palette();
        let pad = metrics::FORM_ROW_PADDING;
        let items = items(self, plugin);
        let body: f32 = items.iter().map(|item| item.height(plugin)).sum();
        let head = metrics::FORM_ROW_DETAILED;
        let height = head + if items.is_empty() { 0.0 } else { BODY_TOP + body + pad };
        let card = Rect::new(column.x, y, column.w, height);
        p.quad(
            Quad::new(card, metrics::FORM_CARD_RADIUS, c.surface_top)
                .fill(Fill { top: c.surface_top, bottom: c.surface_bottom })
                .border(c.divider, 1.0),
        );
        let Some(index) = self.session.plugins.list.iter().position(|listed| listed.id == plugin.id) else {
            return height;
        };
        let header = Rect::new(card.x, y, card.w, head);
        let expanded = !items.is_empty();
        let target = match (plugin.installed, expanded) {
            (false, _) => PluginsTarget::Get(index),
            (true, false) => PluginsTarget::Open(index),
            (true, true) => PluginsTarget::Fold,
        };
        if plugin.installed {
            if self.is_plugin(target) {
                p.quad(Quad::new(header, metrics::FORM_CARD_RADIUS, c.hover));
            }
            self.plugin_hit(hits, header, visible, target);
        }
        let tile = Rect::new(header.x + pad, header.center().y - TILE * 0.5, TILE, TILE);
        BrandTile { brand: brand(&plugin.icon), name: &plugin.name }.paint(p, tile);
        let mut right = header.right() - pad;
        if plugin.installed {
            let icon = if expanded { icons::CARET_UP } else { icons::CHEVRON_DOWN };
            p.icon_at(
                icon,
                Point::new(right - metrics::ICON_LARGE * 0.5, header.center().y),
                metrics::ICON_LARGE,
                Some(c.icon_muted),
            );
            right -= metrics::ICON_LARGE + pad;
        } else {
            let label = strings.t(if plugin.is_ai_provider() { Text::Connect } else { Text::InstallPlugin });
            let b = Button { label: &label, primary: false, hovered: self.is_plugin(target), enabled: true };
            let w = b.width(p);
            let at = Point::new(right - w, header.center().y - metrics::CHIP_HEIGHT * 0.5);
            let rect = b.paint(p, Rect::new(at.x, at.y, w, metrics::CHIP_HEIGHT));
            self.plugin_hit(hits, rect, visible, target);
            right = rect.x - pad;
        }
        let x = tile.right() + metrics::FORM_ICON_GAP;
        let width = (right - x).max(0.0);
        p.label_fit(&plugin.name, typography::MENU, x, header.center().y - 9.0, width, c.text);
        p.label_fit(&plugin.description, typography::SUBTITLE, x, header.center().y + 9.0, width, c.text_secondary);
        if expanded {
            let line = Rect::new(card.x + pad, y + head, card.w - pad * 2.0, 1.0);
            p.quad(Quad::new(line, 0.0, c.divider));
        }
        let mut at = Point::new(card.x + pad, y + head + BODY_TOP);
        let inner = card.w - pad * 2.0;
        for item in &items {
            self.paint_item(p, item, plugin, at, inner, visible, hits);
            at.y += item.height(plugin);
        }
        height
    }
}
