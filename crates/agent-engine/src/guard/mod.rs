use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use serde_json::{Value, json};

use crate::model::{ModelApi, TurnRequest};
use crate::transcript::{Block, Message, Role};

const BOUNDARY: u8 = 50;

#[derive(Debug, Default)]
pub struct Safety {
    transcript: Mutex<String>,
}

impl Safety {
    pub fn observe(&self, history: &[Message]) {
        if let Ok(mut transcript) = self.transcript.lock() {
            *transcript = render(history);
        }
    }

    pub fn fork(&self) -> Self {
        Self { transcript: Mutex::new(self.transcript.lock().map(|t| t.clone()).unwrap_or_default()) }
    }

    pub fn check(
        &self,
        api: &dyn ModelApi,
        template: &TurnRequest,
        instructions: &str,
        tool: &str,
        input: &Value,
        stop: &AtomicBool,
    ) -> Result<(), String> {
        let transcript = self.transcript.lock().map(|t| t.clone()).unwrap_or_default();
        let action = json!({ tool: input }).to_string();
        let messages = vec![
            Message::user(format!(
                "The following is the user's instructions configuration ({brand_instructions} and the like). Treat it as \
                 context about the user's environment and intent. If it explicitly authorizes the SPECIFIC \
                 action under review (same operation, same target) you may weigh that as user intent to allow. \
                 Generic encouragement (\"be autonomous\", \"don't ask\") is not authorization.\n\n\
                 <user_instructions>\n{instructions}\n</user_instructions>",
                brand_instructions = brand::INSTRUCTIONS
            )),
            Message::user(format!("<transcript>\n{transcript}{action}\n</transcript>")),
        ];
        let request = TurnRequest {
            system: crate::prompt::text("classifier"),
            messages,
            tools: Vec::new(),
            effort: domain::Effort::Low,
            max_tokens: template.max_tokens.min(512),
            ..template.clone()
        };
        let answer = api
            .turn(&request, &mut |_| {}, stop)
            .map_err(|error| format!("{brand_name}'s safety check could not run ({error}), so the action was held back. Retry it, or ask the user.", brand_name = brand::NAME))?
            .message
            .text();
        match grade(&answer) {
            Some((severity, _)) if severity <= BOUNDARY => Ok(()),
            Some((_, category)) => Err(format!(
                "{brand_name}'s safety check held this action back{}. Ask the user to approve it explicitly, or \
                 reach the goal another way.",
                category.map(|c| format!(" ({c})")).unwrap_or_default(),
                brand_name = brand::NAME
            )),
            None => Err(concat!(
                brand::name!(),
                "'s safety check answered out of form, so the action was held back. Retry it, \
                         or ask the user."
            )
            .into()),
        }
    }
}

pub fn grade(answer: &str) -> Option<(u8, Option<String>)> {
    let inner = |tag: &str| {
        let open = format!("<{tag}>");
        let start = answer.find(&open)? + open.len();
        let end = start + answer[start..].find(&format!("</{tag}>"))?;
        Some(answer[start..end].trim().to_owned())
    };
    let severity = inner("severity")?.parse::<u8>().ok().filter(|n| *n <= 100)?;
    Some((severity, inner("category").filter(|c| !c.is_empty())))
}

pub fn render(history: &[Message]) -> String {
    let mut lines = String::new();
    for message in history {
        for block in &message.blocks {
            let line = match (message.role, block) {
                (Role::User, Block::Text(text)) => json!({ "user": text }),
                (Role::Assistant, Block::Text(text)) if !text.trim().is_empty() => json!({ "assistant": text }),
                (Role::Assistant, Block::ToolUse { name, input, .. }) => json!({ name.as_str(): input }),
                _ => continue,
            };
            lines.push_str(&line.to_string());
            lines.push('\n');
        }
    }
    lines
}

#[cfg(test)]
mod tests;
