use domain::SessionId;
use serde_json::{Value, json};

use crate::engine::job::Job;
use crate::hooks::HookEvent;
use crate::tools::Workspace;
use crate::transcript::Message;

pub(in crate::engine) type Announced = Vec<(HookEvent, &'static str, Value)>;

impl Job {
    pub(in crate::engine) fn announced(&self, workspace: &Workspace, session: &SessionId, user: &Message) -> Announced {
        let source = if self.request.session.resume.is_some() { "resume" } else { "startup" };
        let started = json!({"source":source,"session_id":session,"model":self.request.session.model.id});
        let mut announced = Vec::new();
        if self.live.begins_session(workspace, self.request.session.cwd.clone()) {
            announced.push((HookEvent::SessionStart, source, started.clone()));
        }
        announced.push((HookEvent::RunStart, source, started));
        announced.push((HookEvent::UserPromptSubmit, "", json!({"prompt":user.text(),"session_id":session})));
        announced
    }
}
