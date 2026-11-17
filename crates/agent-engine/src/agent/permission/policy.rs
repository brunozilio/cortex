use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use domain::permission::{Branches, Call, Effect, Judgement, PolicyContext, PolicyEngine, Ruling, Surroundings};
use domain::permission_rules::PermissionRules;
use serde_json::Value;

use crate::agent::Agent;
use crate::tools::names::ToolName;

struct Around<'a, 'b> {
    agent: &'a Agent<'b>,
    tool: &'a str,
    input: &'a Value,
}

impl Surroundings for Around<'_, '_> {
    fn branches(&self) -> Branches {
        branches(&self.agent.workspace.cwd, self.agent.control.stop_flag())
    }

    fn address_concern(&self, _call: &Call<'_>) -> Option<String> {
        let url = self.input["url"].as_str().filter(|_| ToolName::of(self.tool) == ToolName::WebFetch)?;
        self.agent.workspace.tracking.heard().concern(url).map(|why| format!("Check this address first: {why}."))
    }

    fn package_concern(&self, _call: &Call<'_>) -> Option<String> {
        self.agent.package_concerns(self.tool, self.input)
    }

    fn real_path(&self, path: &Path) -> std::path::PathBuf {
        crate::tools::settled(path)
    }
}

fn branches(cwd: &Path, stop: &AtomicBool) -> Branches {
    let git = |args: &[&str]| {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        crate::agent_runtime::git::output(cwd, &args, stop).ok().map(|out| out.trim().to_owned())
    };
    let current = git(&["symbolic-ref", "--short", "HEAD"]);
    let default = git(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .map(|name| name.split_once('/').map_or(name.clone(), |(_, branch)| branch.to_owned()));
    Branches { current, default }
}

pub(in crate::agent) struct Called<'r> {
    pub(in crate::agent) name: &'r str,
    pub(in crate::agent) subject: &'r str,
    pub(in crate::agent) input: &'r Value,
    pub(in crate::agent) effects: &'r [Effect],
}

impl Called<'_> {
    fn call(&self) -> Call<'_> {
        Call { tool: self.name, subject: self.subject, effects: self.effects }
    }
}

impl Agent<'_> {
    pub(super) fn package_concerns(&self, name: &str, input: &Value) -> Option<String> {
        let network = self.workspace.services.network.as_ref().filter(|_| self.workspace.policy.dependency_guard)?;
        let command = input["command"].as_str().filter(|_| ToolName::of(name) == ToolName::Bash)?;
        let found = crate::guard::packages::concerns(&*network.0, command, crate::guard::packages::today());
        (!found.is_empty()).then(|| found.join("; "))
    }

    fn within<T>(&self, called: &Called<'_>, decide: impl FnOnce(&Call<'_>, &PolicyContext<'_>) -> T) -> T {
        let workspace = self.workspace;
        let around = Around { agent: self, tool: called.name, input: called.input };
        let remembered = self.control.remembered();
        let bounds = workspace.bounds();
        let cwd = workspace.cwd.to_string_lossy();
        let home = workspace.home.as_ref().map(|home| home.to_string_lossy().into_owned());
        let missing = workspace.sandbox_refusal();
        let context = PolicyContext {
            mode: self.control.mode(),
            ceiling: self.control.ceiling(),
            rules: &workspace.policy.permissions,
            remembered: &remembered,
            bounds: &bounds,
            cwd: &cwd,
            home: home.as_deref(),
            sandboxed: workspace.sandboxed(),
            sandbox_missing: missing.as_deref(),
            no_prompts: self.control.no_prompts(),
            surroundings: &around,
        };
        decide(&called.call(), &context)
    }

    pub(in crate::agent) fn judged(&self, called: &Called<'_>) -> Judgement {
        self.within(called, PolicyEngine::judge)
    }

    pub(in crate::agent) fn rule_for(&self, called: &Called<'_>) -> Ruling {
        self.within(called, |call, context| domain::permission::rule_for(call, context, false))
    }

    pub(in crate::agent) fn ruling_in(&self, rules: &PermissionRules, called: &Called<'_>) -> Ruling {
        self.within(called, |call, context| domain::permission::ruling_in(rules, call, context.cwd, context.home))
    }
}
