use std::sync::{Arc, Mutex, OnceLock};

use domain::code_mode::{Declared, Mode};
use platform::sync::Locked;
use serde_json::Value;

use super::{Access, Outcome, Tool, Workspace};
use crate::model::ToolSpec;

pub use domain::code_mode::{SEARCH as SEARCH_TOOLS, TOOL as REPL_TOOL};

const KEPT: usize = 8;
const HELPERS: &str = "repl/helpers.js";

pub const OUT_OF_REACH: [&str; 5] = [REPL_TOOL, "EnterPlanMode", "ExitPlanMode", "EnterWorktree", "ExitWorktree"];

pub const TOP_LEVEL: [&str; 6] =
    [REPL_TOOL, "AskUserQuestion", "EnterPlanMode", "ExitPlanMode", "EnterWorktree", "ExitWorktree"];

#[derive(Debug)]
pub struct Repl;

impl Tool for Repl {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared(REPL_TOOL, &Value::Null)
    }

    fn access(&self) -> Access {
        Access::Reads
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

type Contexts = Mutex<Vec<(String, Arc<script::session::Session>)>>;

pub fn session(id: &str) -> Arc<script::session::Session> {
    static ALL: OnceLock<Contexts> = OnceLock::new();
    let mut all = ALL.get_or_init(Mutex::default).locked();
    if let Some(at) = all.iter().position(|(owner, _)| owner == id) {
        let found = all.remove(at);
        let session = Arc::clone(&found.1);
        all.push(found);
        return session;
    }
    let helpers = content::source(HELPERS).unwrap_or_default();
    let session = Arc::new(script::session::Session::with_helpers(helpers));
    all.push((id.to_owned(), Arc::clone(&session)));
    if all.len() > KEPT {
        all.remove(0);
    }
    session
}

pub fn reachable(name: &str) -> bool {
    !OUT_OF_REACH.contains(&name)
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
    let catalog = domain::code_mode::declarations(&declared);
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
    fn one_session_keeps_one_context_and_the_oldest_ones_are_let_go() {
        let first = session("repl-test-a");
        assert!(Arc::ptr_eq(&first, &session("repl-test-a")));
        for n in 0..KEPT {
            session(&format!("repl-test-other-{n}"));
        }
        assert!(!Arc::ptr_eq(&first, &session("repl-test-a")), "a context unused for longest is let go");
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
        assert_eq!(names, [REPL_TOOL, "AskUserQuestion"]);
        let repl = &shown[0].description;
        assert!(repl.starts_with("REPL does things."), "{repl}");
        assert!(repl.contains("declare function Bash(") && repl.contains("declare function Read("), "{repl}");
        assert!(!repl.contains("declare function AskUserQuestion("), "{repl}");
        assert!(!repl.contains("searchTools"), "nothing more to find: {repl}");
        assert!(only(vec![spec("Read"), spec(REPL_TOOL)], true)[0].description.contains("searchTools"));
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
