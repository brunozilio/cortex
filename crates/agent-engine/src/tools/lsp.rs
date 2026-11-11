//! `LSP`: questions about code (definitions, references, symbols, calls)
//! answered by the file's language server.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use domain::lsp::answer::{describe, explain};
use domain::lsp::uri::uri_of;
use domain::lsp::{Operation, Query};

use super::{Access, Outcome, Tool, Workspace, clip, field, shown};
use crate::model::ToolSpec;

#[derive(Debug)]
pub struct Lsp;

impl Tool for Lsp {
    fn spec(&self) -> ToolSpec {
        let operations: Vec<&str> = Operation::NAMES.iter().map(|(name, _)| *name).collect();
        ToolSpec {
            name: "LSP".into(),
            description: "Ask the file's language server about code: `goToDefinition`, `findReferences`, \
                          `goToImplementation`, `hover` (type and docs), `incomingCalls`/`outgoingCalls` (who \
                          calls a function, what it calls) at a position; `documentSymbol` (a file's outline) or \
                          `workspaceSymbol` (find a symbol by name across the project, with `query`). `line` and \
                          `character` count from 1, as Read shows them; point at the symbol's name. Prefer this \
                          over Grep to find where a symbol is defined or used: it follows the language's rules, \
                          not the text. The first question to a project can wait while its server starts."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "operation": { "type": "string", "enum": operations },
                    "file_path": { "type": "string", "description": "The file the question is about." },
                    "line": { "type": "integer", "description": "Line of the symbol, from 1." },
                    "character": { "type": "integer", "description": "Column of the symbol, from 1." },
                    "query": { "type": "string", "description": "The name to find (workspaceSymbol)." }
                },
                "required": ["operation", "file_path"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn reaches_outside(&self, input: &Value, workspace: &Workspace) -> Option<std::path::PathBuf> {
        workspace.outside(field(input, "file_path"))
    }

    fn summary(&self, input: &Value) -> String {
        let operation = field(input, "operation").unwrap_or_default();
        format!("{operation} {}", field(input, "file_path").unwrap_or_default())
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        match ask(input, workspace) {
            Ok(answer) => Outcome::ok(clip(&answer, OUTPUT)),
            Err(error) => Outcome::error(explain(&error)),
        }
    }
}

const OUTPUT: usize = 30_000;
const ANSWER_WAIT: Duration = Duration::from_secs(60);

fn ask(input: &Value, workspace: &Workspace) -> Result<String, String> {
    let lsp = workspace.lsp.as_ref().ok_or("No language servers are available in this run.")?;
    let query = Query::parse(input)?;
    let file = workspace.resolve(&query.file);
    let text =
        std::fs::read_to_string(&file).map_err(|error| format!("Cannot read {}: {error}", shown(&file, workspace)))?;
    let opened = lsp.open(&file, &workspace.cwd, &workspace.stop)?;
    let server = &opened.server;
    server.sync(&file, &opened.language, &text)?;
    let line = text.lines().nth(query.line as usize - 1).unwrap_or_default();
    let (method, params) = query.request(&uri_of(&file), line);
    let mut answer = server.request(method, params, &workspace.stop, ANSWER_WAIT)?;
    if let Some((method, params)) = query.follow_up(&answer) {
        answer = server.request(method, params, &workspace.stop, ANSWER_WAIT)?;
    }
    let lines = |path: &Path, number: u32| {
        let read = if path == file { Some(text.clone()) } else { std::fs::read_to_string(path).ok() };
        read?.lines().nth(number as usize).map(str::to_owned)
    };
    let real = workspace.cwd.canonicalize().unwrap_or_else(|_| workspace.cwd.clone());
    Ok(describe(query.operation, &answer, &[&workspace.cwd, &real], lines))
}

#[cfg(test)]
#[path = "lsp_tests.rs"]
mod tests;
