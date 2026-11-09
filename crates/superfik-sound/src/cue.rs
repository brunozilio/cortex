//! What the interface wants to say with a sound.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cue {
    Button,
    Select,
    Tap,
    Type,
    Swipe,
    ToggleOn,
    ToggleOff,
    TransitionUp,
    TransitionDown,
    Notification,
    Caution,
    Disabled,
    Celebration,
}

impl Cue {
    pub const ALL: [Cue; 13] = [
        Cue::Button,
        Cue::Select,
        Cue::Tap,
        Cue::Type,
        Cue::Swipe,
        Cue::ToggleOn,
        Cue::ToggleOff,
        Cue::TransitionUp,
        Cue::TransitionDown,
        Cue::Notification,
        Cue::Caution,
        Cue::Disabled,
        Cue::Celebration,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Cue::Button => "button",
            Cue::Select => "select",
            Cue::Tap => "tap",
            Cue::Type => "type",
            Cue::Swipe => "swipe",
            Cue::ToggleOn => "toggle_on",
            Cue::ToggleOff => "toggle_off",
            Cue::TransitionUp => "transition_up",
            Cue::TransitionDown => "transition_down",
            Cue::Notification => "notification",
            Cue::Caution => "caution",
            Cue::Disabled => "disabled",
            Cue::Celebration => "celebration",
        }
    }

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}
