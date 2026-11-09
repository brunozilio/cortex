//! How the agent talks while it works, as the user picks it in Settings.
//! The engine holds each style's text; this is only the choice.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputStyle {
    #[default]
    Default,
    Concise,
    Proactive,
    Explanatory,
    Learning,
}

impl OutputStyle {
    pub const ALL: [Self; 5] = [Self::Default, Self::Concise, Self::Proactive, Self::Explanatory, Self::Learning];

    pub const fn id(self) -> Option<&'static str> {
        match self {
            Self::Default => None,
            Self::Concise => Some("concise"),
            Self::Proactive => Some("proactive"),
            Self::Explanatory => Some("explanatory"),
            Self::Learning => Some("learning"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_chosen_style_is_named() {
        assert_eq!(OutputStyle::Default.id(), None);
        assert_eq!(OutputStyle::Learning.id(), Some("learning"));
        assert_eq!(serde_json::to_string(&OutputStyle::Concise).expect("json"), "\"concise\"");
    }
}
