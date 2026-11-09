//! Side effects requested by the reducer and executed by the app shell.

use std::path::PathBuf;

use superfik_core::SessionRecord;
use superfik_protocol::ClientCommand;
use superfik_sound::Cue;

#[derive(Debug, Clone)]
pub enum Effect {
    LoadBrowserProfiles,
    SaveBrowserProfiles {
        book: Box<superfik_core::browser_profiles::ProfileBook>,
        credential: Option<superfik_browser::profiles::SecretChange>,
    },
    Command(Box<ClientCommand>),
    PickWorkspace,
    PickReferences,
    Upload(Vec<PathBuf>),
    PickRepository,
    Persist,
    KeepSession {
        path: PathBuf,
        project: Option<superfik_core::project::ProjectRef>,
        session: Box<SessionRecord>,
    },
    Sound(Cue),
    OpenUrl(String),
    PickPluginFolder,
    ImportLogins(superfik_browser::import::BrowserKind),
    ClearCookies,
    CopyText(String),
    Unlock,
    Update(super::UpdateCall),
    JoinServer(super::JoinRequest),
    LeaveServer,
    Browser(superfik_browser::Call),
    SaveLogin {
        browser: superfik_core::BrowserId,
        account: String,
    },
    Voice(superfik_voice::VoiceCall),
}

impl Effect {
    pub fn is_dialog(&self) -> bool {
        matches!(self, Self::PickWorkspace | Self::PickReferences | Self::PickRepository | Self::PickPluginFolder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pickers_are_dialogs() {
        assert!(Effect::PickReferences.is_dialog());
        assert!(Effect::PickPluginFolder.is_dialog());
        assert!(!Effect::Persist.is_dialog());
    }
}
