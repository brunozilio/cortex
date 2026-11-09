//! Executes reducer effects: backend commands, native dialogs, settings.

use superfik_backend::{Backend, Wake};
use superfik_server::Server;

use super::server_join::Joiner;
use superfik_i18n::Text;
use superfik_protocol::{Envelope, ServerEvent};
use superfik_sound::Sounds;

use super::settings::Settings;
use super::unlock::Unlocker;
use crate::state::{Action, Effect, ServerAction, Session};

pub struct EffectRunner {
    backend: Box<dyn Backend>,
    clipboard: super::clipboard::Clipboard,
    sounds: Sounds,
    joining: Option<Joining>,
    unlocker: Option<Unlocker>,
    early: std::collections::VecDeque<ServerEvent>,
    voice: Box<dyn superfik_voice::VoiceDevice>,
    updater: Option<super::update::UpdateRunner>,
}

struct Joining {
    device: Server,
    joiner: Joiner,
    wake: Wake,
}

impl std::fmt::Debug for EffectRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectRunner").finish_non_exhaustive()
    }
}

impl EffectRunner {
    pub fn new(backend: Box<dyn Backend>, sounds: Sounds) -> Self {
        Self {
            backend,
            clipboard: super::clipboard::Clipboard::default(),
            sounds,
            joining: None,
            unlocker: None,
            early: Default::default(),
            voice: Box::new(superfik_voice::NoVoice::default()),
            updater: None,
        }
    }

    pub fn clipboard(self, clipboard: super::clipboard::Clipboard) -> Self {
        Self { clipboard, ..self }
    }

    pub fn updater(self, updater: Option<super::update::UpdateRunner>) -> Self {
        Self { updater, ..self }
    }

    pub fn poll_update(&self) -> Vec<Action> {
        self.updater.as_ref().map(super::update::UpdateRunner::poll).unwrap_or_default()
    }

    pub fn update_quitting(&self) -> bool {
        self.updater.as_ref().is_some_and(super::update::UpdateRunner::quitting)
    }

    pub fn voice(self, device: Box<dyn superfik_voice::VoiceDevice>) -> Self {
        Self { voice: device, ..self }
    }

    pub fn poll_voice(&self) -> Vec<Action> {
        let events = self.voice.poll().into_iter();
        events.map(|event| Action::Voice(crate::state::VoiceAction::Device(event))).collect()
    }

    pub fn fetch_settings(&mut self, wait: std::time::Duration) -> Option<Option<serde_json::Value>> {
        let ask = Envelope::local(superfik_protocol::ClientCommand::LoadSettings);
        self.backend.send(ask).ok()?;
        let deadline = std::time::Instant::now() + wait;
        while std::time::Instant::now() < deadline {
            match self.backend.try_recv() {
                Some(ServerEvent::Settings { settings }) => return Some(settings),
                Some(other) => self.early.push_back(other),
                None => std::thread::yield_now(),
            }
        }
        None
    }

    pub fn next_event(&mut self) -> Option<ServerEvent> {
        self.early.pop_front().or_else(|| self.backend.try_recv())
    }

    pub fn sound_prefs(&mut self, prefs: superfik_sound::SoundPrefs) {
        self.sounds.set_prefs(prefs);
    }

    pub fn unlocker(self, unlocker: Unlocker) -> Self {
        Self { unlocker: Some(unlocker), ..self }
    }

    pub fn poll_unlock(&self) -> Vec<Action> {
        self.unlocker.as_ref().map(Unlocker::poll).unwrap_or_default()
    }

    #[cfg(target_os = "windows")]
    pub fn unlock_parent_window(&self, window: isize) {
        if let Some(unlocker) = &self.unlocker {
            unlocker.set_parent_window(window);
        }
    }

    pub fn joining(self, device: Server, wake: Wake) -> Self {
        let joiner = Joiner::new(wake.clone());
        Self { joining: Some(Joining { device, joiner, wake }), ..self }
    }

    pub fn resume_server(&self) {
        if let Some(joining) = &self.joining
            && joining.joiner.resume()
        {
            tracing::info!("connecting to the team's server signed in to last");
        }
    }

    pub fn poll_join(&mut self) -> Option<Action> {
        let joining = self.joining.as_ref()?;
        let joined = joining.joiner.finished()?.and_then(|connected| {
            let (remote, joined, sign_in) = connected.split();
            super::connect::greeted(&joining.device, Some(remote), joining.wake.clone())
                .map(|backend| (backend, joined, sign_in))
                .map_err(|error| error.to_string())
        });
        Some(Action::ServerLink(ServerAction::Joined(joined.map(|(backend, joined, sign_in)| {
            self.backend = backend;
            if let Some(sign_in) = sign_in {
                sign_in.keep();
            }
            joined
        }))))
    }

    fn leave_server(&mut self) -> Option<Action> {
        let joining = self.joining.as_ref()?;
        joining.joiner.leave();
        match super::connect::greeted(&joining.device, None, joining.wake.clone()) {
            Ok(backend) => self.backend = backend,
            Err(error) => tracing::error!(%error, "could not go back to this computer's server"),
        }
        Some(Action::ServerLink(ServerAction::Left))
    }

    pub fn backend(&self) -> &dyn Backend {
        self.backend.as_ref()
    }

    pub fn run(&mut self, effects: Vec<Effect>, session: &Session) -> Vec<Action> {
        let mut follow_up = Vec::new();
        for effect in effects {
            match effect {
                Effect::LoadBrowserProfiles => {
                    let root = superfik_backend::local_data_dir().unwrap_or_else(std::env::temp_dir).join("browser");
                    let result = superfik_browser::profiles::ProfileStore::new(root).load();
                    follow_up
                        .push(Action::BrowserProfile(crate::state::browser_profiles::ProfileAction::Loaded(result)));
                }
                Effect::SaveBrowserProfiles { book, credential } => {
                    let root = superfik_backend::local_data_dir().unwrap_or_else(std::env::temp_dir).join("browser");
                    let result = superfik_browser::profiles::ProfileStore::new(root)
                        .save_with_credentials(
                            &book,
                            credential.as_ref(),
                            &superfik_browser::profiles::KeychainCredentials,
                        )
                        .map(|()| *book);
                    follow_up
                        .push(Action::BrowserProfile(crate::state::browser_profiles::ProfileAction::Saved(result)));
                }
                Effect::Command(command) => {
                    tracing::debug!(command = command.name(), "send");
                    if let Err(error) = self.backend.send(Envelope::local(*command)) {
                        tracing::error!(%error, "backend rejected command");
                    }
                }
                Effect::PickWorkspace => {
                    let title = session.strings().t(Text::DialogOpenRepository);
                    if let Some(path) = rfd::FileDialog::new().set_title(title).pick_folder() {
                        follow_up.push(Action::OpenWorkspace(path));
                    }
                }
                Effect::PickReferences => {
                    let strings = session.strings();
                    let files = rfd::FileDialog::new()
                        .set_title(strings.t(Text::DialogAddReferences))
                        .add_filter(strings.t(Text::ImagesFilter), &["png", "jpg", "jpeg", "gif", "webp"])
                        .pick_files();
                    if let Some(files) = files {
                        follow_up.push(Action::AddReferences(files));
                    }
                }
                Effect::Upload(paths) => {
                    for path in paths {
                        let sent = super::upload::command(&path)
                            .and_then(|command| self.backend.send(Envelope::local(command)).map_err(|e| e.to_string()));
                        if let Err(why) = sent {
                            follow_up.push(Action::Server(ServerEvent::error(why)));
                        }
                    }
                }
                Effect::PickRepository => {
                    let title = session.strings().t(Text::DialogAttachRepository);
                    if let Some(path) = rfd::FileDialog::new().set_title(title).pick_folder() {
                        follow_up.push(Action::AddRepositoryPath(path));
                    }
                }
                Effect::Persist => {
                    self.sounds.set_prefs(session.preferences.sound);
                    let settings = Settings::of(session).to_value();
                    let command = superfik_protocol::ClientCommand::SaveSettings { settings };
                    if let Err(error) = self.backend.send(Envelope::local(command)) {
                        tracing::error!(%error, "backend rejected the settings");
                    }
                }
                Effect::KeepSession { path, project, session } => {
                    let command = superfik_protocol::ClientCommand::SaveSession { path, project, session };
                    if let Err(error) = self.backend.send(Envelope::local(command)) {
                        tracing::error!(%error, "backend rejected command");
                    }
                }
                Effect::Sound(cue) => self.sounds.play(cue),
                Effect::Voice(call) => self.voice.call(call),
                Effect::OpenUrl(url) => super::url::open(&url),
                Effect::Browser(_) | Effect::ImportLogins(_) | Effect::ClearCookies | Effect::SaveLogin { .. } => {}
                Effect::JoinServer(request) => match &self.joining {
                    Some(joining) => joining.joiner.join(request),
                    None => follow_up.push(Action::ServerLink(ServerAction::Joined(Err(
                        "This app cannot join a server.".to_owned(),
                    )))),
                },
                Effect::LeaveServer => follow_up.extend(self.leave_server()),
                Effect::Unlock => match &self.unlocker {
                    Some(unlocker) => unlocker.ask(),
                    None => follow_up
                        .push(Action::Lock(crate::state::LockAction::Failed(crate::state::LockProblem::Unavailable))),
                },
                Effect::Update(call) => match &self.updater {
                    Some(updater) => updater.run(call),
                    None => tracing::warn!(?call, "this install cannot update itself"),
                },
                Effect::CopyText(text) => {
                    let copied = self.clipboard.set_text(text);
                    if let Err(error) = copied {
                        tracing::error!(%error, "could not copy");
                    }
                }
                Effect::PickPluginFolder => {
                    let title = session.strings().t(Text::DialogAddPlugin);
                    if let Some(path) = rfd::FileDialog::new().set_title(title).pick_folder() {
                        follow_up.push(Action::InstallPluginFolder(path));
                    }
                }
            }
        }
        follow_up
    }
}
