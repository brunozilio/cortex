//! What a pulse means, and how it feels.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pattern {
    Generic,
    Alignment,
    LevelChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Strike {
    pub actuation: i32,
    pub repeats: u32,
}

impl Strike {
    pub const LIGHT: Strike = Strike { actuation: 6, repeats: 1 };
    pub const MEDIUM: Strike = Strike { actuation: 6, repeats: 2 };
    pub const FIRM: Strike = Strike { actuation: 6, repeats: 3 };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Haptic {
    Lift,
    Contact,
    Align,
    Level,
    Threshold,
}

impl Haptic {
    pub const ALL: [Haptic; 5] = [Haptic::Lift, Haptic::Contact, Haptic::Align, Haptic::Level, Haptic::Threshold];

    pub fn pattern(self) -> Pattern {
        match self {
            Haptic::Lift | Haptic::Contact => Pattern::Generic,
            Haptic::Align => Pattern::Alignment,
            Haptic::Level | Haptic::Threshold => Pattern::LevelChange,
        }
    }

    pub fn strike(self) -> Strike {
        match self {
            Haptic::Contact => Strike::FIRM,
            Haptic::Lift | Haptic::Threshold => Strike::MEDIUM,
            Haptic::Align | Haptic::Level => Strike::LIGHT,
        }
    }
}
