pub(super) mod cli;
mod board;
mod follow;
mod http;
mod live;
mod outside;
mod page;

use std::path::{Path, PathBuf};

use domain::SessionId;
use serde_json::{Value, json};

use super::{Feature, Slash};
use crate::tui::app::{App, Msg};

const NAME: &str = "trace";

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["trace"],
            about: "Open the live trace in the browser: each run's spans, cost, waits and checks, plus the project's last 30 days (/trace [session])",
            run: trace,
        }],
        tick: Some(follow::tick),
        ..Feature::default()
    }
}

fn trace(app: &mut App, argument: Option<String>) {
    let current = app.session.as_ref().and_then(sdk::Session::id);
    let Some(session) = argument.map(SessionId).or(current) else {
        return app.transcript.notice("Nothing to trace yet: send a prompt first, or name a session: /trace <id>.");
    };
    let cwd = app.cwd.clone();
    app.spawn(move |sdk| match report(sdk, &session, &cwd) {
        Ok(report) => Msg::Feature(Box::new(move |app: &mut App| follow::open(app, &session, &report))),
        Err(error) => Msg::Error(error),
    });
}

fn report(sdk: &sdk::Sdk, session: &SessionId, cwd: &Path) -> Result<Value, String> {
    sdk.feature(NAME, json!({ "session": session.0, "cwd": cwd })).map_err(|e| e.to_string())
}

fn export(sdk: &sdk::Sdk, session: &SessionId, cwd: &Path) -> Result<PathBuf, String> {
    keep(session, &report(sdk, session, cwd)?)
}

fn keep(session: &SessionId, trace: &Value) -> Result<PathBuf, String> {
    let home = std::env::home_dir().ok_or("no home folder to keep the trace in")?;
    let path = home.join(brand::DIR).join("traces").join(format!("{}.html", page::file_name(&session.0)));
    platform::private_file::replace(&path, page::page(&session.0, trace).as_bytes()).map_err(|e| e.to_string())?;
    Ok(path)
}
