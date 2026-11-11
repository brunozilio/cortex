use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use super::{CommandOutput, Handler};
use crate::model::{ModelApi, TurnRequest};
use crate::tools::Workspace;
use crate::tools::script_bridge::{Bridge, Request, ask_model, read_only_tool};
use crate::tools::script_io::StandaloneHost;

pub fn script(
    handler: &Handler,
    input: &Value,
    workspace: &Workspace,
    model: Option<(&dyn ModelApi, &TurnRequest)>,
) -> Result<CommandOutput, String> {
    let path = std::path::PathBuf::from(&handler.command);
    let source = std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let hook = Hook {
        shown: path.display().to_string(),
        source,
        input: input.clone(),
        data: serde_json::json!({ "cwd": workspace.cwd, "session": workspace.session_id, "agent": workspace.agent_name }),
        limit: Duration::from_secs(handler.timeout),
    };
    let standalone =
        StandaloneHost { cwd: workspace.cwd.clone(), access: workspace.scripts, stop: Arc::clone(&workspace.stop) };
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

struct Hook {
    shown: String,
    source: String,
    input: Value,
    data: Value,
    limit: Duration,
}

impl Hook {
    fn run(self, host: Arc<dyn script::Host>, workspace: &Workspace) -> Result<Value, String> {
        let stop = Arc::clone(&workspace.stop);
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
    let (send, requests) = mpsc::channel::<Request>();
    let host: Arc<dyn script::Host> = Arc::new(Bridge(Mutex::new(send)));
    let stop = Arc::clone(&workspace.stop);
    std::thread::scope(|scope| {
        let running = scope.spawn(move || {
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
        });
        loop {
            match requests.recv_timeout(Duration::from_millis(25)) {
                Ok(request) => {
                    let answer = match request.function.as_str() {
                        "llm" => ask_model(api, template, &request.arguments, &workspace.stop),
                        "tool" => read_only_tool(&request.arguments, workspace),
                        other => script::Host::call(standalone, other, request.arguments),
                    };
                    let _ = request.reply.send(answer);
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        running.join().unwrap_or_else(|_| Err("the hook script stopped unexpectedly".into()))
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
            .run_with_model("PreToolUse", "Bash", input, &workspace, Some((&model, &template())))
            .expect("ran");
        let asked = model.asked()[0].messages[0].text();
        assert!(asked.contains("Policy: ") && asked.contains("never rm") && asked.contains("rm -rf /"), "{asked}");
        let blocked = decision.blocked.expect("blocked");
        assert!(blocked.contains("stays inside the project"), "{blocked}");
        assert!(blocked.contains("only read-only tools"), "{blocked}");
    }
}
