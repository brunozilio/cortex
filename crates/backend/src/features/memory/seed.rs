use std::path::Path;

use agent_engine::sessions::{FileSessions, records};
use domain::memory::seed::{Candidate, candidates};
use domain::trace::Record;
use domain::{Author, MemoryEdit, MemoryKind, ModelOption, Principal, PromptRequest, Provenance, SessionId};
use serde_json::{Value, json};

use super::{Memories, now, team};
use crate::feature::Context;

const ORIGIN: &str = "seeded from past sessions";

impl Memories {
    pub fn seed(&self, context: &Context<'_>, project: &Path) -> Result<Value, String> {
        let offered = self.offered(context, project)?;
        Ok(json!({ "path": project, "candidates": offered }))
    }

    pub fn keep_seeds(&self, context: &Context<'_>, project: &Path, picked: &[usize]) -> Result<Value, String> {
        let offered = self.offered(context, project)?;
        let edits: Vec<MemoryEdit> = picked
            .iter()
            .filter_map(|n| n.checked_sub(1).and_then(|at| offered.get(at)))
            .map(|candidate| MemoryEdit::Remember {
                kind: MemoryKind::Procedural,
                text: candidate.text.clone(),
                tags: vec!["seeded".into()],
                touches: Vec::new(),
            })
            .collect();
        if edits.is_empty() {
            return Err("None of those numbers is offered (see /memory seed).".into());
        }
        let by = Provenance { author: Author::User, now: now(), origin: Some(ORIGIN.into()) };
        self.edit(context.principal, project, &edits, by)
    }

    fn offered(&self, context: &Context<'_>, project: &Path) -> Result<Vec<Candidate>, String> {
        if !team::features(project, self.home(context.principal).as_deref()).on(domain::features::MEMORY_SEED) {
            return Err(format!("/memory seed is turned off ({}).", domain::features::MEMORY_SEED));
        }
        let data = crate::local_data_dir().unwrap_or_else(std::env::temp_dir);
        let store = FileSessions::new(data.join(crate::native::SESSIONS));
        let owner_settings = context.services.settings.load(context.principal).unwrap_or_default();
        let sessions = crate::settings::recent_sessions(&owner_settings, project);
        let known = {
            let mut books = self.lock();
            self.book(&mut books, context.principal, project).memories.clone()
        };
        Ok(candidates(&prompts(&store, context.principal, project, &sessions), &known))
    }
}

pub fn prompts(store: &FileSessions, owner: &Principal, project: &Path, sessions: &[SessionId]) -> Vec<Vec<String>> {
    let mut request = PromptRequest::plain(project.into(), "", ModelOption::new("", ""));
    request.credentials.owner = Some(owner.clone());
    sessions
        .iter()
        .filter_map(|id| store.entries(&request, id).ok())
        .map(|entries| {
            records(entries)
                .into_iter()
                .filter_map(|record| match record {
                    Record::Prompted { prompt, .. } => Some(prompt),
                    _ => None,
                })
                .collect::<Vec<String>>()
        })
        .filter(|said| !said.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use agent_engine::sessions::{Entry, Sessions};

    use super::*;

    #[test]
    fn each_session_gives_the_prompts_typed_in_it() {
        let dir = tempfile::tempdir().expect("dir");
        let store = FileSessions::new(dir.path().join("sessions"));
        let mut request = PromptRequest::plain(dir.path().into(), "", ModelOption::new("", ""));
        request.credentials.owner = Some(Principal::LocalOwner);
        let (one, two) = (SessionId("one".into()), SessionId("two".into()));
        for (session, prompt) in [(&one, "Always run cargo nextest"), (&two, "always run cargo nextest please")] {
            store.record(&request, session, &Entry::Prompted { at_ms: 1, prompt: prompt.into() }).expect("record");
        }

        let said = prompts(&store, &Principal::LocalOwner, dir.path(), &[one, two, SessionId("gone".into())]);

        assert_eq!(
            said,
            [vec!["Always run cargo nextest".to_owned()], vec!["always run cargo nextest please".to_owned()]]
        );
        assert_eq!(candidates(&said, &[]).len(), 1);
    }
}
