use std::path::Path;

use domain::SessionId;
use domain::catalog::Listed;
use domain::trace::Record;
use serde_json::{Value, json};

use crate::{Error, Sdk};

const FEATURE: &str = "sessions";

impl Sdk {
    pub fn sessions(&self) -> Result<Vec<Listed>, Error> {
        let answer = self.ask(json!({ "op": "list" }))?;
        serde_json::from_value(answer["sessions"].clone()).map_err(|error| Error::Backend(error.to_string()))
    }

    pub fn history(&self, session: &SessionId, cwd: &Path) -> Result<Vec<Record>, Error> {
        let answer = self.ask(json!({ "op": "history", "session": session.0, "cwd": cwd }))?;
        serde_json::from_value(answer["records"].clone()).map_err(|error| Error::Backend(error.to_string()))
    }

    pub fn session_length(&self, session: &SessionId, cwd: &Path) -> Result<u64, Error> {
        let answer = self.ask(json!({ "op": "length", "session": session.0, "cwd": cwd }))?;
        answer["length"].as_u64().ok_or_else(|| Error::Backend("the session's length was not a number".into()))
    }

    pub fn rename_session(&self, session: &SessionId, title: &str) -> Result<(), Error> {
        self.ask(json!({ "op": "rename", "session": session.0, "title": title })).map(drop)
    }

    pub fn archive_session(&self, session: &SessionId, archived: bool) -> Result<(), Error> {
        self.ask(json!({ "op": "archive", "session": session.0, "archived": archived })).map(drop)
    }

    pub fn seen_session(&self, session: &SessionId) -> Result<(), Error> {
        self.ask(json!({ "op": "seen", "session": session.0 })).map(drop)
    }

    pub fn delete_session(&self, session: &SessionId) -> Result<(), Error> {
        self.ask(json!({ "op": "delete", "session": session.0 })).map(drop)
    }

    fn ask(&self, input: Value) -> Result<Value, Error> {
        self.feature(FEATURE, input)
    }
}
