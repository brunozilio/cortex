//! The canvas every session starts on: nodes in their slots, the camera at
//! home. Nothing is kept from one session to another, or between launches.

use super::App;
use crate::view::motions::Motions;

impl App {
    pub(super) fn switched(&mut self) {
        self.motions = Motions::default();
        self.spacing.reset();
        self.camera.reset();
    }
}
