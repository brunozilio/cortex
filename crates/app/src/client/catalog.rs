use std::path::Path;

use crate::api::features::sessions::{SessionsReply, SessionsRequest};
use domain::SessionId;
use domain::catalog::Listed;
use domain::trace::Record;

use crate::client::features::unexpected;
use crate::{Client, Error};

impl Client {
    pub fn sessions(&self) -> Result<Vec<Listed>, Error> {
        match self.ask_sessions(SessionsRequest::List)? {
            SessionsReply::Listed(listed) => Ok(listed),
            _ => Err(unexpected()),
        }
    }

    pub fn history(&self, session: &SessionId, cwd: &Path) -> Result<Vec<Record>, Error> {
        match self.ask_sessions(SessionsRequest::History { session: session.clone(), cwd: cwd.to_path_buf() })? {
            SessionsReply::History(records) => Ok(records),
            _ => Err(unexpected()),
        }
    }

    pub fn session_length(&self, session: &SessionId, cwd: &Path) -> Result<u64, Error> {
        match self.ask_sessions(SessionsRequest::Length { session: session.clone(), cwd: cwd.to_path_buf() })? {
            SessionsReply::Length(length) => Ok(length),
            _ => Err(unexpected()),
        }
    }

    pub fn rename_session(&self, session: &SessionId, title: &str) -> Result<(), Error> {
        self.change(SessionsRequest::Rename { session: session.clone(), title: title.to_owned() })
    }

    pub fn seen_session(&self, session: &SessionId) -> Result<(), Error> {
        self.change(SessionsRequest::Seen { session: session.clone() })
    }

    pub fn delete_session(&self, session: &SessionId) -> Result<(), Error> {
        self.change(SessionsRequest::Delete { session: session.clone() })
    }

    fn change(&self, request: SessionsRequest) -> Result<(), Error> {
        match self.ask_sessions(request)? {
            SessionsReply::Changed => Ok(()),
            _ => Err(unexpected()),
        }
    }
}
