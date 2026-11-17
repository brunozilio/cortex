use std::path::Path;
use std::time::Duration;

use domain::lsp::Query;
use domain::lsp::answer::{describe, explain};
use domain::lsp::diagnostics::{Scope, listed};
use domain::lsp::uri::{path_of, uri_of};
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
        let mut effects: Vec<crate::tools::Effect> = field(input, "file_path")
            .map(|path| crate::tools::Effect::ReadPath(workspace.place(path)))
            .into_iter()
            .collect();
        if let Some(what) = pending_install(input, workspace) {
            let server = server_name(input, workspace).unwrap_or_default();
            effects.push(crate::tools::Effect::Install { server, what });
        }
        effects
    }

    fn summary(&self, input: &Value) -> String {
        let operation = field(input, "operation").unwrap_or_default();
        format!("{operation} {}", field(input, "file_path").unwrap_or_default())
    }

    fn shown(&self, input: &Value, workspace: &Workspace) -> Option<String> {
        let pending = pending_install(input, workspace)?;
        Some(format!("First {pending}\nthen {}", self.summary(input)))
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
const DIAGNOSTICS_WAIT: Duration = Duration::from_secs(10);

fn trusted(workspace: &Workspace) -> bool {
    workspace.policy.trust == domain::trust::Trust::Trusted
}

fn pending_install(input: &Value, workspace: &Workspace) -> Option<String> {
    let lsp = workspace.services.lsp.as_ref().filter(|_| trusted(workspace))?;
    lsp.pending_install(&workspace.resolve(field(input, "file_path")?))
}

fn server_name(input: &Value, workspace: &Workspace) -> Option<String> {
    let lsp = workspace.services.lsp.as_ref()?;
    let file = workspace.resolve(field(input, "file_path")?);
    domain::lsp::server_for(&file, &lsp.specs).map(|(spec, _)| spec.name.clone())
}

fn installing(workspace: &Workspace) -> crate::lsp::Installing {
    if trusted(workspace) {
        crate::lsp::Installing::Allowed
    } else {
        crate::lsp::Installing::Refused("this folder is not trusted (/trust), so nothing is installed for it")
    }
}

fn with_install(installed: Option<String>, answer: String) -> String {
    match installed {
        Some(name) => format!("Installed the {name} language server on first use.\n\n{answer}"),
        None => answer,
    }
}

fn diagnose(lsp: &crate::lsp::Lsp, scope: Scope, input: &Value, workspace: &Workspace) -> Result<String, String> {
    let file = workspace.resolve(field(input, "file_path").ok_or("Missing required parameter `file_path`.")?);
    let named = shown(&file, workspace);
    let text = std::fs::read_to_string(&file).map_err(|error| format!("Cannot read {named}: {error}"))?;
    let (opened, installed) = lsp.open(&file, &workspace.cwd, &workspace.stop, installing(workspace))?;
    let version = opened.server.sync(&file, &opened.language, &text)?;
    let found = opened.server.diagnostics(&file, version, DIAGNOSTICS_WAIT);
    let answer = match scope {
        Scope::File => match found {
            Some(items) => {
                listed(&[(named.clone(), items)]).unwrap_or_else(|| format!("No errors or warnings in {named}."))
            }
            None => format!(
                "The language server said nothing about {named} within {} seconds; it may still be checking. Ask \
                 again shortly.",
                DIAGNOSTICS_WAIT.as_secs()
            ),
        },
        Scope::Workspace => {
            let real = workspace.cwd.canonicalize().unwrap_or_else(|_| workspace.cwd.clone());
            let relative = |path: &Path| {
                let inside = path.strip_prefix(&workspace.cwd).or_else(|_| path.strip_prefix(&real));
                inside.unwrap_or(path).display().to_string()
            };
            let files: Vec<(String, Vec<Value>)> = opened
                .server
                .published()
                .into_iter()
                .filter_map(|(uri, items)| Some((relative(&path_of(&uri)?), items)))
                .collect();
            listed(&files).unwrap_or_else(|| {
                "The language server reports no errors or warnings in the files it has checked.".to_owned()
            })
        }
    };
    Ok(with_install(installed, answer))
}

pub const NOT_TRUSTED: &str = "Language servers do not start in a folder that is not trusted: a server runs the \
                                project's own code (rust-analyzer runs build scripts and procedural macros, \
                                tsserver loads node_modules/typescript). The user can trust this folder with /trust; \
                                until then, read and search the code instead.";

fn ask(input: &Value, workspace: &Workspace) -> Result<String, String> {
    let lsp = workspace.services.lsp.as_ref().ok_or("No language servers are available in this run.")?;
    if !trusted(workspace) {
        return Err(NOT_TRUSTED.to_owned());
    }
    if let Some(scope) = field(input, "operation").and_then(Scope::named) {
        return diagnose(lsp, scope, input, workspace);
    }
    let query = Query::parse(input)?;
    let file = workspace.resolve(&query.file);
    let text =
        std::fs::read_to_string(&file).map_err(|error| format!("Cannot read {}: {error}", shown(&file, workspace)))?;
    let (opened, installed) = lsp.open(&file, &workspace.cwd, &workspace.stop, installing(workspace))?;
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
    Ok(with_install(installed, described))
}

#[cfg(test)]
#[path = "lsp_tests.rs"]
mod tests;
