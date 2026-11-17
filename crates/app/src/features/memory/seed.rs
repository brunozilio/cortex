use std::path::Path;

use crate::api::features::memory::MemoryListing;
use agent_engine::sessions::{FileSessions, records};
use domain::memory::seed::{Candidate, candidates};
use domain::trace::Record;
use domain::{Author, MemoryEdit, MemoryKind, ModelOption, PromptRequest, Provenance, SessionId};

use super::{Memories, now, team};
use crate::features::context::Context;

const ORIGIN: &str = "seeded from past sessions";

impl Memories {
    pub fn keep_seeds(&self, context: &Context<'_>, project: &Path, picked: &[usize]) -> Result<MemoryListing, String> {
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
        self.edit(project, &edits, by)
    }

    pub fn offered(&self, context: &Context<'_>, project: &Path) -> Result<Vec<Candidate>, String> {
        if !team::features(project, self.home().as_deref()).on(domain::features::MEMORY_SEED) {
            return Err(format!("/memory seed is turned off ({}).", domain::features::MEMORY_SEED));
        }
        let store = context.services.sessions.as_ref();
        let owner_settings = context.services.settings.load().unwrap_or_default();
        let sessions = crate::settings::recent_sessions(&owner_settings, project);
        let known = {
            let mut books = self.lock();
            self.book(&mut books, project).memories.clone()
        };
        Ok(candidates(&prompts(store, project, &sessions), &known))
    }
}

pub fn prompts(store: &FileSessions, project: &Path, sessions: &[SessionId]) -> Vec<Vec<String>> {
    let request = PromptRequest::plain(project.into(), "", ModelOption::new("", ""));
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
        let request = PromptRequest::plain(dir.path().into(), "", ModelOption::new("", ""));
        let (one, two) = (SessionId("one".into()), SessionId("two".into()));
        for (session, prompt) in [(&one, "Always run cargo nextest"), (&two, "always run cargo nextest please")] {
            store.record(&request, session, &Entry::Prompted { at_ms: 1, prompt: prompt.into() }).expect("record");
        }

        let said = prompts(&store, dir.path(), &[one, two, SessionId("gone".into())]);

        assert_eq!(
            said,
            [vec!["Always run cargo nextest".to_owned()], vec!["always run cargo nextest please".to_owned()]]
        );
        assert_eq!(candidates(&said, &[]).len(), 1);
    }
}
