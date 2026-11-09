//! What the reducer's effects come to in a browser tab (ADR 0010: the
//! client only shows and asks). Commands, kept sessions and settings go to
//! the server; links open in a new tab; copied text goes to the
//! clipboard; picking references opens the browser's file picker (the
//! picked files are uploaded by `WebClient`, which holds their bytes).
//! What a tab cannot do (other dialogs, sounds, other browsers' logins,
//! pages driven here) the platform never asks for, and is dropped. Pure.

use superfik_client::state::{Action, Effect, Session};
use superfik_client::user_settings::Settings;
use superfik_protocol::ClientCommand;

#[derive(Debug)]
pub enum Io {
    Send(Box<ClientCommand>),
    Paste(fn(String) -> Action),
    Copy(String),
    Open(String),
    PickFiles,
    PaintPage(crate::client::PagePaint),
}

pub fn io(effects: Vec<Effect>, session: &Session) -> Vec<Io> {
    effects
        .into_iter()
        .filter_map(|effect| match effect {
            Effect::Command(command) => Some(Io::Send(command)),
            Effect::KeepSession { path, project, session } => {
                Some(Io::Send(Box::new(ClientCommand::SaveSession { path, project, session })))
            }
            Effect::Persist => {
                Some(Io::Send(Box::new(ClientCommand::SaveSettings { settings: Settings::of(session).to_value() })))
            }
            Effect::OpenUrl(url) => Some(Io::Open(url)),
            Effect::CopyText(text) => Some(Io::Copy(text)),
            Effect::PickReferences => Some(Io::PickFiles),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use superfik_client::state::{Preferences, Theme};

    use super::*;

    #[test]
    fn settings_and_commands_go_to_the_server() {
        let session =
            Session { preferences: Preferences { theme: Theme::Dark, ..Preferences::default() }, ..Session::default() };
        let effects = vec![
            Effect::Persist,
            Effect::Command(Box::new(ClientCommand::LoadNotes { path: "/repo".into() })),
            Effect::OpenUrl("https://superfik.com".into()),
            Effect::PickWorkspace,
        ];
        let io = io(effects, &session);
        let Io::Send(save) = &io[0] else { unreachable!("settings first") };
        let ClientCommand::SaveSettings { settings } = save.as_ref() else { unreachable!("settings first") };
        assert_eq!(Settings::from_value(settings.clone()).preferences.theme, Theme::Dark);
        assert!(matches!(&io[1], Io::Send(load) if matches!(load.as_ref(), ClientCommand::LoadNotes { .. })));
        assert!(matches!(&io[2], Io::Open(url) if url == "https://superfik.com"));
        assert_eq!(io.len(), 3, "a tab has no folder dialog");
    }
}
