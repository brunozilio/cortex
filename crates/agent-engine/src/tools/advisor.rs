use std::sync::atomic::AtomicBool;

use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace};
use crate::model::{ModelApi, ToolSpec, TurnRequest};
use crate::transcript::{Block, Message, Role};

pub const ADVISOR_TOOL: &str = "Advisor";

const RESULT_CHARS: usize = 4000;

pub const GUIDE: &str = "\
# The advisor
You have the Advisor tool, backed by a stronger reviewer model. It takes no parameters: when you call it, your \
whole conversation is forwarded. The advisor sees the task, every tool call you made and every result you saw.

Call the advisor before substantive work: before writing, before committing to an interpretation, before \
building on an assumption. If the task needs orientation first (finding files, fetching a source, seeing what is \
there), do that, then call it. Orientation is not substantive work; writing, editing and declaring an answer are.

Also call it:
- when you believe the task is complete. Before that call, make the deliverable durable: write the file, save \
the result, commit the change. The call takes time, and if the session ends during it, a durable result persists;
- when stuck: errors recurring, an approach not converging, results that do not fit;
- when considering a change of approach.

On tasks longer than a few steps, call it at least once before committing to an approach and once before \
declaring done. On short reactive tasks where the next action follows from tool output you just read, you need \
not keep calling: the advisor adds most of its value on the first call, before the approach settles.

Give the advice serious weight. If you follow a step and it fails in practice, or you have primary evidence \
against a specific claim (the file says X, the documentation says Y), adapt. A passing self-test is not evidence \
the advice is wrong: it is evidence your test does not check what the advice checks.

If data you retrieved points one way and the advisor another, do not switch silently. Raise the conflict in one \
more call: \"I found X, you suggest Y; which constraint breaks the tie?\" A reconciling call is cheaper than \
committing to the wrong branch.";

const ADVISOR: &str = "\
You are reviewing an agent's work in progress on a task. Below is its full transcript: the task, every tool call \
it made, every result it saw, every problem it hit. It asked for your advice without a specific question.

Read the transcript to see where it is:

STARTING OUT (just the task, no work yet)
  Give the right approach: what needs touching, enumerated. It works through what is listed and skims what is \
explained. Flag constraints the task implies but does not state, after the approach, not instead of it. When the \
answer depends on a fact you cannot verify from the transcript, give the search strategy, not a guess. \"Start \
from the tightest constraint and work outward\" is reliable; a fact reconstructed from partial recall is a coin \
flip dressed as an answer.

STUCK (errors recurring in recent turns, cycling between approaches)
  Diagnose what is actually going wrong from what you can see. Do not re-plan: find the specific point of \
failure. Look at what it actually tried, not what you would have tried. If it looped on the same thing several \
times, the fix is not another variation of it.

REVIEWING WORK (work done, recent self-checks pass)
  Find what its self-checks did not cover: implicit requirements, ways its check differs from the real \
verification. It does not know its blind spots at this stage; that is why you are here. If the transcript shows \
it already noting a mismatch (\"X does not quite fit, but…\"), that is a signal to pivot, not to commit: a \
constraint it noticed is not a blind spot but a flag it is asking you to let it ignore. Do not. If it is on a \
reasonable track, sharpen that track; propose another only if this one is failing. Keep the review to what the \
task requires; do not suggest defensive steps beyond that.

CHOOSING BETWEEN CANDIDATES (the transcript shows it computing several readings)
  Not a bug hunt: it found the ambiguity and enumerated it. Do not construct a reading it has not already run. \
Default to the plain, face-value reading; a sophisticated close reading that gives a tidier answer is \
confirmation bias, not evidence. If it oscillates between two candidates, pick one, once; do not escalate \
certainty by repeating the same choice across calls. Reject the plain reading only if it is impossible.

Your earlier advice appears in the transcript. Being there does not make it right: it was a guess made with less \
information than exists now. Read what happened after you gave it: did the approach produce results, or more \
searching? Turns of effort without convergence are the approach failing, not the agent executing it badly. If \
what it found shows you were wrong, say so plainly (\"ignore my earlier X\"). Do not defend stale advice, and do \
not silently flip either.

You have the full transcript, including what did not work. Do not suggest what it already tried. Build on what it \
has.

When you think you know the specific answer, give it as a check, not a verdict: \"verify whether X satisfies \
constraint Y\" keeps it driving; \"the answer is X\" anchors it to a recall you cannot verify from here. Your \
reliable output is which constraint discriminates, not which candidate wins.

If a concern remains, say whether it blocks. \"One question remains\" without a verdict reads as permission to \
ship. Either it changes the answer or it does not; say which.

Be direct. Give something immediately actionable.";

#[derive(Debug, Clone, Default)]
pub struct Advisor;

impl Tool for Advisor {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: ADVISOR_TOOL.into(),
            description: "Consult a stronger reviewer who sees your whole conversation. No parameters: your \
                          history (the task, every tool call and result) is forwarded."
                .into(),
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
        system: ADVISOR.into(),
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
}
