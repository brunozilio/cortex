use domain::usage::{ContextSlice, ContextUsage};
use sdk::EngineEvent;

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

pub fn left(used: u64, window: Option<u64>) -> Option<String> {
    domain::usage::percent_left(used, window).map(|left| format!("{left}% context left"))
}

#[cfg(test)]
mod tests {
    use domain::usage::ContextShare;

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
