//! `/context`: what fills the context window, part by part, and how much
//! is left before the conversation is compacted.

use superfik_core::usage::{ContextSlice, ContextUsage};
use superfik_sdk::EngineEvent;

use super::app::{App, Msg};
use super::entries::tokens;

pub fn measure(app: &mut App) {
    let Some(session) = &app.session else { return app.transcript.notice("No model yet: /login connects one.") };
    match session.measure_context() {
        Ok(run) => {
            let tx = app.tx.clone();
            std::thread::spawn(move || {
                for event in run {
                    match event {
                        EngineEvent::ContextMeasured(usage) => {
                            let _ = tx.send(Msg::Context(usage));
                        }
                        EngineEvent::Failed(error) => {
                            let _ = tx.send(Msg::Error(error));
                        }
                        _ => {}
                    }
                }
            });
        }
        Err(error) => app.transcript.error(error.to_string()),
    }
}

/// How `/context` reads: a bar, the total against the window, then each
/// part.
pub fn view(usage: &ContextUsage) -> String {
    const WIDTH: usize = 40;
    let filled = ((usage.fraction() * WIDTH as f32).round() as usize).min(WIDTH);
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(WIDTH - filled));
    let mut text = format!(
        "Context {bar} {}%\n  {} of {} used · {} free before compacting",
        usage.percent(),
        tokens(usage.used),
        tokens(usage.window),
        tokens(usage.free())
    );
    for share in usage.shares.iter().filter(|share| share.tokens > 0) {
        let name = match &share.slice {
            ContextSlice::SystemPrompt => "System prompt".to_owned(),
            ContextSlice::Tools => "Tools".to_owned(),
            ContextSlice::Mcp => "MCP tools".to_owned(),
            ContextSlice::Agents => "Agents".to_owned(),
            ContextSlice::Memory => "Memory".to_owned(),
            ContextSlice::Skills => "Skills".to_owned(),
            ContextSlice::Messages => "Messages".to_owned(),
            ContextSlice::Other(name) => name.clone(),
        };
        text.push_str(&format!("\n  {name:<14} {}", tokens(share.tokens)));
    }
    text
}

/// What the status line says of the context: the share left, once the
/// window is known.
pub fn left(used: u64, window: Option<u64>) -> Option<String> {
    let window = window.filter(|w| *w > 0)?;
    let left = 100u64.saturating_sub(used.saturating_mul(100) / window);
    Some(format!("{left}% context left"))
}

#[cfg(test)]
mod tests {
    use superfik_core::usage::ContextShare;

    use super::*;

    #[test]
    fn the_breakdown_shows_each_part_and_what_is_free() {
        let usage = ContextUsage {
            used: 50_000,
            window: 200_000,
            reserved: 20_000,
            shares: vec![
                ContextShare { slice: ContextSlice::SystemPrompt, tokens: 8_000 },
                ContextShare { slice: ContextSlice::Messages, tokens: 42_000 },
                ContextShare { slice: ContextSlice::Mcp, tokens: 0 },
            ],
        };
        let shown = view(&usage);
        assert!(shown.starts_with("Context ██████████░"), "{shown}");
        assert!(shown.contains("25%") && shown.contains("free before compacting"), "{shown}");
        assert!(shown.contains("System prompt") && shown.contains("Messages") && !shown.contains("MCP"), "{shown}");
        assert_eq!(left(50_000, Some(200_000)).as_deref(), Some("75% context left"));
        assert_eq!(left(50_000, None), None);
    }
}
