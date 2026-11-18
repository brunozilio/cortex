use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use domain::code_mode::{Declared, Mode};
use platform::sync::Locked;
use serde_json::Value;

use super::{Outcome, Tool, Workspace};
use crate::model::ToolSpec;

pub use domain::code_mode::{DESCRIBE as DESCRIBE_TOOL, READ_TEXT, SEARCH as SEARCH_TOOLS, TOOL as REPL_TOOL};

const KEPT: usize = 8;
const IDLE: Duration = Duration::from_secs(600);
const HELPERS: &str = "repl/helpers.js";

pub const OUT_OF_REACH: [&str; 5] = [REPL_TOOL, "EnterPlanMode", "ExitPlanMode", "EnterWorktree", "ExitWorktree"];

pub const TOP_LEVEL: [&str; 5] = [REPL_TOOL, "EnterPlanMode", "ExitPlanMode", "EnterWorktree", "ExitWorktree"];

#[derive(Debug)]
pub struct Repl;

impl Tool for Repl {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared(REPL_TOOL, &Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<super::Effect> {
        Vec::new()
    }

    fn summary(&self, input: &Value) -> String {
        let described = input["description"].as_str().map(str::trim).filter(|text| !text.is_empty());
        let first_line = || input["code"].as_str().unwrap_or_default().lines().next().unwrap_or_default().trim();
        described.unwrap_or_else(first_line).to_owned()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("REPL runs only inside an agent's turn.")
    }

    fn offered(&self, workspace: &Workspace) -> bool {
        workspace.policy.code_mode != Mode::Off
    }

    fn serial(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct Context {
    pub session: Arc<script::session::Session>,
    pub started_now: bool,
}

#[derive(Debug, Default)]
struct Contexts(Vec<(String, Arc<script::session::Session>, Instant)>);

impl Contexts {
    fn get(&mut self, id: &str, now: Instant, start: impl FnOnce() -> script::session::Session) -> Context {
        if let Some(at) = self.0.iter().position(|(owner, ..)| owner == id) {
            let (owner, session, _) = self.0.remove(at);
            self.0.push((owner, Arc::clone(&session), now));
            return Context { session, started_now: false };
        }
        let session = Arc::new(start());
        self.0.push((id.to_owned(), Arc::clone(&session), now));
        while self.0.len() > KEPT && self.0.first().is_some_and(|(.., used)| now.duration_since(*used) >= IDLE) {
            self.0.remove(0);
        }
        Context { session, started_now: true }
    }
}

pub fn session(id: &str) -> Context {
    static ALL: OnceLock<Mutex<Contexts>> = OnceLock::new();
    let mut all = ALL.get_or_init(Mutex::default).locked();
    all.get(id, Instant::now(), || script::session::Session::with_helpers(helpers()))
}

pub fn helpers() -> String {
    content::source(HELPERS).unwrap_or_default()
}

pub fn reachable(name: &str) -> bool {
    !OUT_OF_REACH.contains(&name)
}

pub fn within_reach(offered: &[ToolSpec]) -> Vec<String> {
    let mut names: Vec<String> = offered.iter().map(|spec| spec.name.clone()).collect();
    for spec in offered.iter().filter(|spec| spec.name == REPL_TOOL) {
        for line in spec.description.lines() {
            let declared = line.strip_prefix("declare function ").and_then(|rest| rest.split('(').next());
            let declared = declared.map(|name| name.trim_start_matches("tools[\"").trim_end_matches("\"]"));
            names.extend(declared.map(str::to_owned));
        }
    }
    names
}

pub fn only(offered: Vec<ToolSpec>, more_to_find: bool) -> Vec<ToolSpec> {
    if !offered.iter().any(|spec| spec.name == REPL_TOOL) {
        return offered;
    }
    let (kept, scripted): (Vec<ToolSpec>, Vec<ToolSpec>) =
        offered.into_iter().partition(|spec| TOP_LEVEL.contains(&spec.name.as_str()));
    let declared: Vec<Declared<'_>> = scripted
        .iter()
        .map(|spec| Declared { name: &spec.name, description: &spec.description, schema: &spec.schema })
        .collect();
    let catalog = format!("{}\n\n{}", domain::code_mode::RESULT_TYPES, domain::code_mode::declarations(&declared));
    let introduction = content::text("repl/only.md");
    let finding = if more_to_find { format!("\n\n{}", content::text("repl/search.md")) } else { String::new() };
    kept.into_iter()
        .map(|mut spec| {
            if spec.name == REPL_TOOL {
                spec.description = format!("{}\n\n{introduction}{finding}\n\n```ts\n{catalog}\n```", spec.description);
            }
            spec
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;

    fn spec(name: &str) -> ToolSpec {
        ToolSpec { name: name.into(), description: format!("{name} does things."), schema: json!({ "type": "object" }) }
    }

    #[test]
    fn the_summary_is_the_description_or_else_the_first_line_of_code() {
        assert_eq!(Repl.summary(&json!({ "code": "x", "description": " Count TODOs " })), "Count TODOs");
        assert_eq!(Repl.summary(&json!({ "code": "const a = 1\nreturn a" })), "const a = 1");
    }

    #[test]
    fn one_session_keeps_one_context_and_only_long_idle_ones_are_let_go() {
        let mut contexts = Contexts::default();
        let start = Instant::now();
        let first = contexts.get("a", start, script::session::Session::start);
        assert!(first.started_now);
        let again = contexts.get("a", start, script::session::Session::start);
        assert!(Arc::ptr_eq(&first.session, &again.session) && !again.started_now);
        for n in 0..KEPT {
            contexts.get(&format!("other-{n}"), start, script::session::Session::start);
        }
        let busy = contexts.get("a", start, script::session::Session::start);
        assert!(Arc::ptr_eq(&first.session, &busy.session), "a context in use is never let go, however many there are");
        let later = start + IDLE;
        for n in 0..KEPT {
            contexts.get(&format!("later-{n}"), later, script::session::Session::start);
        }
        let renewed = contexts.get("a", later, script::session::Session::start);
        assert!(!Arc::ptr_eq(&first.session, &renewed.session), "past the limit, a long idle context is let go");
        assert!(renewed.started_now);
    }

    #[test]
    fn a_script_cannot_reach_the_tools_that_change_the_turn_itself() {
        assert!(!reachable("REPL") && !reachable("ExitPlanMode"));
        assert!(reachable("Read") && reachable("mcp__notes__find") && reachable("AskUserQuestion"));
    }

    #[test]
    fn in_code_only_mode_the_scripted_tools_move_into_the_repl_description() {
        let offered = vec![spec("Read"), spec(REPL_TOOL), spec("AskUserQuestion"), spec("Bash")];

        let shown = only(offered, false);

        let names: Vec<&str> = shown.iter().map(|spec| spec.name.as_str()).collect();
        assert_eq!(names, [REPL_TOOL]);
        let repl = &shown[0].description;
        assert!(repl.starts_with("REPL does things."), "{repl}");
        assert!(repl.contains("declare function Bash(") && repl.contains("declare function Read("), "{repl}");
        assert!(repl.contains("declare function AskUserQuestion("), "{repl}");
        assert!(!repl.contains("searchTools"), "nothing more to find: {repl}");
        assert!(only(vec![spec("Read"), spec(REPL_TOOL)], true)[0].description.contains("searchTools"));
    }

    #[test]
    fn the_tools_a_model_can_reach_are_its_own_and_those_its_repl_declares() {
        let offered = vec![spec("Read"), spec(REPL_TOOL), spec("has-dash"), spec("mcp__jira__ping")];
        let shown = only(offered, false);

        let reached = within_reach(&shown);

        for name in [REPL_TOOL, "Read", "has-dash", "mcp__jira__ping"] {
            assert!(reached.contains(&name.to_owned()), "{name} in {reached:?}");
        }
        assert_eq!(within_reach(&[spec("Bash")]), ["Bash"]);
    }

    #[test]
    fn without_the_repl_the_tools_stay_as_they_are() {
        let offered = vec![spec("Read"), spec("Bash")];

        assert_eq!(only(offered.clone(), true), offered);
    }

    #[test]
    fn the_same_tools_always_give_the_same_description() {
        let first = only(vec![spec("Read"), spec(REPL_TOOL), spec("Bash")], false);
        let again = only(vec![spec("Bash"), spec(REPL_TOOL), spec("Read")], false);

        assert_eq!(first[0].description, again[0].description, "a stable order keeps the prompt cache");
    }
}
