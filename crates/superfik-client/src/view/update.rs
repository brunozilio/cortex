//! The update screen, over everything while it covers the window: "Update
//! required" (or available) with its button, "Updating Superfik" with the
//! title swept by light, the bar and its stage, then "Update ready" with
//! Restart. The emote stands where a logo would.

use superfik_i18n::Text;
use superfik_render::Rect;
use superfik_ui::Painter;
use superfik_ui::components::{BarProgress, Button, Emote, UpdateScreen};
use superfik_ui::emote::EmoteState;

use super::target::{HitMap, Target, UpdateTarget};
use crate::state::{Session, UpdatePhase, UpdateStage};

pub struct UpdateView<'a> {
    pub session: &'a Session,
    pub hovered: Option<Target>,
    pub viewport: (f32, f32),
    pub now: f64,
}

pub struct Look {
    pub title: Text<'static>,
    pub caption: Text<'static>,
    pub shimmer: bool,
    pub progress: Option<BarProgress>,
    pub status: Option<Text<'static>>,
    pub button: Option<Text<'static>>,
    pub mood: EmoteState,
}

pub fn look(phase: &UpdatePhase, required: bool, time: f32) -> Look {
    let busy = |progress, status| Look {
        title: Text::UpdatingTitle,
        caption: Text::UpdatingCaption,
        shimmer: true,
        progress: Some(progress),
        status,
        button: None,
        mood: EmoteState::Thinking,
    };
    let unknown = BarProgress::Unknown { time };
    match phase {
        UpdatePhase::Preparing(UpdateStage::Checking) => busy(unknown, Some(Text::UpdateChecking)),
        UpdatePhase::Preparing(UpdateStage::Starting) => busy(unknown, Some(Text::UpdateStarting)),
        UpdatePhase::Preparing(UpdateStage::Downloading(done)) => {
            busy(BarProgress::Known(*done), Some(Text::UpdatePercent((done.clamp(0.0, 1.0) * 100.0) as u8)))
        }
        UpdatePhase::Preparing(UpdateStage::Verifying) => busy(unknown, Some(Text::UpdateVerifying)),
        UpdatePhase::Preparing(UpdateStage::Installing) => busy(unknown, Some(Text::UpdateInstalling)),
        UpdatePhase::Ready => Look {
            title: Text::UpdateReadyTitle,
            caption: Text::UpdateReadyCaption,
            shimmer: false,
            progress: Some(BarProgress::Known(1.0)),
            status: None,
            button: Some(Text::UpdateRestart),
            mood: EmoteState::Done,
        },
        UpdatePhase::Failed(_) => Look {
            title: Text::UpdateFailedTitle,
            caption: Text::UpdatingCaption,
            shimmer: false,
            progress: None,
            status: None,
            button: Some(Text::UpdateRetry),
            mood: EmoteState::Alert,
        },
        UpdatePhase::Idle | UpdatePhase::Available => Look {
            title: if required { Text::UpdateRequiredTitle } else { Text::UpdateAvailableTitle },
            caption: Text::UpdatingCaption,
            shimmer: false,
            progress: None,
            status: None,
            button: Some(Text::UpdateButton),
            mood: if required { EmoteState::Alert } else { EmoteState::Idle },
        },
    }
}

impl UpdateView<'_> {
    pub fn paint(&self, p: &mut Painter, hits: &mut HitMap) {
        let update = &self.session.update;
        let Some(offer) = update.offer.as_ref().filter(|_| update.covers()) else { return };
        let strings = self.session.strings();
        let area = Rect::new(0.0, 0.0, self.viewport.0, self.viewport.1);
        hits.add_screen(area, Target::Update(UpdateTarget::Surface));
        let time = self.now as f32;
        let look = look(&update.phase, update.required(), time);
        let current = env!("CARGO_PKG_VERSION");
        let caption = match (&update.phase, update.required()) {
            (UpdatePhase::Idle | UpdatePhase::Available, true) => strings.t(Text::UpdateRequiredCaption(current)),
            (UpdatePhase::Idle | UpdatePhase::Available, false) => {
                strings.t(Text::UpdateAvailableCaption(&offer.version))
            }
            (UpdatePhase::Failed(why), _) => why.clone(),
            _ => strings.t(look.caption),
        };
        let status = match update.phase {
            UpdatePhase::Ready => Some(strings.t(Text::UpdateVersion(&offer.version))),
            _ => look.status.map(|text| strings.t(text)),
        };
        let (title, button) = (strings.t(look.title), look.button.map(|text| strings.t(text)));
        let later = (!update.required()).then(|| strings.t(Text::NotNow));
        let hovered = |target| self.hovered == Some(Target::Update(target));
        let screen = UpdateScreen {
            emote: Emote { state: look.mood, from: None, time, width: 0.0 },
            title: &title,
            shimmer: look.shimmer.then_some(time),
            caption: &caption,
            progress: look.progress,
            status: status.as_deref(),
            primary: button.as_deref().map(|label| Button {
                label,
                primary: true,
                hovered: hovered(UpdateTarget::Primary),
                enabled: true,
            }),
            secondary: later.as_deref().map(|label| Button {
                label,
                primary: false,
                hovered: hovered(UpdateTarget::Close),
                enabled: true,
            }),
        };
        let drawn = screen.paint(p, area);
        if let Some(rect) = drawn.primary {
            hits.add_screen(rect, Target::Update(UpdateTarget::Primary));
        }
        if let Some(rect) = drawn.secondary {
            hits.add_screen(rect, Target::Update(UpdateTarget::Close));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_phase_has_its_look() {
        let required = look(&UpdatePhase::Available, true, 0.0);
        assert_eq!(
            (required.title, required.button, required.mood),
            (Text::UpdateRequiredTitle, Some(Text::UpdateButton), EmoteState::Alert)
        );
        let downloading = look(&UpdatePhase::Preparing(UpdateStage::Downloading(0.52)), true, 0.0);
        assert!(downloading.shimmer && downloading.button.is_none());
        assert_eq!(downloading.status, Some(Text::UpdatePercent(52)));
        assert_eq!(downloading.progress, Some(BarProgress::Known(0.52)));
        let installing = look(&UpdatePhase::Preparing(UpdateStage::Installing), false, 1.0);
        assert!(matches!(installing.progress, Some(BarProgress::Unknown { .. })));
        let ready = look(&UpdatePhase::Ready, false, 0.0);
        assert_eq!((ready.title, ready.button), (Text::UpdateReadyTitle, Some(Text::UpdateRestart)));
        assert_eq!(look(&UpdatePhase::Failed("x".into()), true, 0.0).button, Some(Text::UpdateRetry));
    }
}
