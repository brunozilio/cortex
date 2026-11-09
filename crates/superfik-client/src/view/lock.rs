//! The lock screen over the whole window while the app is locked
//! (`LockState::covers`): Unlock with Touch ID, why the last try failed,
//! and every pixel a screen hit so nothing underneath answers. Painted in
//! window coordinates, last in the overlay pass, above Get Started.

use superfik_i18n::Text;
use superfik_render::Rect;
use superfik_ui::Painter;
use superfik_ui::components::{Button, Emote, SignIn};
use superfik_ui::emote::EmoteState;

use super::target::{HitMap, LockTarget, Target};
use crate::state::{LockProblem, LockStatus, Session};

pub struct LockView<'a> {
    pub session: &'a Session,
    pub hovered: Option<Target>,
    pub viewport: (f32, f32),
    pub now: f64,
}

fn mood(status: LockStatus, failed: bool) -> EmoteState {
    match status {
        LockStatus::Unlocking => EmoteState::Thinking,
        _ if failed => EmoteState::Alert,
        _ => EmoteState::Idle,
    }
}

impl LockView<'_> {
    pub fn paint(&self, p: &mut Painter, hits: &mut HitMap) {
        let lock = &self.session.lock;
        if !lock.covers() {
            return;
        }
        let strings = self.session.strings();
        let area = Rect::new(0.0, 0.0, self.viewport.0, self.viewport.1);
        hits.add_screen(area, Target::Lock(LockTarget::Surface));
        let method = self.session.platform.system_unlock.map(|method| method.name()).unwrap_or("Touch ID");
        let (title, caption, unlock) =
            (strings.t(Text::LockTitle), strings.t(Text::LockCaption), strings.t(Text::UnlockWith(method)));
        let error = lock.error.as_ref().map(|problem| {
            strings.t(match problem {
                LockProblem::Cancelled => Text::LockCancelled,
                LockProblem::Unavailable => Text::LockUnavailable,
            })
        });
        let enabled = lock.status == LockStatus::Locked;
        let screen = SignIn {
            emote: Emote { state: mood(lock.status, error.is_some()), from: None, time: self.now as f32, width: 0.0 },
            title: &title,
            caption: &caption,
            error: error.as_deref(),
            primary: Button {
                label: &unlock,
                primary: true,
                hovered: enabled && self.hovered == Some(Target::Lock(LockTarget::Unlock)),
                enabled,
            },
            secondary: None,
        };
        let drawn = screen.paint(p, area);
        if enabled {
            hits.add_screen(drawn.primary, Target::Lock(LockTarget::Unlock));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_emote_thinks_while_asking_and_alerts_after_a_failure() {
        assert_eq!(mood(LockStatus::Locked, false), EmoteState::Idle);
        assert_eq!(mood(LockStatus::Unlocking, true), EmoteState::Thinking);
        assert_eq!(mood(LockStatus::Locked, true), EmoteState::Alert);
    }
}
