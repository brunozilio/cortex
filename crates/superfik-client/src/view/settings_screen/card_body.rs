//! What an expanded plugin card lists under its header: accounts, the
//! sheet that adds one, and a footer with "Add account" at the left and
//! "Remove plugin" at the right (vaults have none). Heights come from the
//! same items that are painted.

use superfik_core::channel::LinkStatus;
use superfik_core::{Accounts, AuthKind, Connection, ConnectionId, PluginSummary};
use superfik_i18n::Text;
use superfik_render::{Point, Rect};
use superfik_ui::components::{Chip, Row, TextField, Trailing};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

use super::SettingsScreenView;
use crate::state::ConnectSheet;
use crate::view::target::{HitMap, PluginsTarget};

pub(super) const LINE: f32 = 22.0;
const FOOTER_GAP: f32 = 8.0;

pub(super) enum Item<'a> {
    Account(&'a Connection),
    Link(ConnectionId, Option<&'a LinkStatus>),
    Pending,
    Ready,
    NoAccounts,
    Sheet(&'a ConnectSheet),
    Actions { add: bool, remove: bool },
}

impl Item<'_> {
    pub(super) fn height(&self, plugin: &PluginSummary) -> f32 {
        let button = metrics::CHIP_HEIGHT + metrics::ROW_GAP;
        match self {
            Item::Account(_) => metrics::ROW_HEIGHT + metrics::ROW_GAP,
            Item::Link(_, Some(LinkStatus::Pairing { .. })) => LINE * 2.0 + metrics::QR_SIDE + metrics::ROW_GAP * 2.0,
            Item::Link(_, Some(LinkStatus::Linked { hook: Some(_), .. })) => {
                LINE + metrics::CHIP_HEIGHT + metrics::ROW_GAP * 2.0
            }
            Item::Link(..) => LINE + metrics::ROW_GAP,
            Item::Pending | Item::Ready | Item::NoAccounts => LINE + metrics::ROW_GAP,
            Item::Sheet(_) => (1 + plugin.fields.len()) as f32 * field_height() + button,
            Item::Actions { .. } => FOOTER_GAP + metrics::CHIP_HEIGHT,
        }
    }
}

fn field_height() -> f32 {
    LINE + metrics::ROW_HEIGHT + metrics::ROW_GAP + 4.0
}

pub(super) fn items<'a>(view: &SettingsScreenView<'a>, plugin: &'a PluginSummary) -> Vec<Item<'a>> {
    let plugins = &view.session.plugins;
    if !plugin.installed || plugins.panel.selected.as_ref() != Some(&plugin.id) {
        return Vec::new();
    }
    let connections: Vec<&Connection> = plugins.connections_of(&plugin.id).collect();
    let pending = plugins.panel.pending.as_ref() == Some(&plugin.id);
    let mut items = Vec::new();
    let sheet = plugins.panel.sheet.as_ref().filter(|sheet| sheet.plugin == plugin.id);
    if !plugin.needs_account() {
        items.push(Item::Ready);
    } else if connections.is_empty() && !pending && sheet.is_none() {
        items.push(Item::NoAccounts);
    }
    for connection in connections.iter().copied().filter(|_| plugin.needs_account()) {
        items.push(Item::Account(connection));
        if plugin.channel.is_some() {
            let status = view.session.channels.accounts.get(&connection.id).map(|a| &a.status);
            items.push(Item::Link(connection.id, status));
        }
    }
    if pending {
        items.push(Item::Pending);
    }
    let can_add = plugin.needs_account() && (plugin.accounts == Accounts::Many || connections.is_empty());
    let add = sheet.is_none() && can_add && !pending;
    if let Some(sheet) = sheet {
        items.push(Item::Sheet(sheet));
    }
    let remove = sheet.is_none() && plugin.removable() && !plugin.is_ai_provider();
    if add || remove {
        items.push(Item::Actions { add, remove });
    }
    items
}

impl SettingsScreenView<'_> {
    #[allow(clippy::too_many_arguments, reason = "a view helper: everything it paints from, nothing stored")]
    pub(super) fn paint_item(
        &self,
        p: &mut Painter,
        item: &Item<'_>,
        plugin: &PluginSummary,
        at: Point,
        width: f32,
        visible: Rect,
        hits: &mut HitMap,
    ) {
        let strings = self.session.strings();
        let c = p.palette();
        match item {
            Item::Account(connection) => {
                let row = Rect::new(at.x, at.y, width, metrics::ROW_HEIGHT);
                let target = PluginsTarget::Disconnect(connection.id);
                let label = if connection.label.is_empty() { plugin.name.clone() } else { connection.label.clone() };
                let label = if plugin.is_ai_provider() {
                    let method =
                        strings.t(if plugin.auth == AuthKind::OAuth { Text::AiSubscription } else { Text::AiApiKey });
                    format!("{label} · {method}")
                } else {
                    label
                };
                Row::new(&label)
                    .icon(icons::CHECK_CIRCLE, Some(c.plugin))
                    .style(typography::SKILL)
                    .color(c.text)
                    .trailing(Trailing::Remove { hovered: self.is_plugin(target) })
                    .paint(p, row);
                self.plugin_hit(hits, Row::knob_rect(row), visible, target);
            }
            Item::Pending => {
                let text = strings.t(Text::SigningIn);
                p.label(&text, typography::STORE_SUBTITLE, at.x, at.y + LINE * 0.5, c.text_subtle);
            }
            Item::Ready => {
                let text = strings.t(Text::PluginReady);
                p.label(&text, typography::STORE_SUBTITLE, at.x, at.y + LINE * 0.5, c.text_label);
            }
            Item::NoAccounts => {
                let text = strings.t(Text::NoAccounts);
                p.label(&text, typography::STORE_SUBTITLE, at.x, at.y + LINE * 0.5, c.text_subtle);
            }
            Item::Link(connection, status) => {
                super::card_link::paint_link(p, &strings, *status, at, width);
                if let Some(LinkStatus::Linked { hook: Some(hook), .. }) = status {
                    self.paint_hook(p, *connection, hook, Point::new(at.x, at.y + LINE), width, visible, hits);
                }
            }
            Item::Sheet(sheet) => self.paint_sheet(p, plugin, sheet, at, width, visible, hits),
            Item::Actions { add, remove } => {
                let y = at.y + FOOTER_GAP;
                if *add {
                    let label = strings.t(Text::AddAccount);
                    let target = PluginsTarget::AddAccount;
                    let chip = Chip {
                        icon: icons::PLUS,
                        label: &label,
                        active: true,
                        hovered: self.is_plugin(target),
                        tint: None,
                    };
                    let rect = chip.paint(p, Point::new(at.x, y));
                    self.plugin_hit(hits, rect, visible, target);
                }
                if *remove {
                    let label = strings.t(Text::UninstallPlugin);
                    let target = PluginsTarget::Uninstall;
                    let hovered = self.is_plugin(target);
                    let tint = hovered.then_some(c.danger);
                    let chip = Chip { icon: icons::DELETE, label: &label, active: false, hovered, tint };
                    let w = chip.width(p);
                    let rect = chip.paint(p, Point::new(at.x + width - w, y));
                    self.plugin_hit(hits, rect, visible, target);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments, reason = "a view helper: everything it paints from, nothing stored")]
    fn paint_sheet(
        &self,
        p: &mut Painter,
        plugin: &PluginSummary,
        sheet: &ConnectSheet,
        at: Point,
        width: f32,
        visible: Rect,
        hits: &mut HitMap,
    ) {
        let strings = self.session.strings();
        let c = p.palette();
        let mut y = at.y;
        let name = strings.t(Text::AccountName);
        let placeholder = strings.t(Text::AccountNamePlaceholder);
        let labels = std::iter::once((name.as_str(), placeholder.as_str()))
            .chain(plugin.fields.iter().map(|f| (f.label.as_str(), f.placeholder.as_str())));
        let edits = std::iter::once(&sheet.label).chain(&sheet.fields);
        for (index, ((label, placeholder), edit)) in labels.zip(edits).enumerate() {
            p.label(label, typography::CAPTION, at.x, y + LINE * 0.4, c.text_subtle);
            y += LINE;
            let field = Rect::new(at.x, y, width, metrics::ROW_HEIGHT);
            TextField {
                edit,
                placeholder,
                focused: sheet.focus == index,
                caret_visible: self.caret_visible,
                multiline: false,
                secret: index > 0 && plugin.fields.get(index - 1).is_none_or(|field| field.key != "base_url"),
            }
            .paint(p, field);
            self.plugin_hit(hits, field, visible, PluginsTarget::Field(index));
            y += field_height() - LINE;
        }
        let confirm = strings.t(match plugin.auth {
            AuthKind::OAuth => Text::SignInWithBrowser,
            AuthKind::Cli => Text::UseCliLogin,
            AuthKind::ApiKey | AuthKind::None => Text::Connect,
            AuthKind::Qr => Text::PairWithQr,
        });
        let rect = self.plugin_button(p, &confirm, true, PluginsTarget::Connect, Point::new(at.x, y), visible, hits);
        let cancel = strings.t(Text::Cancel);
        let beside = Point::new(rect.right() + 8.0, y);
        self.plugin_button(p, &cancel, false, PluginsTarget::CancelSheet, beside, visible, hits);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::scenario_plugins::{plugin_page, plugins_connect};
    use crate::state::Session;

    fn kinds(session: &Session, id: &str) -> Vec<String> {
        let icons = HashMap::new();
        let view = SettingsScreenView {
            session,
            hovered: None,
            viewport: (800.0, 600.0),
            browser_icons: &icons,
            now: 0.0,
            caret_visible: true,
        };
        let plugin = session.plugins.list.iter().find(|p| p.id.0 == id).expect("listed");
        items(&view, plugin)
            .iter()
            .map(|item| match item {
                Item::Account(_) => "account".into(),
                Item::Link(..) => "link".into(),
                Item::Pending => "pending".into(),
                Item::Ready => "ready".into(),
                Item::NoAccounts => "no accounts".into(),
                Item::Sheet(_) => "sheet".into(),
                Item::Actions { add, remove } => format!("actions add={add} remove={remove}"),
            })
            .collect()
    }

    #[test]
    fn accounts_then_one_line_of_actions() {
        let (session, _) = plugin_page();
        assert_eq!(kinds(&session, "linear"), ["account", "account", "actions add=true remove=true"]);
    }

    #[test]
    fn the_sheet_hides_the_actions_and_the_empty_line() {
        let (mut session, _) = plugins_connect();
        assert_eq!(kinds(&session, "context7"), ["sheet"]);
        session.plugins.panel.sheet = None;
        assert_eq!(kinds(&session, "context7"), ["no accounts", "actions add=true remove=true"]);
    }
}
