mod compact;
mod counted;

use domain::{ContextShare, ContextSlice, ContextUsage, ModelOption};

pub use self::compact::{compact, compact_hooked, compact_hooked_on, compact_on, summary_text};
pub use self::counted::Counted;
use crate::model::ToolSpec;
use crate::transcript::{Block, Message};

const CHARS_PER_TOKEN: u64 = 4;

pub fn window(model: &ModelOption) -> u64 {
    if let Some(tokens) = model.window {
        return tokens;
    }
    llm_providers::find(model.provider).map_or(200_000, |provider| provider.window())
}

pub fn tokens(text: &str) -> u64 {
    (text.len() as u64).div_ceil(CHARS_PER_TOKEN)
}

fn message_tokens(message: &Message) -> u64 {
    message
        .blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => tokens(text),
            Block::Thinking { text, .. } => tokens(text),
            Block::ToolUse { name, input, .. } => tokens(name) + tokens(&input.to_string()),
            Block::ToolResult { content, .. } => tokens(content),
            Block::ToolResultRich { content, images, .. } => tokens(content) + images.len() as u64 * 1600,
            Block::Image(_) => 1600,
        })
        .sum()
}

pub fn measure(model: &ModelOption, system: &str, tools: &[ToolSpec], history: &[Message]) -> ContextUsage {
    let spec_tokens =
        |spec: &ToolSpec| tokens(&spec.name) + tokens(&spec.description) + tokens(&spec.schema.to_string());
    let (mcp, own): (Vec<&ToolSpec>, Vec<&ToolSpec>) = tools.iter().partition(|spec| spec.name.starts_with("mcp__"));
    let shares = vec![
        ContextShare { slice: ContextSlice::SystemPrompt, tokens: tokens(system) },
        ContextShare { slice: ContextSlice::Tools, tokens: own.into_iter().map(spec_tokens).sum() },
        ContextShare { slice: ContextSlice::Mcp, tokens: mcp.into_iter().map(spec_tokens).sum() },
        ContextShare { slice: ContextSlice::Messages, tokens: history.iter().map(message_tokens).sum() },
    ];
    let window = window(model);
    ContextUsage {
        used: shares.iter().map(|share| share.tokens).sum(),
        window,
        reserved: window / 10,
        shares: shares.into_iter().filter(|share| share.tokens > 0).collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use domain::Effort;
    use serde_json::json;

    use super::*;
    use crate::model::TurnRequest;
    use crate::model::scripted::ScriptedModel;

    fn model() -> ModelOption {
        ModelOption::new("gpt-5-mini", "GPT-5 mini").with_window(131_072)
    }

    fn template() -> TurnRequest {
        TurnRequest {
            model: "qwen".into(),
            system: "s".into(),
            messages: Vec::new(),
            tools: Vec::new(),
            effort: Effort::Low,
            max_tokens: 100,
        }
    }

    #[test]
    fn measures_the_prompt_the_tools_and_the_conversation() {
        let tools = vec![
            ToolSpec { name: "Read".into(), description: "x".repeat(40), schema: json!({}) },
            ToolSpec {
                name: concat!("mcp__", brand::command!(), "_memory__memory_search").into(),
                description: String::new(),
                schema: json!({}),
            },
        ];
        let history = vec![Message::user("a".repeat(400))];
        let usage = measure(&model(), &"s".repeat(800), &tools, &history);
        let share = |slice: ContextSlice| usage.shares.iter().find(|s| s.slice == slice).map(|s| s.tokens);
        assert_eq!(share(ContextSlice::SystemPrompt), Some(200));
        assert_eq!(share(ContextSlice::Messages), Some(100));
        assert!(share(ContextSlice::Mcp).is_some_and(|t| t > 0));
        assert_eq!((usage.window, usage.reserved), (131_072, 13_107));
        assert_eq!(window(&model().with_window(262_144)), 262_144, "the model's own window wins");
        let unknown = ModelOption::new("m", "M").served_by(domain::ModelProvider::new("nobody"));
        assert_eq!(window(&unknown), 200_000, "a provider no folder knows gets the usual window");
        assert_eq!(usage.used, usage.shares.iter().map(|s| s.tokens).sum::<u64>());
    }

    #[test]
    fn compacts_into_one_summary_message_and_keeps_nothing_on_failure() {
        let mut history =
            vec![Message::user("x".repeat(4000)), Message::assistant(vec![Block::Text("y".repeat(4000))])];
        let model =
            ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("They asked for x; y is done.".into())])]);
        let compaction = compact(&model, &template(), &mut history, &AtomicBool::new(false)).expect("compact");
        assert_eq!(history.len(), 1);
        assert!(
            history[0].text().starts_with(&crate::prompt::text("continued"))
                && history[0].text().ends_with("y is done.")
        );
        assert_eq!(compaction.before, 2000);
        assert!(compaction.after < 50);

        let mut tagged = vec![Message::user("x".repeat(4000))];
        let model = ScriptedModel::new([ScriptedModel::reply(vec![Block::Text(
            "<analysis>They asked for x.</analysis>\n<summary>\n1. Primary request: x.\n</summary>".into(),
        )])]);
        compact(&model, &template(), &mut tagged, &AtomicBool::new(false)).expect("compact");
        let focused = crate::model::scripted::ScriptedModel::new([crate::model::scripted::ScriptedModel::reply(vec![
            crate::transcript::Block::Text("<summary>kept</summary>".into()),
        ])]);
        let mut history = vec![Message::user("one"), Message::user("two")];
        compact_on(&focused, &template(), &mut history, &AtomicBool::new(false), Some("the API changes"))
            .expect("focused");
        let asked = focused.asked()[0].messages.last().map(Message::text).unwrap_or_default();
        assert!(asked.ends_with("The user asks the summary to focus on: the API changes"), "{asked}");
        assert_eq!(
            tagged[0].text(),
            format!("{}\n\n1. Primary request: x.", crate::prompt::text("continued")),
            "the analysis is dropped"
        );

        let mut kept = vec![Message::user("z")];
        let silent = ScriptedModel::new([ScriptedModel::reply(vec![])]);
        assert!(compact(&silent, &template(), &mut kept, &AtomicBool::new(false)).is_err());
        assert_eq!(kept, vec![Message::user("z")]);
    }
}
