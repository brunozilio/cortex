//! A channel account's link under it on its plugin card: where it stands,
//! the QR code to scan while pairing, and the webhook to give the provider
//! once an official account is linked.

use superfik_core::ConnectionId;
use superfik_core::channel::LinkStatus;
use superfik_i18n::Text;
use superfik_render::{Point, Rect};
use superfik_ui::components::{Chip, QrCode};
use superfik_ui::theme::{metrics, typography};
use superfik_ui::{Painter, icons};

use super::SettingsScreenView;
use super::card_body::LINE;
use crate::view::target::{HitMap, PluginsTarget};

pub(super) fn paint_link(
    p: &mut Painter,
    strings: &superfik_i18n::Strings,
    status: Option<&LinkStatus>,
    at: Point,
    width: f32,
) {
    let c = p.palette();
    let gutter = metrics::SKILL_ICON + metrics::SKILL_ICON_GAP;
    let x = at.x + gutter;
    let line = match status {
        None | Some(LinkStatus::Opening) => strings.t(Text::ChannelOpening),
        Some(LinkStatus::Pairing { .. }) => strings.t(Text::ChannelPairing),
        Some(LinkStatus::Linked { account, .. }) => {
            let shown =
                if account.bytes().all(|b| b.is_ascii_digit()) { format!("+{account}") } else { account.clone() };
            strings.t(Text::ChannelLinked(&shown))
        }
        Some(LinkStatus::LoggedOut) => strings.t(Text::ChannelLoggedOut),
        Some(LinkStatus::Failed { reason }) => strings.t(Text::ChannelFailed(reason)),
    };
    let color = match status {
        Some(LinkStatus::Linked { .. }) => c.text_label,
        Some(LinkStatus::LoggedOut | LinkStatus::Failed { .. }) => c.danger,
        _ => c.text_subtle,
    };
    let layout = p.layout(&line, typography::STORE_SUBTITLE, Some(width - gutter));
    p.text(&layout, Point::new(x, at.y), color);
    if let Some(LinkStatus::Pairing { qr }) = status
        && let Ok(code) = qrcode::QrCode::new(qr.as_bytes())
    {
        let modules: Vec<bool> = code.to_colors().iter().map(|m| *m == qrcode::Color::Dark).collect();
        let tile = Rect::new(x, at.y + LINE * 2.0, metrics::QR_SIDE, metrics::QR_SIDE);
        QrCode { modules: &modules, width: code.width() }.paint(p, tile);
    }
}

impl SettingsScreenView<'_> {
    #[allow(clippy::too_many_arguments, reason = "a view helper: everything it paints from, nothing stored")]
    pub(super) fn paint_hook(
        &self,
        p: &mut Painter,
        connection: ConnectionId,
        hook: &str,
        at: Point,
        width: f32,
        visible: Rect,
        hits: &mut HitMap,
    ) {
        let (strings, c) = (self.session.strings(), p.palette());
        let target = PluginsTarget::CopyHook(connection);
        let label = strings.t(Text::CopyWebhook);
        let chip =
            Chip { icon: icons::BOLT, label: &label, active: false, hovered: self.is_plugin(target), tint: None };
        let chip_w = chip.width(p);
        let rect = chip.paint(p, Point::new(at.x + width - chip_w, at.y + metrics::ROW_GAP));
        self.plugin_hit(hits, rect, visible, target);
        let gutter = metrics::SKILL_ICON + metrics::SKILL_ICON_GAP;
        let line = strings.t(Text::ChannelWebhook(hook));
        let room = width - gutter - chip_w - metrics::ROW_GAP;
        let y = at.y + metrics::ROW_GAP + (metrics::CHIP_HEIGHT - LINE) * 0.5;
        p.label_fit(&line, typography::STORE_SUBTITLE, at.x + gutter, y + LINE * 0.5, room, c.text_subtle);
    }
}
