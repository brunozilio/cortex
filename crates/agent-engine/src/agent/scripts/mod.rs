use std::cell::{Cell, RefCell};

use domain::permission_rules::RuleKind;
use serde_json::{Value, json};

use super::Agent;
use super::dispatch::Calling;
use super::run::ThisRun;
use crate::model::ToolSpec;
use crate::tools::script_bridge::hosted;
use crate::tools::script_tool::{Capabilities, Capability, Origin, ScriptTool};
use crate::tools::skill::FoundSkill;
use crate::tools::{Outcome, Tool, Workspace};
use crate::transcript::{Block, Message};

mod host_calls;

use host_calls::HostCall;

const DEEPEST: usize = 8;

struct Script<'a> {
    name: &'a str,
    path: &'a str,
    source: &'a str,
    origin: Origin,
    capabilities: Capabilities,
}

#[derive(Clone, Copy)]
struct Within<'a> {
    id: &'a str,
    parent: &'a [Message],
    run: &'a ThisRun,
}

impl<'a> Within<'a> {
    fn the_call(calling: &Calling<'a, '_>) -> Self {
        Self { id: calling.id, parent: calling.parent, run: calling.run }
    }
}

thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

struct ScriptCall<'a, 'b> {
    agent: &'a Agent<'b>,
    tool: &'a ScriptTool,
    within: Within<'a>,
}

impl Tool for ScriptCall<'_, '_> {
    fn spec(&self) -> ToolSpec {
        self.tool.spec.clone()
    }

    fn effects(&self, input: &Value, workspace: &Workspace) -> Vec<crate::tools::Effect> {
        self.tool.effects(input, workspace)
    }

    fn summary(&self, input: &Value) -> String {
        self.tool.summary(input)
    }

    fn run(&self, input: &Value, _workspace: &Workspace) -> Outcome {
        self.agent.run_script(self.within, self.tool, input)
    }
}

fn answered(block: Block) -> Result<Value, String> {
    match block {
        Block::ToolResult { content, is_error: false, .. } | Block::ToolResultRich { content, is_error: false, .. } => {
            Ok(Value::String(content))
        }
        Block::ToolResult { content, .. } | Block::ToolResultRich { content, .. } => Err(content),
        other => Ok(json!(format!("{other:?}"))),
    }
}

fn reported(result: Result<Value, String>, last: Option<Outcome>, stopped: bool, name: &str) -> Outcome {
    match (result, last) {
        (Ok(Value::String(text)), Some(outcome)) if text == outcome.content => outcome,
        (Err(_), Some(outcome)) if stopped => outcome,
        (Ok(Value::String(text)), _) => Outcome::ok(text),
        (Ok(Value::Null), _) => Outcome::ok(""),
        (Ok(value), _) => Outcome::ok(value.to_string()),
        (Err(error), _) => Outcome::error(format!("{name} failed: {error}")),
    }
}

impl Agent<'_> {
    pub(in crate::agent) fn script_call(
        &self,
        calling: &Calling<'_, '_>,
        tool: &ScriptTool,
        summary: &str,
        input: &Value,
    ) -> Outcome {
        let call = ScriptCall { agent: self, tool, within: Within::the_call(calling) };
        let approved = tool.trusted || self.approved(tool, summary, input);
        let permission = if approved { calling.permission } else { Some(RuleKind::Ask) };
        self.permitted(calling.id, &call, summary, input, permission)
    }

    fn approved(&self, tool: &dyn Tool, summary: &str, input: &Value) -> bool {
        let (name, effects) = (tool.spec().name, tool.effects(input, self.workspace));
        let called = crate::agent::permission::Called { name: &name, subject: summary, input, effects: &effects };
        matches!(self.rule_for(&called), Some((RuleKind::Allow, _)))
    }

    pub(in crate::agent) fn project_skill<'t>(&self, tool: &'t dyn Tool, input: &Value) -> Option<&'t FoundSkill> {
        let name = input["skill"].as_str()?.trim_start_matches('/');
        tool.skills().iter().find(|skill| skill.name == name && skill.project_script())
    }

    pub(in crate::agent) fn skill_call(
        &self,
        calling: &Calling<'_, '_>,
        tool: &dyn Tool,
        skill: &FoundSkill,
        summary: &str,
        input: &Value,
    ) -> Outcome {
        let approved = self.approved(tool, summary, input);
        let shown = format!("{summary}, the project's script {}", skill.file.display());
        let permission = if approved { calling.permission } else { Some(RuleKind::Ask) };
        self.permitted_as(calling.id, tool, summary, Some(&shown), input, permission)
    }

    fn script_context(&self) -> Value {
        let skills: Vec<Value> = self
            .tools
            .iter()
            .flat_map(|tool| tool.skills())
            .map(|skill| json!({ "name": skill.name, "description": skill.description, "path": skill.file.display().to_string() }))
            .collect();
        json!({
            "cwd": self.workspace.cwd.display().to_string(),
            "model": self.request.model,
            "mode": format!("{:?}", self.control.mode()).to_ascii_lowercase(),
            "session": self.workspace.session_id,
            "tools": self.tools.iter().map(|tool| tool.spec().name).collect::<Vec<_>>(),
            "skills": skills,
            "agents": self.agents.iter().map(|agent| json!({ "name": agent.name, "description": agent.description })).collect::<Vec<_>>(),
        })
    }

    fn run_script(&self, within: Within<'_>, tool: &ScriptTool, input: &Value) -> Outcome {
        let path = tool.path.display().to_string();
        let origin = Origin::trusted(tool.trusted);
        let capabilities = tool.capabilities.clone();
        let script = Script { name: &tool.spec.name, path: &path, source: &tool.source, origin, capabilities };
        self.run_js(within, &script, input, None)
    }

    pub(in crate::agent) fn execute_declared(
        &self,
        calling: &Calling<'_, '_>,
        input: &Value,
        builtin: &dyn Fn(&Value) -> Outcome,
    ) -> Outcome {
        let name = calling.name;
        let path = crate::tools::declared_path(name);
        let Some(source) = content::source(&path) else { return builtin(input) };
        let spec = content::value(&path, "spec").unwrap_or_default();
        let capabilities = Capabilities::read(&spec).unwrap_or_default();
        let script = Script { name, path: &path, source: &source, origin: Origin::Owner, capabilities };
        self.run_js(Within::the_call(calling), &script, input, Some(builtin))
    }

    fn run_js(
        &self,
        within: Within<'_>,
        script: &Script<'_>,
        input: &Value,
        builtin: Option<&dyn Fn(&Value) -> Outcome>,
    ) -> Outcome {
        let depth = DEPTH.with(Cell::get);
        if depth >= DEEPEST {
            return Outcome::error(format!("{} called tools {DEEPEST} levels deep; stopped", script.name));
        }
        let data = self.script_context();
        let stop = self.workspace.stop.flag();
        let export = if script::exports(script.path, script.source, "execute") { "execute" } else { "run" };
        let (path, source, input) = (script.path.to_owned(), script.source.to_owned(), input.clone());
        let last: RefCell<Option<Outcome>> = RefCell::new(None);
        DEPTH.with(|cell| cell.set(depth + 1));
        let result = hosted(
            "the script crashed",
            move |host| script::run(&path, &source, export, &[input], &data, host, stop),
            |function, arguments| match function {
                "builtin" => built_in(builtin, &arguments, &last, script.name),
                function => self.serve(within, script, function, arguments),
            },
        );
        DEPTH.with(|cell| cell.set(depth));
        let stopped = self.workspace.stop.load(std::sync::atomic::Ordering::Relaxed);
        reported(result, last.into_inner(), stopped, script.name)
    }

    fn serve(
        &self,
        within: Within<'_>,
        script: &Script<'_>,
        function: &str,
        arguments: Value,
    ) -> Result<Value, String> {
        if let Some(capability) = Capability::of(function) {
            return self.host_call(within, script, capability, &arguments);
        }
        match function {
            "tool" => {
                let name = arguments.get("name").and_then(Value::as_str).unwrap_or_default();
                let id = format!("script-{}", self.control.next_agent());
                let spawn = |job: Box<dyn FnOnce() + Send + '_>| job();
                answered(self.call(&id, name, &arguments["input"], within.run, within.parent, &spawn))
            }
            "llm" => self.script_llm(&arguments),
            "text" => Ok(Value::String(content::text(arguments.as_str().unwrap_or_default()))),
            "log" => {
                let text = arguments.as_str().map_or_else(|| arguments.to_string(), str::to_owned);
                (self.emit)(domain::EngineEvent::ToolLog { id: within.id.to_owned(), text });
                Ok(Value::Null)
            }
            other => Err(format!("ctx.{other} is not a function")),
        }
    }

    fn host_call(
        &self,
        within: Within<'_>,
        script: &Script<'_>,
        capability: Capability,
        arguments: &Value,
    ) -> Result<Value, String> {
        script.capabilities.allows(capability, script.name)?;
        capability.granted(self.workspace.policy.scripts.of(script.origin))?;
        let call = HostCall { capability, script: script.name, origin: script.origin };
        let id = format!("{}-{}-{}", within.id, capability.name(), self.control.next_agent());
        let (subject, shown) = (call.subject(arguments), call.shown(arguments));
        HostCall::answer(self.permitted_as(&id, &call, &subject, Some(&shown), arguments, None))
    }

    fn script_llm(&self, arguments: &Value) -> Result<Value, String> {
        crate::tools::script_bridge::ask_model(self.api, &self.request, arguments, self.control.stop_flag())
    }
}

fn built_in(
    builtin: Option<&dyn Fn(&Value) -> Outcome>,
    arguments: &Value,
    last: &RefCell<Option<Outcome>>,
    name: &str,
) -> Result<Value, String> {
    let builtin = builtin.ok_or_else(|| format!("{name} has no built-in to call"))?;
    let outcome = builtin(arguments);
    let content = Value::String(outcome.content.clone());
    last.replace(Some(outcome));
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_s_answer_keeps_the_built_in_s_outcome_when_it_passed_it_through() {
        let built = Outcome::ok("listing").showing("3 files");
        let same = reported(Ok(json!("listing")), Some(built.clone()), false, "Glob");
        assert_eq!(same, built, "what the built-in showed survives the script");
        let rewritten = reported(Ok(json!("2 files")), Some(built.clone()), false, "Glob");
        assert_eq!(rewritten, Outcome::ok("2 files"));
        assert_eq!(reported(Ok(Value::Null), None, false, "Glob"), Outcome::ok(""));
        assert_eq!(reported(Ok(json!({"n": 1})), None, false, "Glob"), Outcome::ok(r#"{"n":1}"#));
        assert_eq!(reported(Err("boom".into()), Some(built.clone()), true, "Glob"), built, "stopped mid-way");
        assert_eq!(reported(Err("boom".into()), Some(built), false, "Glob"), Outcome::error("Glob failed: boom"));
    }
}
