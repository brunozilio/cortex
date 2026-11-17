use serde_json::{Value, json};

use crate::model::ToolSpec;
use crate::tools::script_tool::{Capability, Origin};
use crate::tools::{Effect, Outcome, Tool, Workspace};

pub(super) struct HostCall<'a> {
    pub(super) capability: Capability,
    pub(super) script: &'a str,
    pub(super) origin: Origin,
}

fn text<'v>(arguments: &'v Value, key: &str) -> &'v str {
    arguments.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn read_path(arguments: &Value) -> &str {
    arguments.as_str().unwrap_or_default()
}

impl HostCall<'_> {
    pub(super) fn subject(&self, arguments: &Value) -> String {
        self.capability.subject(arguments)
    }

    pub(super) fn shown(&self, arguments: &Value) -> String {
        format!("{}\n(ctx.{} in the script tool {})", self.subject(arguments), self.capability.name(), self.script)
    }

    pub(super) fn answer(outcome: Outcome) -> Result<Value, String> {
        if outcome.is_error {
            return Err(outcome.content);
        }
        serde_json::from_str(&outcome.content).map_err(|error| format!("the app's answer was not readable: {error}"))
    }

    fn performed(&self, arguments: &Value, workspace: &Workspace) -> Result<Value, String> {
        match self.capability {
            Capability::ReadFile => {
                let path = workspace.resolve(read_path(arguments));
                std::fs::read_to_string(&path)
                    .map(Value::String)
                    .map_err(|error| format!("{}: {error}", path.display()))
            }
            Capability::WriteFile => {
                let path = workspace.resolve(text(arguments, "path"));
                let _not_said_to_a_script = workspace.before_write(&path);
                platform::atomic::write(&path, text(arguments, "text").as_bytes())
                    .map(|_| Value::Null)
                    .map_err(|error| format!("{}: {error}", path.display()))
            }
            Capability::Exec => {
                let confine = workspace.confiner();
                crate::tools::script_io::exec(&workspace.cwd, arguments, &workspace.stop, &confine)
            }
            Capability::Fetch => crate::tools::script_io::fetch(arguments, self.origin, workspace.stop.token()),
        }
    }
}

impl Tool for HostCall<'_> {
    fn spec(&self) -> ToolSpec {
        let name = self.capability.tool();
        ToolSpec { name: name.to_owned(), description: String::new(), schema: json!({ "type": "object" }) }
    }

    fn effects(&self, arguments: &Value, workspace: &Workspace) -> Vec<Effect> {
        self.capability.effects(arguments, &|path| workspace.place(path))
    }

    fn summary(&self, arguments: &Value) -> String {
        self.subject(arguments)
    }

    fn run(&self, arguments: &Value, workspace: &Workspace) -> Outcome {
        match self.performed(arguments, workspace) {
            Ok(value) => Outcome::ok(value.to_string()),
            Err(error) => Outcome::error(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::names;

    fn call(capability: Capability) -> HostCall<'static> {
        HostCall { capability, script: "Runner", origin: Origin::Owner }
    }

    #[test]
    fn each_host_function_is_judged_as_the_tool_that_does_the_same() {
        let workspace = Workspace::new("/repo");
        let exec = call(Capability::Exec);
        assert_eq!(exec.spec().name, names::BASH);
        assert!(matches!(&exec.effects(&json!({ "command": "ls" }), &workspace)[..], [Effect::Exec(_)]));
        let read = call(Capability::ReadFile);
        assert_eq!(read.spec().name, names::READ);
        assert!(
            matches!(&read.effects(&json!("/etc/hosts"), &workspace)[..], [Effect::ReadPath(place)] if place.written == std::path::Path::new("/etc/hosts"))
        );
        let write = call(Capability::WriteFile);
        assert!(matches!(&write.effects(&json!({ "path": "a.txt" }), &workspace)[..], [Effect::WritePath(_)]));
        let fetch = call(Capability::Fetch);
        assert!(
            matches!(&fetch.effects(&json!({ "url": "https://x.dev/a" }), &workspace)[..], [Effect::Net { host: Some(host) }] if host == "x.dev")
        );
        assert!(exec.shown(&json!({ "command": "ls" })).contains("ctx.exec in the script tool Runner"));
    }

    #[test]
    fn an_answer_comes_back_as_the_value_the_script_reads() {
        assert_eq!(HostCall::answer(Outcome::ok("\"text\"")), Ok(json!("text")));
        assert_eq!(HostCall::answer(Outcome::error("no")), Err("no".into()));
    }
}
