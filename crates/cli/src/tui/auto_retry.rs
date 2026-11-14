use std::time::{Duration, Instant};

use ratatui::style::Stylize;
use ratatui::text::Line;

use super::app::App;

pub const DELAY: Duration = Duration::from_secs(30);

pub fn failed(app: &mut App) {
    if app.turn.transient && !app.turn.auto_retried {
        app.turn.auto_retried = true;
        app.turn.auto_retry = Some(Instant::now() + DELAY);
    }
}

pub fn tick(app: &mut App) {
    let Some(at) = app.turn.auto_retry else { return };
    if Instant::now() >= at && !app.busy() {
        app.turn.auto_retry = None;
        app.retry();
    }
}

pub fn cancel(app: &mut App) -> bool {
    app.turn.auto_retry.take().is_some()
}

pub fn fresh_prompt(app: &mut App) {
    app.turn.auto_retry = None;
    app.turn.auto_retried = false;
}

pub fn line(at: Instant, now: Instant) -> Line<'static> {
    let left = at.saturating_duration_since(now).as_secs_f64().ceil();
    Line::from(vec![
        format!("↻ Retrying in {left:.0}s").yellow(),
        " · the provider was unavailable · esc cancels".dark_gray(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_countdown_rounds_up_and_never_goes_below_zero() {
        let now = Instant::now();
        let text = |line: Line| line.spans.iter().map(|span| span.content.as_ref()).collect::<String>();
        assert!(text(line(now + Duration::from_millis(29_100), now)).starts_with("↻ Retrying in 30s"));
        assert!(text(line(now, now + Duration::from_secs(2))).starts_with("↻ Retrying in 0s"));
    }
}
