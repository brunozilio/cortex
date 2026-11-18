use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use platform::sync::Locked;
use serde_json::Value;

use super::Agent;
use super::subagents::background::Background;
use super::subagents::conversations::Conversations;
use crate::tools::Tool;

#[derive(Default)]
pub(in crate::agent) struct ThisRun {
    pub(in crate::agent) tools: ToolIndex,
    pub(in crate::agent) background: Background,
    pub(in crate::agent) conversations: Conversations,
    pub(in crate::agent) repeats: Mutex<domain::repeats::Repeats>,
    pub(in crate::agent) watches: Watches,
    pub(in crate::agent) stuck: StuckWatch,
    pub(in crate::agent) keep: Option<super::Keep>,
    pub(in crate::agent) watching: std::sync::Arc<super::observers::Watching>,
}

impl ThisRun {
    pub(in crate::agent) fn transition(&self, history: &[crate::transcript::Message]) {
        if let Some(keep) = &self.keep {
            keep(history);
        }
    }

    pub(in crate::agent) fn admit(&self, name: &str, input: &Value, may_change: bool) -> bool {
        self.repeats.locked().admit(name, &input.to_string(), may_change)
    }
}

#[derive(Default)]
pub(in crate::agent) struct ToolIndex(OnceLock<HashMap<String, usize>>);

impl ToolIndex {
    pub(in crate::agent) fn find<'t>(&self, tools: &'t [Box<dyn Tool>], name: &str) -> Option<&'t dyn Tool> {
        let at = self.0.get_or_init(|| by_name(tools)).get(name)?;
        tools.get(*at).map(Box::as_ref)
    }
}

fn by_name(tools: &[Box<dyn Tool>]) -> HashMap<String, usize> {
    let mut at = HashMap::with_capacity(tools.len());
    for (position, tool) in tools.iter().enumerate() {
        at.entry(tool.spec().name).or_insert(position);
    }
    at
}

impl<'a> Agent<'a> {
    pub(in crate::agent) fn tool(&self, run: &ThisRun, name: &str) -> Option<&'a dyn Tool> {
        run.tools.find(self.tools, name)
    }
}

#[derive(Default)]
pub(in crate::agent) struct Watches(Mutex<Vec<String>>);

impl Watches {
    pub(in crate::agent) fn started(&self, id: String) {
        self.0.locked().push(id);
    }

    pub(in crate::agent) fn still_running(&self, client: u64) -> Vec<String> {
        let active = crate::tools::watch::active(client);
        let started = self.0.locked().clone();
        started.into_iter().filter(|id| active.iter().any(|watch| watch.id == *id)).collect()
    }
}

#[derive(Default)]
pub(in crate::agent) struct StuckWatch {
    stuck: Mutex<domain::stuck::Stuck>,
    help_due: AtomicBool,
}

impl StuckWatch {
    pub(in crate::agent) fn held_back(&self) {
        if self.stuck.locked().held_back() {
            self.help_due.store(true, Ordering::Relaxed);
        }
    }

    pub(in crate::agent) fn take_due(&self) -> bool {
        self.help_due.swap(false, Ordering::Relaxed)
    }

    pub(in crate::agent) fn failed(&self, name: &str, input: &Value) -> bool {
        self.stuck.locked().failed(name, &input.to_string())
    }

    pub(in crate::agent) fn succeeded(&self, name: &str, input: &Value) {
        self.stuck.locked().succeeded(name, &input.to_string());
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;
    use crate::model::ToolSpec;
    use crate::tools::{Effect, Outcome, Workspace};

    struct Counting {
        name: &'static str,
        described: &'static AtomicUsize,
    }

    impl Tool for Counting {
        fn spec(&self) -> ToolSpec {
            self.described.fetch_add(1, Ordering::Relaxed);
            ToolSpec { name: self.name.into(), description: String::new(), schema: Value::Null }
        }

        fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<Effect> {
            Vec::new()
        }

        fn summary(&self, _input: &Value) -> String {
            self.name.into()
        }

        fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
            Outcome::ok(self.name)
        }
    }

    static DESCRIBED: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn a_tool_is_found_by_name_and_each_one_is_described_once_for_the_whole_run() {
        let tool = |name| Box::new(Counting { name, described: &DESCRIBED }) as Box<dyn Tool>;
        let tools = vec![tool("Read"), tool("Grep"), tool("Read")];
        let index = ToolIndex::default();
        for _ in 0..50 {
            assert!(index.find(&tools, "Grep").is_some_and(|found| found.summary(&Value::Null) == "Grep"));
            assert!(index.find(&tools, "Missing").is_none());
        }
        let first = index.find(&tools, "Read").map(|found| std::ptr::addr_eq(found, tools[0].as_ref()));
        assert_eq!(first, Some(true), "the first of two tools sharing a name wins, as a linear search did");
        assert_eq!(DESCRIBED.load(Ordering::Relaxed), tools.len());
    }

    #[test]
    fn help_falls_due_the_second_time_a_repeat_is_held_back_and_is_taken_once() {
        let watch = StuckWatch::default();
        watch.held_back();
        assert!(!watch.take_due());
        watch.held_back();
        assert!(watch.take_due());
        assert!(!watch.take_due());
    }
}
