use std::sync::Arc;

use domain::watch::{Plan, Step, Watching};
use serde_json::Value;

use super::in_loop::Handler;
use crate::agent::Agent;
use crate::agent::run::ThisRun;
use crate::tools::{Outcome, Tool};

impl Agent<'_> {
    pub(super) fn watch(&self, id: &str, input: &Value, run: &ThisRun) -> Outcome {
        let mut plan = match Plan::parse(input) {
            Ok(plan) => plan,
            Err(error) => return Outcome::error(error),
        };
        let active = crate::tools::watch::active(self.workspace.credentials.client);
        if let Some(refusal) = domain::watch::duplicate(&plan.tool, &plan.input, &active) {
            return Outcome::error(refusal);
        }
        let name = plan.tool.as_str();
        let Some(target) = self.tool(run, name) else {
            return Outcome::error(format!("There is no tool named {name} to watch."));
        };
        if !self.offers(name) {
            return Outcome::error(format!("Load {name} with ToolSearch before watching it."));
        }
        let Some(kept) = keepable(name, target, crate::tools::changes_nothing(target, &plan.input, self.workspace))
        else {
            return Outcome::error(format!(
                "{name} cannot be watched: Watch calls read-only tools and MCP tools. Use Monitor for a shell command."
            ));
        };
        let model = Some((self.api, &self.request));
        let hook = match crate::tools::watch::before(name, &plan.input, self.workspace, model) {
            Ok(hook) => hook,
            Err(refusal) => return Outcome::error(format!("Nothing is watched. {refusal}")),
        };
        if let Some(updated) = hook.updated_input {
            plan.input = updated;
        }
        let summary = target.summary(&plan.input);
        let shown = format!("{}: {summary}", plan.explained());
        let first = self.permitted_as(id, target, &summary, Some(&shown), &plan.input, hook.permission);
        let first = crate::tools::watch::after(name, &plan.input, first, self.workspace, model);
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
        run.watches.started(watch.clone());
        Outcome::ok(format!(
            "Watching as {watch}: {explained}. Checks run without you; you are told once, when it is met or if the \
             watch gives up, and that event reaches you as a new message even while the chat is idle. End your turn \
             now: do not poll, schedule wakeups or start another watch for the same thing. Anything the user asked \
             you to do once it happens waits for that event; do not do it before. Stop it with KillShell."
        ))
    }
}

fn keepable(name: &str, tool: &dyn Tool, reads: bool) -> Option<Arc<dyn Tool>> {
    if Handler::of(name).run_by_the_loop() {
        return None;
    }
    tool.kept().or_else(|| {
        let standard = reads.then(|| crate::tools::named(name)).flatten()?;
        Some(Arc::from(standard))
    })
}
