use std::sync::Arc;

use domain::watch::{Plan, Step, Watching};
use serde_json::Value;

use super::Agent;
use super::tasks::Tasks;
use crate::tools::watch::WATCH_TOOL;
use crate::tools::{Access, Outcome, Tool};

const RUN_BY_THE_LOOP: [&str; 11] = [
    WATCH_TOOL,
    crate::tools::ask::ASK_TOOL,
    crate::tools::plan_mode::ENTER_PLAN_TOOL,
    crate::tools::plan_mode::EXIT_PLAN_TOOL,
    crate::tools::worktree::ENTER_WORKTREE,
    crate::tools::worktree::EXIT_WORKTREE,
    crate::tools::subagent::AGENT_TOOL,
    crate::tools::send_message::SEND_MESSAGE_TOOL,
    "TaskOutput",
    "TaskStop",
    "TaskList",
];

impl Agent<'_> {
    pub(super) fn watch(&self, id: &str, input: &Value, tasks: &Tasks) -> Outcome {
        let plan = match Plan::parse(input) {
            Ok(plan) => plan,
            Err(error) => return Outcome::error(error),
        };
        let active = crate::tools::watch::active(self.workspace.credentials.client);
        if let Some(refusal) = domain::watch::duplicate(&plan.tool, &plan.input, &active) {
            return Outcome::error(refusal);
        }
        let name = plan.tool.as_str();
        let Some(target) = self.tools.iter().find(|tool| tool.spec().name == name) else {
            return Outcome::error(format!("There is no tool named {name} to watch."));
        };
        if !self.offers(name) {
            return Outcome::error(format!("Load {name} with ToolSearch before watching it."));
        }
        let Some(kept) = keepable(target.as_ref()) else {
            return Outcome::error(format!(
                "{name} cannot be watched: Watch calls read-only tools and MCP tools. Use Monitor for a shell command."
            ));
        };
        let summary = target.summary(&plan.input);
        let shown = format!("{}: {summary}", plan.explained());
        let first = self.permitted_as(id, target.as_ref(), &summary, Some(&shown), &plan.input, None);
        if first.is_error {
            return Outcome::error(format!("The first check failed, so nothing is watched: {}", first.content));
        }
        let mut watching = Watching::new(plan);
        if let step @ Step::Met(_) = watching.observe(Ok(&first.content)) {
            let told = watching.told(&step).unwrap_or_default();
            return Outcome::ok(format!("Already true on the first check, so nothing is watched. {told}"));
        }
        let explained = watching.plan.explained();
        let watch = crate::tools::watch::start(watching, kept, self.workspace);
        tasks.watched(watch.clone());
        Outcome::ok(format!(
            "Watching as {watch}: {explained}. Checks run without you; you are told once, when it is met or if the \
             watch gives up, and that event reaches you as a new message even while the chat is idle. End your turn \
             now: do not poll, schedule wakeups or start another watch for the same thing. Anything the user asked \
             you to do once it happens waits for that event; do not do it before. Stop it with KillShell."
        ))
    }

    fn offers(&self, name: &str) -> bool {
        self.request.tools.iter().any(|spec| spec.name == name)
            || self.workspace.deferred.as_ref().is_some_and(|deferred| deferred.is_loaded(name))
    }
}

fn keepable(tool: &dyn Tool) -> Option<Arc<dyn Tool>> {
    let name = tool.spec().name;
    if RUN_BY_THE_LOOP.contains(&name.as_str()) {
        return None;
    }
    tool.kept().or_else(|| {
        let standard = (tool.access() == Access::Reads).then(|| crate::tools::named(&name)).flatten()?;
        Some(Arc::from(standard))
    })
}
