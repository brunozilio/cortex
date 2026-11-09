//! Where pulses go: the trackpad, nowhere, or a test recorder.

use std::cell::RefCell;
use std::rc::Rc;

use crate::Haptic;

pub trait Actuator: std::fmt::Debug {
    fn perform(&self, haptic: Haptic);
}

#[derive(Debug, Default)]
pub struct Trackpad {
    #[cfg(target_os = "macos")]
    multitouch: Option<crate::multitouch::Multitouch>,
}

impl Trackpad {
    pub fn open(burst: f64) -> Self {
        #[cfg(target_os = "macos")]
        {
            let multitouch = crate::multitouch::Multitouch::open(burst);
            tracing::debug!(firm = multitouch.is_some(), "trackpad haptics");
            Self { multitouch }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = burst;
            Self {}
        }
    }

    pub fn strike(&self, strike: crate::Strike) -> bool {
        #[cfg(target_os = "macos")]
        if let Some(multitouch) = &self.multitouch {
            multitouch.strike(strike);
            return true;
        }
        let _ = strike;
        false
    }
}

#[cfg(target_os = "macos")]
impl Actuator for Trackpad {
    fn perform(&self, haptic: Haptic) {
        use objc2_app_kit::{
            NSHapticFeedbackManager, NSHapticFeedbackPattern, NSHapticFeedbackPerformanceTime,
            NSHapticFeedbackPerformer,
        };

        use crate::Pattern;
        if self.strike(haptic.strike()) {
            return;
        }
        let pattern = match haptic.pattern() {
            Pattern::Generic => NSHapticFeedbackPattern::Generic,
            Pattern::Alignment => NSHapticFeedbackPattern::Alignment,
            Pattern::LevelChange => NSHapticFeedbackPattern::LevelChange,
        };
        NSHapticFeedbackManager::defaultPerformer()
            .performFeedbackPattern_performanceTime(pattern, NSHapticFeedbackPerformanceTime::Now);
    }
}

#[cfg(not(target_os = "macos"))]
impl Actuator for Trackpad {
    fn perform(&self, _haptic: Haptic) {}
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Still;

impl Actuator for Still {
    fn perform(&self, _haptic: Haptic) {}
}

#[derive(Debug, Default, Clone)]
pub struct Recorder {
    performed: Rc<RefCell<Vec<Haptic>>>,
}

impl Recorder {
    pub fn performed(&self) -> Vec<Haptic> {
        self.performed.borrow().clone()
    }
}

impl Actuator for Recorder {
    fn perform(&self, haptic: Haptic) {
        self.performed.borrow_mut().push(haptic);
    }
}
