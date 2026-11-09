//! Comments on shared sessions (`superfik_core::comments`), kept by the
//! organization's server per project and session. Only who can see a
//! session reads or adds its comments, and a new one goes to them alone.

use std::path::PathBuf;
use std::sync::Mutex;

use superfik_core::comments::{self, Comment};
use superfik_core::project::ProjectRef;
use superfik_core::{ConversationId, Member, Principal, UserId};
use superfik_protocol::{Addressed, ClientCommand, Recipient, ServerEvent};

use crate::Organization;
use crate::project_sessions::{ProjectSessions, sees};

pub trait CommentStore: Send + Sync {
    fn load(&self, project: &ProjectRef, session: ConversationId) -> Vec<Comment>;
    fn store(&self, project: &ProjectRef, session: ConversationId, comments: &[Comment]) -> std::io::Result<()>;
}

#[derive(Debug)]
pub struct FileComments {
    root: PathBuf,
    writing: Mutex<()>,
}

impl FileComments {
    pub fn new(root: PathBuf) -> Self {
        Self { root, writing: Mutex::new(()) }
    }

    fn file(&self, project: &ProjectRef, session: ConversationId) -> PathBuf {
        self.root.join(crate::project_sessions::folder_name(project)).join(format!("{session}.json"))
    }
}

impl CommentStore for FileComments {
    fn load(&self, project: &ProjectRef, session: ConversationId) -> Vec<Comment> {
        let file = self.file(project, session);
        std::fs::read(file).ok().and_then(|json| serde_json::from_slice(&json).ok()).unwrap_or_default()
    }

    fn store(&self, project: &ProjectRef, session: ConversationId, comments: &[Comment]) -> std::io::Result<()> {
        let _writing = self.writing.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let file = self.file(project, session);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = file.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(comments).map_err(std::io::Error::other)?)?;
        std::fs::rename(temporary, file)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryComments(Mutex<Vec<(ProjectRef, ConversationId, Vec<Comment>)>>);

impl CommentStore for InMemoryComments {
    fn load(&self, project: &ProjectRef, session: ConversationId) -> Vec<Comment> {
        let kept = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        kept.iter().find(|(p, s, _)| p == project && *s == session).map(|(.., c)| c.clone()).unwrap_or_default()
    }

    fn store(&self, project: &ProjectRef, session: ConversationId, comments: &[Comment]) -> std::io::Result<()> {
        let mut kept = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        kept.retain(|(p, s, _)| !(p == project && *s == session));
        kept.push((project.clone(), session, comments.to_vec()));
        Ok(())
    }
}

pub fn answer(
    org: &Organization,
    sessions: &dyn ProjectSessions,
    store: &dyn CommentStore,
    by: &Principal,
    command: &ClientCommand,
) -> Option<Vec<Addressed>> {
    let (project, session) = match command {
        ClientCommand::LoadComments { project, session } | ClientCommand::AddComment { project, session, .. } => {
            (project, *session)
        }
        _ => return None,
    };
    let to_sender = |event| Addressed { to: Recipient::Principal(by.clone()), event };
    let refused = |why: &str| Some(vec![to_sender(ServerEvent::error(why))]);
    let Principal::Member(member) = by else { return refused("not allowed") };
    let all = sessions.all(project);
    let Some((owner, record)) = all.iter().find(|(_, record)| record.id == session) else {
        return refused("no such session");
    };
    if !sees(org, member, *owner, record) {
        return refused("not allowed");
    }
    let mut kept = store.load(project, session);
    let ClientCommand::AddComment { after_turn, text, .. } = command else {
        return Some(vec![to_sender(ServerEvent::Comments { session, comments: kept })]);
    };
    let Some(text) = comments::tidy(text) else { return Some(Vec::new()) };
    let author_name = org
        .directory
        .members()
        .ok()
        .and_then(|accounts| accounts.into_iter().find(|account| account.user == member.user))
        .map(|account| account.name)
        .unwrap_or_default();
    kept.push(Comment { author: member.user, author_name, at_unix: org.now(), after_turn: *after_turn, text });
    if let Err(error) = store.store(project, session, &kept) {
        return refused(&format!("could not keep the comment: {error}"));
    }
    let readers: Vec<UserId> = org
        .directory
        .members()
        .unwrap_or_default()
        .into_iter()
        .map(|account| Member { user: account.user, org: member.org })
        .filter(|reader| reader.user == *owner || sees(org, reader, *owner, record))
        .map(|reader| reader.user)
        .collect();
    Some(
        readers
            .into_iter()
            .map(|user| Addressed {
                to: Recipient::Principal(Principal::Member(Member { user, org: member.org })),
                event: ServerEvent::Comments { session, comments: kept.clone() },
            })
            .collect(),
    )
}
