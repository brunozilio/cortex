//! The notes tools, served to agents over MCP on a loopback port (the
//! same shape as the memory tool). Each run gets a bearer token naming
//! its owner, project and run, revoked when the run ends. Every change an
//! agent makes goes out as `ServerEvent::Notes { by: Some(run) }`, so the
//! canvas shows it and a note the agent creates links to its session.
//!
//! [`NotesTools::handle`] is the protocol over a token; [`NotesTools::start`]
//! adds the socket.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde_json::{Value, json};
use superfik_core::note::{
    Author, Line, NOTES_CHECK_TOOL, NOTES_LIST_TOOL, NOTES_READ_TOOL, NOTES_SERVER, NOTES_WRITE_TOOL, Note, NoteBook,
    NoteEdit, NoteId, NotesTool, Paper, Stamp,
};
use superfik_core::{Principal, RunId};
use superfik_protocol::ServerEvent;

use super::Notes;
use crate::loopback::{self, RpcError};

pub type Publish = Arc<dyn Fn(&Principal, ServerEvent) + Send + Sync>;

#[derive(Clone)]
struct Grant {
    owner: Principal,
    project: PathBuf,
    run: RunId,
}

#[derive(Clone)]
pub struct NotesTools {
    url: String,
    grants: Arc<Mutex<HashMap<String, Grant>>>,
    notes: Notes,
    publish: Publish,
}

impl std::fmt::Debug for NotesTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotesTools").field("url", &self.url).finish_non_exhaustive()
    }
}

#[derive(Debug, PartialEq)]
enum Ask {
    List,
    Read(NoteId),
    Edit(NoteEdit),
}

impl NotesTools {
    pub fn new(notes: Notes, publish: Publish) -> Self {
        Self { url: String::new(), grants: Arc::default(), notes, publish }
    }

    pub fn start(notes: Notes, publish: Publish) -> Option<Self> {
        let server = match tiny_http::Server::http("127.0.0.1:0") {
            Ok(server) => server,
            Err(error) => {
                tracing::warn!(%error, "notes tools unavailable");
                return None;
            }
        };
        let port = server.server_addr().to_ip()?.port();
        let tools = Self { url: format!("http://127.0.0.1:{port}/mcp"), ..Self::new(notes, publish) };
        let served = tools.clone();
        std::thread::Builder::new()
            .name("superfik-notes-mcp".into())
            .spawn(move || {
                for request in server.incoming_requests() {
                    served.serve(request);
                }
            })
            .ok()?;
        Some(tools)
    }

    pub fn grant(&self, owner: &Principal, project: PathBuf, run: RunId) -> NotesTool {
        let token = uuid::Uuid::new_v4().simple().to_string();
        lock(&self.grants).insert(token.clone(), Grant { owner: owner.clone(), project, run });
        NotesTool { url: self.url.clone(), token }
    }

    pub fn revoke(&self, token: &str) {
        lock(&self.grants).remove(token);
    }

    pub fn handle(&self, token: &str, message: &Value) -> Result<Option<Value>, u16> {
        let grant = lock(&self.grants).get(token).cloned().ok_or(401_u16)?;
        Ok(loopback::respond(message, NOTES_SERVER, definitions, |name, arguments| self.call(&grant, name, arguments)))
    }

    fn call(&self, grant: &Grant, name: &str, arguments: &Value) -> Result<Value, RpcError> {
        let ask = match ask(name, arguments)? {
            Ok(ask) => ask,
            Err(problem) => return Ok(loopback::text(problem, true)),
        };
        let book = || self.notes.book(&grant.owner, &grant.project);
        Ok(match ask {
            Ask::List => loopback::text(listing(&book()), false),
            Ask::Read(id) => match book().get(id) {
                Some(note) => loopback::text(format!("#{} {}\n{}", id.0, note.title(), note.body), false),
                None => loopback::text(format!("There is no note #{}.", id.0), true),
            },
            Ask::Edit(edit) => {
                let by = Stamp { author: Author::Agent, now: crate::memory::now() };
                let event = self.notes.edit(&grant.owner, &grant.project, &[edit], by, Some(grant.run));
                let (text, error) = report(&event);
                (self.publish)(&grant.owner, event);
                loopback::text(text, error)
            }
        })
    }

    fn serve(&self, mut request: tiny_http::Request) {
        let Some(token) = loopback::bearer(&request) else { return loopback::reply(request, 401, None) };
        if !lock(&self.grants).contains_key(&token) {
            return loopback::reply(request, 401, None);
        }
        let message = match loopback::read_message(&mut request) {
            Ok(message) => message,
            Err(status) => return loopback::reply(request, status, None),
        };
        match self.handle(&token, &message) {
            Ok(Some(answer)) => loopback::reply(request, 200, Some(answer)),
            Ok(None) => loopback::reply(request, 202, None),
            Err(status) => loopback::reply(request, status, None),
        }
    }
}

fn definitions() -> Vec<Value> {
    let id = json!({ "type": "integer", "minimum": 0, "description": "The note's id (#n)." });
    vec![
        json!({
            "name": NOTES_LIST_TOOL,
            "description": "List the user's notes for this project: id, title and checklist progress.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": NOTES_READ_TOOL,
            "description": "Read one note's whole text.",
            "inputSchema": { "type": "object", "properties": { "id": id }, "required": ["id"] }
        }),
        json!({
            "name": NOTES_WRITE_TOOL,
            "description": "Write a note. Without an id, creates one (linked to this session); with an id, \
                            replaces that note's whole text. The first line is the title; `- [ ] item` and \
                            `- [x] item` lines are a checklist.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": id,
                    "body": { "type": "string", "description": "The note's whole text." },
                    "paper": { "type": "string", "enum": ["plain", "yellow", "blue", "pink", "green"], "description": "Paper colour of a new note (default yellow)." }
                },
                "required": ["body"]
            }
        }),
        json!({
            "name": NOTES_CHECK_TOOL,
            "description": "Tick or untick one checklist item of a note.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": id,
                    "item": { "type": "integer", "minimum": 1, "description": "Which checklist item, counting checklist lines only, from 1." },
                    "done": { "type": "boolean" }
                },
                "required": ["id", "item", "done"]
            }
        }),
    ]
}

fn ask(name: &str, arguments: &Value) -> Result<Result<Ask, String>, RpcError> {
    let id = || {
        arguments
            .get("id")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .map(NoteId)
            .ok_or_else(|| "Give the note's id.".to_owned())
    };
    Ok(match name {
        NOTES_LIST_TOOL => Ok(Ask::List),
        NOTES_READ_TOOL => id().map(Ask::Read),
        NOTES_WRITE_TOOL => {
            let body = arguments.get("body").and_then(Value::as_str).unwrap_or_default().to_owned();
            if body.trim().is_empty() {
                return Ok(Err("Give the note's text.".to_owned()));
            }
            match arguments.get("id") {
                Some(Value::Null) | None => {
                    let paper = arguments.get("paper").cloned().map(serde_json::from_value::<Paper>);
                    match paper.transpose() {
                        Ok(paper) => Ok(Ask::Edit(NoteEdit::Create { body, paper: paper.unwrap_or_default() })),
                        Err(_) => Err("Paper is plain, yellow, blue, pink or green.".to_owned()),
                    }
                }
                Some(_) => id().map(|id| Ask::Edit(NoteEdit::Revise { id, body, base: None })),
            }
        }
        NOTES_CHECK_TOOL => {
            let item = arguments.get("item").and_then(Value::as_u64).filter(|n| *n >= 1);
            let done = arguments.get("done").and_then(Value::as_bool);
            match (id(), item, done) {
                (Ok(id), Some(item), Some(done)) => {
                    Ok(Ask::Edit(NoteEdit::Check { id, item: item as usize - 1, done }))
                }
                (Err(problem), ..) => Err(problem),
                _ => Err("Give the item (from 1) and whether it is done.".to_owned()),
            }
        }
        _ => return Err(loopback::unknown_tool(name)),
    })
}

fn listing(book: &NoteBook) -> String {
    if book.notes.is_empty() {
        return "No notes yet.".to_owned();
    }
    book.notes.iter().map(summary).collect::<Vec<_>>().join("\n")
}

fn summary(note: &Note) -> String {
    let (done, items) = note.lines().iter().fold((0, 0), |(done, items), line| match line {
        Line::Check { done: true, .. } => (done + 1, items + 1),
        Line::Check { .. } => (done, items + 1),
        Line::Text(_) => (done, items),
    });
    let progress = if items > 0 { format!(" ({done} of {items} done)") } else { String::new() };
    format!("#{} {}{progress}", note.id.0, note.title())
}

fn report(event: &ServerEvent) -> (String, bool) {
    match event {
        ServerEvent::Notes { created, refused, .. } => match (created.first(), refused.first()) {
            (Some(id), _) => (format!("Created note #{}, linked to this session.", id.0), false),
            (None, Some(id)) => (format!("Note #{} does not exist, or has no such item.", id.0), true),
            (None, None) => ("Saved.".to_owned(), false),
        },
        ServerEvent::Error { message } => (message.clone(), true),
        _ => ("Nothing changed.".to_owned(), true),
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
