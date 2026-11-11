//! The well-known reasons a run cannot start or continue.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineProblem {
    NotLoggedIn,
    SessionNotFound,
}

impl EngineProblem {
    pub fn message(self) -> &'static str {
        match self {
            Self::NotLoggedIn => {
                "The provider refused the sign-in, or none is connected (the reason follows): \
                 run `superfik login` (or /login in the chat)."
            }
            Self::SessionNotFound => "The earlier conversation was not found; your next message starts a new one.",
        }
    }
}
