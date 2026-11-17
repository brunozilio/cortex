use std::path::Path;
use std::time::Duration;

use domain::lsp::Query;
use domain::lsp::answer::{describe, explain};
use domain::lsp::uri::uri_of;
use serde_json::Value;

use super::{Outcome, Tool, Workspace, clip, field, shown};
use crate::model::ToolSpec;

#[derive(Debug)]
pub struct Lsp;

impl Tool for Lsp {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared("LSP", &Value::Null)
    }

    fn effects(&self, input: &Value, workspace: &Workspace) -> Vec<crate::tools::Effect> {
        field(input, "file_path")
            .map(|path| crate::tools::Effect::ReadPath(workspace.place(path)))
            .into_iter()
            .collect()
    }

    fn summary(&self, input: &Value) -> String {
        let operation = field(input, "operation").unwrap_or_default();
        format!("{operation} {}", field(input, "file_path").unwrap_or_default())
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        match ask(input, workspace) {
            Ok(answer) => Outcome::ok(clip(&answer, OUTPUT)),
            Err(error) => Outcome::error(error),
        }
    }
}

const OUTPUT: usize = 30_000;
const ANSWER_WAIT: Duration = Duration::from_secs(60);

fn ask(input: &Value, workspace: &Workspace) -> Result<String, String> {
    let lsp = workspace.services.lsp.as_ref().ok_or("No language servers are available in this run.")?;
    let query = Query::parse(input)?;
    let file = workspace.resolve(&query.file);
    let text =
        std::fs::read_to_string(&file).map_err(|error| format!("Cannot read {}: {error}", shown(&file, workspace)))?;
    let (opened, installed) = lsp.open(&file, &workspace.cwd, &workspace.stop)?;
    let server = &opened.server;
    server.sync(&file, &opened.language, &text)?;
    let line = text.lines().nth(query.line as usize - 1).unwrap_or_default();
    let (method, params) = query.request(&uri_of(&file), line);
    let mut answer = server.request(method, params, &workspace.stop, ANSWER_WAIT).map_err(|error| explain(&error))?;
    if let Some((method, params)) = query.follow_up(&answer) {
        answer = server.request(method, params, &workspace.stop, ANSWER_WAIT).map_err(|error| explain(&error))?;
    }
    let lines = |path: &Path, number: u32| {
        let read = if path == file { Some(text.clone()) } else { std::fs::read_to_string(path).ok() };
        read?.lines().nth(number as usize).map(str::to_owned)
    };
    let real = workspace.cwd.canonicalize().unwrap_or_else(|_| workspace.cwd.clone());
    let described = describe(query.operation, &answer, &[&workspace.cwd, &real], lines);
    Ok(match installed {
        Some(name) => format!("Installed the {name} language server on first use.\n\n{described}"),
        None => described,
    })
}

#[cfg(test)]
#[path = "lsp_tests.rs"]
mod tests;
