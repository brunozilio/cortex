use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::{CommandOutput, Handler};
use crate::model::{ModelApi, TurnRequest};
use crate::tools::Workspace;
use crate::tools::host_judge::Grounds;
use crate::tools::script_bridge::{ask_model, hosted, read_only_tool};
use crate::tools::script_io::StandaloneHost;
use crate::tools::script_tool::{Capabilities, Declaration};

pub fn script(
    handler: &Handler,
    input: &Value,
    workspace: &Workspace,
    model: Option<(&dyn ModelApi, &TurnRequest)>,
) -> Result<CommandOutput, String> {
    let source = super::scripts::source(&handler.command)?;
    let hook = Hook {
        shown: handler.command.clone(),
        source,
        input: input.clone(),
        data: serde_json::json!({ "cwd": workspace.cwd, "session": workspace.session_id, "agent": workspace.agent_name }),
        limit: Duration::from_secs(handler.timeout),
    };
    let capabilities = match declared(&hook.shown, &hook.source) {
        Ok(capabilities) => capabilities,
        Err(error) => return Ok(CommandOutput { code: Some(1), stdout: String::new(), stderr: error }),
    };
    let origin = crate::tools::script_tool::Origin::of_script(&handler.command, workspace.home.as_deref());
    let standalone = StandaloneHost {
        cwd: workspace.cwd.clone(),
        access: workspace.policy.scripts.of(origin),
        origin,
        confine: workspace.confiner(),
        stop: workspace.stop.flag(),
        cancel: workspace.stop.token().clone(),
        script: format!("the hook {}", handler.command),
        capabilities,
        judge: workspace.host_judge(),
        grounds: Grounds::of(workspace),
    };
    let result = match model {
        None => hook.run(Arc::new(standalone), workspace),
        Some((api, template)) => bridged(hook, &standalone, workspace, api, template),
    };
    Ok(match result {
        Ok(Value::Null) => CommandOutput { code: Some(0), stdout: String::new(), stderr: String::new() },
        Ok(Value::String(text)) => CommandOutput { code: Some(0), stdout: text, stderr: String::new() },
        Ok(value) => CommandOutput { code: Some(0), stdout: value.to_string(), stderr: String::new() },
        Err(error) => CommandOutput { code: Some(1), stdout: String::new(), stderr: error },
    })
}

fn declared(shown: &str, source: &str) -> Result<Capabilities, String> {
    let listed = script::exported(shown, source, "capabilities").ok();
    Capabilities::listed(listed.as_ref(), Declaration::Export).map_err(|error| format!("{shown}: {error}"))
}

struct Hook {
    shown: String,
    source: String,
    input: Value,
    data: Value,
    limit: Duration,
}

impl Hook {
    fn run(self, host: Arc<dyn script::Host>, workspace: &Workspace) -> Result<Value, String> {
        let stop = workspace.stop.flag();
        script::run_within(
            &self.shown,
            &self.source,
            "default",
            &[self.input],
            &self.data,
            host,
            stop,
            Some(self.limit),
        )
    }
}

fn bridged(
    hook: Hook,
    standalone: &StandaloneHost,
    workspace: &Workspace,
    api: &dyn ModelApi,
    template: &TurnRequest,
) -> Result<Value, String> {
    let stop = workspace.stop.flag();
    let run = move |host| {
        script::run_within(
            &hook.shown,
            &hook.source,
            "default",
            &[hook.input],
            &hook.data,
            host,
            stop,
            Some(hook.limit),
        )
    };
    hosted("the hook script stopped unexpectedly", run, |function, arguments| match function {
        "llm" => ask_model(api, template, &arguments, &workspace.stop),
        "tool" => read_only_tool(&arguments, workspace),
        other => script::Host::call(standalone, other, arguments),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::model::scripted::ScriptedModel;
    use crate::transcript::Block;

    const HOOK: &str = r#"
export default function (input, ctx) {
  const policy = ctx.tool("Read", { file_path: "policy.txt" });
  const verdict = ctx.llm({ prompt: `Policy: ${policy}\nCommand: ${input.tool_input.command}` });
  let outside = "";
  try { ctx.tool("Read", { file_path: "/etc/hosts" }); } catch (error) { outside = String(error); }
  let editing = "";
  try { ctx.tool("Write", { file_path: "x", content: "y" }); } catch (error) { editing = String(error); }
  return JSON.stringify({ decision: verdict.trim() === "deny" ? "block" : "approve", reason: `${outside} | ${editing}` });
}
"#;

    fn template() -> TurnRequest {
        TurnRequest {
            model: "m".into(),
            system: String::new(),
            messages: Vec::new(),
            tools: Vec::new(),
            effort: domain::Effort::Low,
            max_tokens: 100,
        }
    }

    #[test]
    fn a_script_hook_reads_the_project_and_asks_the_model_through_ctx() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("policy.txt"), "never rm").expect("policy");
        std::fs::write(dir.path().join("hook.js"), HOOK).expect("hook");
        let mut workspace = Workspace::new(dir.path());
        let command = dir.path().join("hook.js").display().to_string();
        workspace.hooks = super::super::Hooks::parse(
            &json!({"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "script", "command": command}]}]}),
        )
        .expect("hooks");
        let model = ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("deny".into())])]);
        let input = json!({"tool_name": "Bash", "tool_input": {"command": "rm -rf /"}});
        let decision = workspace
            .hooks
            .run_with_model(super::super::HookEvent::PreToolUse, "Bash", input, &workspace, Some((&model, &template())))
            .expect("ran");
        let asked = model.asked()[0].messages[0].text();
        assert!(asked.contains("Policy: ") && asked.contains("never rm") && asked.contains("rm -rf /"), "{asked}");
        let blocked = decision.blocked.expect("blocked");
        assert!(blocked.contains("stays inside the project"), "{blocked}");
        assert!(blocked.contains("only read-only tools"), "{blocked}");
    }

    #[test]
    fn every_built_in_hook_declares_the_host_functions_it_uses() {
        for (path, source) in content::builtin_files("hooks/") {
            let source = String::from_utf8_lossy(source);
            let declared = declared(path, &source).expect("a valid capabilities list");
            assert!(
                script::exported(path, &source, "capabilities").is_ok_and(|listed| listed.is_array()),
                "{path} says what it uses, even when it is nothing"
            );
            for capability in crate::tools::script_tool::Capability::ALL {
                let named = format!("ctx.{}(", capability.name());
                assert!(!source.contains(&named) || declared.declares(capability), "{path} uses {named}");
            }
        }
    }

    #[test]
    fn a_script_hook_that_runs_out_of_time_fails_alone_and_the_run_goes_on() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("slow.js"), "export default function () { for (;;) {} }").expect("hook");
        let mut workspace = Workspace::new(dir.path());
        let command = dir.path().join("slow.js").display().to_string();
        workspace.hooks = super::super::Hooks::parse(&json!({"PreToolUse": [{"matcher": "Bash", "hooks": [
            {"type": "script", "command": command, "timeout": 1}
        ]}]}))
        .expect("hooks");
        let input = json!({"tool_name": "Bash", "tool_input": {"command": "ls"}});
        let model = ScriptedModel::new([]);
        for model in [None, Some((&model as &dyn ModelApi, &template()))] {
            let decision = workspace.hooks.run_with_model(
                super::super::HookEvent::PreToolUse,
                "Bash",
                input.clone(),
                &workspace,
                model,
            );
            assert_eq!(decision.expect("ran").blocked, None, "a failing hook does not block");
            assert!(!workspace.stop.load(std::sync::atomic::Ordering::Relaxed), "the run was not cancelled");
        }
    }
}

#[cfg(test)]
#[path = "script_access_tests.rs"]
mod access_tests;
