//! Updating the app itself from the latest GitHub release's manifest. A
//! `required` update (this version is below the oldest supported one)
//! covers the window until it is installed; another downloads in the
//! background and waits for a restart. The shell does the work
//! (`Effect::Update`) and reports each stage; the reducer only decides what
//! shows.

use super::{Effect, Session};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateOffer {
    pub version: String,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpdateStage {
    Checking,
    Starting,
    Downloading(f32),
    Verifying,
    Installing,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdatePhase {
    Idle,
    Available,
    Preparing(UpdateStage),
    Ready,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateAction {
    Offered(Option<UpdateOffer>),
    Open,
    Close,
    Primary,
    Stage(UpdateStageReport),
    Ready,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStageReport {
    Checking,
    Starting,
    Downloading { per_mille: u16 },
    Verifying,
    Installing,
}

impl From<UpdateStageReport> for UpdateStage {
    fn from(report: UpdateStageReport) -> Self {
        match report {
            UpdateStageReport::Checking => Self::Checking,
            UpdateStageReport::Starting => Self::Starting,
            UpdateStageReport::Downloading { per_mille } => Self::Downloading(f32::from(per_mille.min(1000)) / 1000.0),
            UpdateStageReport::Verifying => Self::Verifying,
            UpdateStageReport::Installing => Self::Installing,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateCall {
    Prepare,
    Restart,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateState {
    pub offer: Option<UpdateOffer>,
    pub phase: UpdatePhase,
    pub open: bool,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self { offer: None, phase: UpdatePhase::Idle, open: false }
    }
}

impl UpdateState {
    pub fn covers(&self) -> bool {
        self.offer.is_some() && (self.open || self.required())
    }

    pub fn required(&self) -> bool {
        self.offer.as_ref().is_some_and(|offer| offer.required)
    }

    pub fn offered(&self) -> bool {
        self.offer.is_some() && !self.covers()
    }

    pub fn ready(&self) -> bool {
        self.phase == UpdatePhase::Ready
    }

    fn prepare(&mut self) -> Option<UpdateCall> {
        match self.phase {
            UpdatePhase::Idle | UpdatePhase::Available | UpdatePhase::Failed(_) => {
                self.phase = UpdatePhase::Preparing(UpdateStage::Starting);
                Some(UpdateCall::Prepare)
            }
            UpdatePhase::Preparing(_) | UpdatePhase::Ready => None,
        }
    }
}

impl Session {
    pub(super) fn update_action(&mut self, action: UpdateAction) -> Vec<Effect> {
        let update = &mut self.update;
        let call = match action {
            UpdateAction::Offered(None) => None,
            UpdateAction::Offered(Some(offer)) => {
                let newer = update.offer.as_ref() != Some(&offer);
                let required = offer.required;
                update.offer = Some(offer);
                if newer && update.phase != UpdatePhase::Ready {
                    update.phase = UpdatePhase::Available;
                }
                if required { None } else { update.prepare() }
            }
            UpdateAction::Open => {
                update.open = update.offer.is_some();
                None
            }
            UpdateAction::Close => {
                update.open = false;
                None
            }
            UpdateAction::Primary if update.offer.is_none() => None,
            UpdateAction::Primary if update.ready() => Some(UpdateCall::Restart),
            UpdateAction::Primary => update.prepare(),
            UpdateAction::Stage(report) => {
                if matches!(update.phase, UpdatePhase::Preparing(_)) {
                    update.phase = UpdatePhase::Preparing(report.into());
                }
                None
            }
            UpdateAction::Ready => {
                update.phase = UpdatePhase::Ready;
                None
            }
            UpdateAction::Failed(why) => {
                update.phase = UpdatePhase::Failed(why);
                None
            }
        };
        if self.update.covers() {
            self.popover = None;
        }
        call.map(Effect::Update).into_iter().collect()
    }
}

#[cfg(test)]
#[path = "tests_update.rs"]
mod tests;
