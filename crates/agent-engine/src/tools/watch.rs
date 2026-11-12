use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use domain::inbox::Notice;
use domain::watch::{LASTS, Step, Watching};
use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace, field, inbox};
use crate::model::ToolSpec;

pub const WATCH_TOOL: &str = "Watch";
const NAP: Duration = Duration::from_millis(200);

#[derive(Debug)]
pub struct Watch;

impl Tool for Watch {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: WATCH_TOOL.into(),
            description: super::describe(WATCH_TOOL, &Value::Null),
            schema: json!({
                "type": "object",
                "properties": {
                    "tool": { "type": "string", "description": "The read-only tool to call on each check, e.g. an MCP tool such as mcp__server__get_issue." },
                    "input": { "type": "object", "description": "The arguments for that tool, as you would call it." },
                    "path": { "type": "string", "description": "Dot path into the JSON the tool returns, e.g. data.fields.status.name; without it the whole result is compared." },
                    "equals": { "description": "End when the value equals this (case ignored); a list ends on any of them.", "anyOf": [{ "type": "string" }, { "type": "array", "items": { "type": "string" } }] },
                    "contains": { "type": "string", "description": "End when the value contains this text (case ignored)." },
                    "every_seconds": { "type": "integer", "minimum": domain::watch::LEAST_EVERY, "maximum": domain::watch::MOST_EVERY, "description": "Seconds between checks (default 300)." },
                    "description": { "type": "string", "description": "What it waits for, in a few words." }
                },
                "required": ["tool"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "description").or(field(input, "tool")).unwrap_or_default().to_owned()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("Watch runs only inside an agent.")
    }
}

struct Entry {
    client: u64,
    stop: Arc<AtomicBool>,
    explained: String,
    tool: String,
    input: Value,
}

fn lock() -> std::sync::MutexGuard<'static, HashMap<String, Entry>> {
    static ALL: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    ALL.get_or_init(Mutex::default).lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn start(watching: Watching, tool: Arc<dyn Tool>, workspace: &Workspace) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = format!("watch_{}", NEXT.fetch_add(1, Ordering::Relaxed));
    let client = workspace.credentials.client;
    let stop = Arc::new(AtomicBool::new(false));
    let workspace = workspace.fork(Arc::clone(&stop));
    let explained = watching.plan.explained();
    let (tool_name, input) = (watching.plan.tool.clone(), watching.plan.input.clone());
    lock().insert(id.clone(), Entry { client, stop: Arc::clone(&stop), explained, tool: tool_name, input });
    let source = format!("watch {id}");
    let thread_id = id.clone();
    std::thread::spawn(move || {
        let told = keep_watching(watching, tool.as_ref(), &workspace, &stop);
        if lock().remove(&thread_id).is_some()
            && let Some(told) = told
        {
            inbox::push(client, Notice::new(source, told));
        }
    });
    id
}

fn keep_watching(mut watching: Watching, tool: &dyn Tool, workspace: &Workspace, stop: &AtomicBool) -> Option<String> {
    let started = Instant::now();
    loop {
        let next = Instant::now() + watching.plan.every;
        while Instant::now() < next {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            std::thread::sleep(NAP.min(next.saturating_duration_since(Instant::now())));
        }
        if started.elapsed() >= LASTS {
            return watching.told(&Step::GaveUp("watches end after 7 days".into()));
        }
        workspace.forget_seen_files();
        let name = watching.plan.tool.as_str();
        let outcome = match before(name, &watching.plan.input, workspace, None) {
            Ok(hook) => {
                let input = hook.updated_input.as_ref().unwrap_or(&watching.plan.input);
                after(name, input, tool.run(input, workspace), workspace, None)
            }
            Err(refusal) => Outcome::error(refusal),
        };
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        let result = if outcome.is_error { Err(outcome.content.as_str()) } else { Ok(outcome.content.as_str()) };
        let step = watching.observe(result);
        if step != Step::Wait {
            return watching.told(&step);
        }
    }
}

type Model<'a> = Option<(&'a dyn crate::model::ModelApi, &'a crate::model::TurnRequest)>;

pub fn before(
    name: &str,
    input: &Value,
    workspace: &Workspace,
    model: Model<'_>,
) -> Result<crate::hooks::Decision, String> {
    let payload = json!({ "tool_name": name, "tool_input": input });
    let blocked = |reason: String| format!("Hook blocked {name}: {reason}");
    let hook = workspace.hooks.run_with_model("PreToolUse", name, payload, workspace, model).map_err(blocked)?;
    match hook.blocked {
        Some(reason) => Err(blocked(reason)),
        None => Ok(hook),
    }
}

pub fn after(name: &str, input: &Value, mut outcome: Outcome, workspace: &Workspace, model: Model<'_>) -> Outcome {
    let event = if outcome.is_error { "PostToolUseFailure" } else { "PostToolUse" };
    let payload = json!({ "tool_name": name, "tool_input": input, "tool_response": outcome.content });
    match workspace.hooks.run_with_model(event, name, payload, workspace, model) {
        Ok(hook) => {
            if let Some(output) = hook.updated_output {
                outcome.content = output.as_str().map(str::to_owned).unwrap_or_else(|| output.to_string());
                outcome.images.clear();
            }
            if let Some(reason) = hook.blocked {
                outcome.is_error = true;
                outcome.content.push_str(&format!("\nHook feedback: {reason}"));
            }
        }
        Err(error) => {
            outcome.is_error = true;
            outcome.content.push_str(&format!("\nHook failed: {error}"));
        }
    }
    outcome
}

pub fn stop(client: u64, id: &str) -> Option<String> {
    let mut all = lock();
    if !all.get(id).is_some_and(|entry| entry.client == client) {
        return None;
    }
    let entry = all.remove(id)?;
    entry.stop.store(true, Ordering::Relaxed);
    Some(entry.explained)
}

pub fn list(client: u64) -> Vec<(String, String)> {
    let mut listed: Vec<_> = lock()
        .iter()
        .filter(|(_, entry)| entry.client == client)
        .map(|(id, entry)| (id.clone(), entry.explained.clone()))
        .collect();
    listed.sort();
    listed
}

pub fn active(client: u64) -> Vec<domain::watch::Active> {
    lock()
        .iter()
        .filter(|(_, entry)| entry.client == client)
        .map(|(id, entry)| domain::watch::Active {
            id: id.clone(),
            tool: entry.tool.clone(),
            input: entry.input.clone(),
        })
        .collect()
}

pub fn forget_client(client: u64) {
    lock().retain(|_, entry| {
        let theirs = entry.client == client;
        if theirs {
            entry.stop.store(true, Ordering::Relaxed);
        }
        !theirs
    });
}

#[cfg(test)]
mod tests;
