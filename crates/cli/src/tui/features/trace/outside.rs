use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use platform::sync::Locked;
use sdk::{Effort, PermissionMode, RunControl, Sdk, SessionId};

use super::live::Prompt;
use crate::tui::app::{App, Msg};

pub type Running = Arc<Mutex<BTreeMap<String, RunControl>>>;

struct Asked {
    cwd: PathBuf,
    model: domain::ModelOption,
    mode: PermissionMode,
    effort: Effort,
    prompt: Prompt,
}

pub fn deliver(app: &mut App, prompts: Vec<Prompt>, running: &Running) {
    for prompt in prompts {
        let current = app.session.as_ref().and_then(sdk::Session::id);
        if current.is_some_and(|id| id.0 == prompt.session) {
            app.take_prompt(prompt.text);
            continue;
        }
        let steered = running.locked().get(&prompt.session).is_some_and(|run| run.steer(&prompt.text).is_ok());
        if steered {
            continue;
        }
        let Some(session) = &app.session else {
            app.transcript.notice("A prompt came from the trace page, but there is no model yet: /login connects one.");
            continue;
        };
        let asked = Asked {
            cwd: app.cwd.clone(),
            model: session.model().clone(),
            mode: session.permission_mode(),
            effort: session.effort_level(),
            prompt,
        };
        let running = Arc::clone(running);
        app.spawn(move |sdk| {
            let session = asked.prompt.session.clone();
            let outcome = ran(sdk, asked, &running);
            Msg::Feature(Box::new(move |app: &mut App| {
                if let Err(error) = outcome {
                    app.transcript.notice(format!("The prompt sent from the trace page to {session} failed: {error}"));
                }
            }))
        });
    }
}

fn ran(sdk: &Sdk, asked: Asked, running: &Running) -> Result<(), String> {
    let sibling = sdk.sibling().unwrap_or_else(|_| sdk.clone());
    let id = asked.prompt.session;
    let session = sibling
        .session(asked.cwd, asked.model)
        .effort(asked.effort)
        .permission(asked.mode)
        .resume(SessionId(id.clone()));
    let run = session.send(&asked.prompt.text).map_err(|error| error.to_string())?;
    running.locked().insert(id.clone(), run.control());
    let finished = run.text();
    running.locked().remove(&id);
    finished.map(|_| ()).map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "outside_tests.rs"]
mod tests;
