use std::sync::atomic::AtomicBool;

use serde_json::{Value, json};

use crate::tools::{Access, Consult, Outcome, Tool, Workspace};
use crate::model::{ModelApi, ToolSpec, TurnRequest};
use crate::transcript::{Block, Message, Role};

pub const ADVISOR_TOOL: &str = "Advisor";

const RESULT_CHARS: usize = 4000;

#[derive(Debug, Clone, Default)]
pub struct Advisor;

impl Tool for Advisor {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: ADVISOR_TOOL.into(),
            description: crate::tools::describe("Advisor", &serde_json::Value::Null),
            schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, _input: &Value) -> String {
        String::new()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("The advisor can only be consulted by the agent loop.")
    }

    fn offered(&self, workspace: &Workspace) -> bool {
        workspace.advisor.is_some()
    }

    fn serial(&self) -> bool {
        true
    }

    fn consult(&self, _input: &Value, workspace: &Workspace, agent: &Consult<'_>) -> Option<Outcome> {
        Some(match &workspace.advisor {
            Some(model) => consult(agent.api, agent.request, model, agent.conversation, agent.stop),
            None => Outcome::error("No advisor is configured."),
        })
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(Advisor)]
}

pub fn transcript(history: &[Message]) -> String {
    let mut text = String::new();
    for message in history {
        let who = if message.role == Role::User { "User" } else { "Agent" };
        for block in &message.blocks {
            let line = match block {
                Block::Text(words) if !words.trim().is_empty() => format!("{who}: {}", words.trim()),
                Block::ToolUse { name, input, .. } => format!("Agent calls {name}: {input}"),
                Block::ToolResult { content, is_error, .. } | Block::ToolResultRich { content, is_error, .. } => {
                    let clipped: String = content.chars().take(RESULT_CHARS).collect();
                    let cut = if clipped.len() < content.len() { " […]" } else { "" };
                    format!("Result{}: {clipped}{cut}", if *is_error { " (error)" } else { "" })
                }
                Block::Image(_) => format!("{who}: [an image]"),
                _ => continue,
            };
            text.push_str(&line);
            text.push('\n');
        }
    }
    text
}

pub fn consult(
    api: &dyn ModelApi,
    template: &TurnRequest,
    model: &str,
    history: &[Message],
    stop: &AtomicBool,
) -> Outcome {
    let request = TurnRequest {
        model: model.into(),
        system: crate::prompt::text("advisor-system"),
        messages: vec![Message::user(format!("<transcript>\n{}</transcript>", transcript(history)))],
        tools: Vec::new(),
        effort: domain::Effort::Max,
        ..template.clone()
    };
    match api.turn(&request, &mut |_| {}, stop) {
        Ok(reply) if !reply.message.text().trim().is_empty() => Outcome::ok(reply.message.text().trim().to_owned()),
        Ok(_) => Outcome::error("The advisor gave no advice; go on with your best judgement."),
        Err(error) => {
            Outcome::error(format!("The advisor could not be reached ({error}); go on with your best judgement."))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use domain::Effort;

    use super::*;
    use crate::model::scripted::ScriptedModel;

    #[test]
    fn the_advisor_reads_the_whole_conversation_at_its_deepest() {
        let history = vec![
            Message::user("fix the parser"),
            Message::assistant(vec![
                Block::Thinking { text: "hidden".into(), signature: None, origin: None },
                Block::ToolUse { id: "1".into(), name: "Read".into(), input: json!({"file_path": "p.rs"}) },
            ]),
            Message {
                role: Role::User,
                blocks: vec![Block::ToolResult { id: "1".into(), content: "x".repeat(5000), is_error: false }],
            },
        ];
        let model = ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Check the lexer first.".into())])]);
        let template = TurnRequest {
            model: "m".into(),
            system: "s".into(),
            messages: Vec::new(),
            tools: Vec::new(),
            effort: Effort::Low,
            max_tokens: 1000,
        };
        let outcome = consult(&model, &template, "strong", &history, &AtomicBool::new(false));
        assert_eq!(outcome.content, "Check the lexer first.");
        let asked = &model.asked()[0];
        assert_eq!((asked.model.as_str(), asked.effort), ("strong", Effort::Max));
        let text = asked.messages[0].text();
        assert!(text.contains("User: fix the parser\nAgent calls Read: {\"file_path\":\"p.rs\"}\nResult: "));
        assert!(text.contains(" […]") && !text.contains("hidden"));
        assert!(asked.system.starts_with("You are reviewing an agent's work"));
    }

    #[test]
    fn the_agent_consults_the_advisor_only_when_one_is_configured() {
        use std::sync::Arc;

        use crate::agent::Agent;
        use crate::control::Control;

        let mut workspace = Workspace::new("/tmp");
        assert!(!Advisor.offered(&workspace), "no advisor model, no tool");
        workspace.advisor = Some("strong".into());
        assert!(Advisor.offered(&workspace) && Advisor.serial());
        let model = ScriptedModel::new([
            ScriptedModel::reply(vec![Block::ToolUse { id: "a1".into(), name: ADVISOR_TOOL.into(), input: json!({}) }]),
            ScriptedModel::reply(vec![Block::Text("Check the lexer first.".into())]),
            ScriptedModel::reply(vec![Block::Text("Done.".into())]),
        ]);
        let tools = tools();
        let control = Control::new(domain::PermissionMode::Bypass, Arc::new(AtomicBool::new(false)));
        let request = TurnRequest {
            model: "m".into(),
            system: "s".into(),
            messages: Vec::new(),
            tools: Agent::specs(&tools),
            effort: Effort::Low,
            max_tokens: 1000,
        };
        let agent = Agent {
            api: &model,
            tools: &tools,
            workspace: &workspace,
            control: &control,
            emit: &|_| {},
            request,
            agents: &[],
            depth: 0,
            turn_limit: 10,
        };
        let mut history = vec![Message::user("fix the parser")];
        agent.run(&mut history).unwrap_or_else(|_| panic!("run"));
        let asked = model.asked();
        assert_eq!(asked[1].model, "strong", "the advisor's own model");
        let result = &asked[2].messages.last().unwrap_or_else(|| panic!("result")).blocks[0];
        assert!(matches!(result, Block::ToolResult { content, is_error: false, .. } if content == "Check the lexer first."));
    }
}
