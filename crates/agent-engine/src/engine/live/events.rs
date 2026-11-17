use domain::SessionId;
use serde_json::{Value, json};

use crate::engine::job::Job;
use crate::tools::Workspace;
use crate::transcript::Message;

pub(in crate::engine) type Announced = Vec<(&'static str, &'static str, Value)>;

impl Job {
    pub(in crate::engine) fn announced(&self, workspace: &Workspace, session: &SessionId, user: &Message) -> Announced {
        let source = if self.request.resume.is_some() { "resume" } else { "startup" };
        let started = json!({"source":source,"session_id":session,"model":self.request.model.id});
        let mut announced = Vec::new();
        if self.live.begins_session(workspace, self.request.cwd.clone()) {
            announced.push(("SessionStart", source, started.clone()));
        }
        announced.push(("RunStart", source, started));
        announced.push(("UserPromptSubmit", "", json!({"prompt":user.text(),"session_id":session})));
        announced
    }
}
