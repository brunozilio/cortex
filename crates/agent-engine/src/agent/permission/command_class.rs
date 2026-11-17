use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use domain::command_policy::{CommandClass, GitFacts, classify, sign_in_refused};
use serde_json::Value;

use crate::agent::Agent;
use crate::tools::names::BASH;

impl Agent<'_> {
    pub(super) fn command_class(&self, tool: &str, input: &Value) -> CommandClass {
        let Some(command) = input["command"].as_str().filter(|_| tool == BASH) else {
            return CommandClass::Unknown;
        };
        let confined = self.workspace.sandboxed() && input["unsandboxed"].as_bool() != Some(true);
        if let Some(why) = sign_in_refused(command, confined) {
            return CommandClass::Forbidden(why);
        }
        let (current, default) = if command.contains("push") {
            branches(&self.workspace.cwd, self.control.stop_flag())
        } else {
            (None, None)
        };
        classify(command, &GitFacts { current_branch: current.as_deref(), default_branch: default.as_deref() })
    }
}

fn branches(cwd: &Path, stop: &AtomicBool) -> (Option<String>, Option<String>) {
    let git = |args: &[&str]| {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        crate::agent_runtime::git::output(cwd, &args, stop).ok().map(|out| out.trim().to_owned())
    };
    let current = git(&["symbolic-ref", "--short", "HEAD"]);
    let default = git(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .map(|name| name.split_once('/').map_or(name.clone(), |(_, branch)| branch.to_owned()));
    (current, default)
}
