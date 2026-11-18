use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use app::{Client, EngineEvent, RunControl, SessionId};
use console::{ChatError, Chats, Live};
use domain::catalog::Listed;
use domain::conversation::{Entry, Transcript};
use serde_json::Value;

const NOTIFY_GAP: Duration = Duration::from_millis(300);

struct Going {
    control: RunControl,
    transcript: Transcript,
    title: Option<String>,
}

struct Sent<'a> {
    text: &'a str,
    early: Vec<EngineEvent>,
    run: app::Run,
}

type Runs =Arc<Mutex<HashMap<String, Going>>>;

pub(super) struct PhoneChats {
    client: Mutex<Client>,
    folder: PathBuf,
    live: Live,
    runs: Runs,
}

fn failed(error: impl ToString) -> ChatError {
    ChatError::Failed(error.to_string())
}

fn locked(runs: &Runs) -> MutexGuard<'_, HashMap<String, Going>> {
    runs.lock().unwrap_or_else(PoisonError::into_inner)
}

impl PhoneChats {
    pub(super) fn new(client: Client, folder: PathBuf, live: Live) -> Self {
        Self { client: Mutex::new(client), folder, live, runs: Arc::default() }
    }

    fn client(&self) -> Client {
        self.client.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    fn row(&self, session: &str) -> Result<Listed, ChatError> {
        let rows = self.client().sessions().map_err(failed)?;
        rows.into_iter()
            .find(|row| row.session.0 == session)
            .ok_or_else(|| ChatError::Missing("There is no such chat on the machine.".to_owned()))
    }

    fn replayed(&self, session: &str, cwd: &Path) -> Result<Transcript, ChatError> {
        let records = self.client().history(&SessionId(session.to_owned()), cwd).map_err(failed)?;
        let mut transcript = Transcript::default();
        transcript.replay(&records);
        Ok(transcript)
    }

    fn going(&self, session: &str, title: Option<String>, mut transcript: Transcript, sent: Sent) -> Result<(), ChatError> {
        transcript.push(Entry::User(sent.text.to_owned()));
        transcript.begin();
        for event in &sent.early {
            transcript.apply(event);
        }
        let control = sent.run.control();
        locked(&self.runs).insert(session.to_owned(), Going { control, transcript, title });
        self.live.chat_changed(session);
        let (runs, live, named) = (Arc::clone(&self.runs), self.live.clone(), session.to_owned());
        let run = sent.run;
        let pumped = std::thread::Builder::new().name("phone-chat".into()).spawn(move || pump(run, &runs, &live, &named));
        if let Err(error) = pumped {
            locked(&self.runs).remove(session);
            return Err(failed(error));
        }
        Ok(())
    }
}

fn pump(run: app::Run, runs: &Runs, live: &Live, session: &str) {
    let mut told = Instant::now();
    for event in run {
        let streaming = matches!(event, EngineEvent::TextDelta(_) | EngineEvent::ThinkingDelta(_));
        if let Some(going) = locked(runs).get_mut(session) {
            going.transcript.apply(&event);
        }
        if !streaming || told.elapsed() >= NOTIFY_GAP {
            live.chat_changed(session);
            told = Instant::now();
        }
    }
    locked(runs).remove(session);
    live.chat_changed(session);
}

impl Chats for PhoneChats {
    fn list(&self) -> Result<Value, ChatError> {
        let rows = self.client().sessions().map_err(failed)?;
        Ok(domain::chat::listed(&rows))
    }

    fn open(&self, session: &str) -> Result<Value, ChatError> {
        if let Some(going) = locked(&self.runs).get(session) {
            return Ok(domain::chat::view(session, going.title.as_deref(), &going.transcript));
        }
        let row = self.row(session)?;
        let transcript = self.replayed(session, &row.cwd)?;
        Ok(domain::chat::view(session, row.title.as_deref(), &transcript))
    }

    fn start(&self, text: &str) -> Result<String, ChatError> {
        let client = self.client();
        let model = crate::print::chosen_model(&client, None).map_err(ChatError::Failed)?;
        let session = client.session(self.folder.clone(), model);
        let mut run = session.send(text).map_err(failed)?;
        let mut early = Vec::new();
        let id = loop {
            match run.next_event() {
                Some(EngineEvent::SessionStarted(id)) => break id.0,
                Some(EngineEvent::Failed(reason)) => return Err(ChatError::Failed(reason)),
                Some(event) => early.push(event),
                None => return Err(ChatError::Failed("The chat ended before it started.".to_owned())),
            }
        };
        self.going(&id, None, Transcript::default(), Sent { text, early, run })?;
        Ok(id)
    }

    fn send(&self, session: &str, text: &str) -> Result<(), ChatError> {
        if locked(&self.runs).contains_key(session) {
            return Err(ChatError::Busy("This chat is still answering.".to_owned()));
        }
        let row = self.row(session)?;
        if !domain::chat::takes_prompt(row.status) {
            return Err(ChatError::Busy("This chat is running on the machine; wait for it to finish.".to_owned()));
        }
        let transcript = self.replayed(session, &row.cwd)?;
        let client = self.client();
        let model = crate::print::chosen_model(&client, None).map_err(ChatError::Failed)?;
        let run = client.session(row.cwd.clone(), model).resume(row.session.clone()).send(text).map_err(failed)?;
        self.going(session, row.title, transcript, Sent { text, early: Vec::new(), run })
    }

    fn answer(&self, session: &str, ask: &str, decision: &str) -> Result<(), ChatError> {
        let decision = domain::chat::decision(decision)
            .ok_or_else(|| ChatError::Failed(format!("{decision} is not a decision a phone can make.")))?;
        let runs = locked(&self.runs);
        let going = runs.get(session).ok_or_else(|| ChatError::Missing("Nothing in this chat waits for you.".to_owned()))?;
        going.control.answer(app::AskId(ask.to_owned()), decision).map_err(failed)
    }

    fn stop(&self, session: &str) -> Result<(), ChatError> {
        let runs = locked(&self.runs);
        let going = runs.get(session).ok_or_else(|| ChatError::Missing("This chat is not running.".to_owned()))?;
        going.control.cancel().map_err(failed)
    }
}

#[cfg(test)]
#[path = "chats_tests.rs"]
mod tests;
