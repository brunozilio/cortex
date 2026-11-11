//! What sits above the prompt: an API key being typed, the question being
//! asked, the picker, a `/btw` answer, or the completions of an `@file` or
//! a `/word`.

use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use superfik_sdk::AskKind;

use super::app::App;
use super::ask::Pending;
use super::wrap::wrap;

pub fn panel(app: &App, width: u16) -> Vec<Line<'static>> {
    let width = usize::from(width).saturating_sub(4).max(10);
    let mut lines = Vec::new();
    let files = super::mentions::offered(app);
    if let Some(secret) = &app.secret {
        let dots = "•".repeat(secret.text.chars().count());
        lines.push(Line::from(vec![format!("{} API key: ", secret.provider.name()).bold(), dots.into()]));
        lines.push(Line::from("Paste or type it · Enter connects · Esc cancels").dim());
    } else if let Some(pending) = &app.ask {
        asking(pending, width, &mut lines);
    } else if let Some(picker) = &app.picker {
        lines.push(Line::from(picker.title.clone()).bold());
        for (i, (name, about)) in picker.items.iter().enumerate() {
            let marker = if i == picker.cursor { "❯ " } else { "  " };
            let style = if i == picker.cursor { Style::new().fg(Color::Cyan) } else { Style::new() };
            lines.push(Line::from(vec![Span::styled(format!("{marker}{name}"), style), format!("  {about}").dim()]));
        }
        lines.push(Line::from("↑↓ Enter picks · esc closes").dim());
    } else if let Some(side) = &app.side {
        super::side::lines(side, app.busy(), width, &mut lines);
    } else if !files.is_empty() {
        let chosen = app.suggestion % files.len();
        for (i, file) in files.iter().enumerate() {
            let style = if i == chosen { Style::new().fg(Color::Cyan) } else { Style::new() };
            lines.push(Line::styled(format!("  @{file}"), style));
        }
        lines.push(Line::from("  ↑↓ picks · tab completes").dim());
    } else if let Some(typed) = app.input.slash_word() {
        let found = super::commands::suggestions(typed, &app.skills);
        let total = found.len();
        let chosen = app.suggestion % total.max(1);
        let window = super::commands::shown(total, chosen);
        let more = window.len() < total;
        for (i, (name, about)) in found.into_iter().enumerate().skip(window.start).take(window.len()) {
            let style = if i == chosen { Style::new().fg(Color::Cyan) } else { Style::new() };
            let about = wrap(&about, width.saturating_sub(24)).into_iter().next().unwrap_or_default();
            lines.push(Line::from(vec![Span::styled(format!("  /{name:<20}"), style), about.dim()]));
        }
        if more {
            lines.push(Line::from(format!("  {}/{total} · ↑↓ for more · tab completes", chosen + 1)).dim());
        }
    } else if app.show_tasks && !app.transcript.tasks.is_empty() {
        tasks(&app.transcript.tasks, width, &mut lines);
    }
    lines
}

/// The agent's task list: done ones crossed out, the one in progress bold.
fn tasks(tasks: &[superfik_core::events::TaskItem], width: usize, lines: &mut Vec<Line<'static>>) {
    let done = tasks.iter().filter(|task| task.status == "completed").count();
    lines.push(Line::from(format!("Tasks {done}/{} · ctrl+t hides", tasks.len())).dim());
    for task in tasks {
        let subject = wrap(&task.subject, width.saturating_sub(4)).into_iter().next().unwrap_or_default();
        lines.push(match task.status.as_str() {
            "completed" => Line::from(format!("  ☒ {subject}")).dim().crossed_out(),
            "in_progress" => Line::from(format!("  ◼ {subject}")).bold().fg(Color::Cyan),
            _ => Line::from(format!("  ☐ {subject}")),
        });
    }
}

/// The question being asked: its title, choices, the note being typed
/// after "No", and the keys.
fn asking(pending: &Pending, width: usize, lines: &mut Vec<Line<'static>>) {
    let title = match &pending.ask.kind {
        AskKind::Permission { tool, subject, .. } => format!("Allow {tool}: {subject}?"),
        AskKind::Plan { .. } => "Ready to code? The plan is above. Would you like to proceed?".to_owned(),
        AskKind::Questions(questions) => questions.get(pending.question).map(|q| q.text.clone()).unwrap_or_default(),
    };
    for line in wrap(&title, width) {
        lines.push(Line::from(line).bold().fg(Color::Yellow));
    }
    let picked = pending.picked.get(pending.question);
    for (i, choice) in pending.choices().into_iter().enumerate() {
        let check = if picked.is_some_and(|p| p.contains(&i)) { "[x] " } else { "" };
        let line = format!("{} {}. {check}{choice}", if i == pending.cursor { "❯" } else { " " }, i + 1);
        let style = if i == pending.cursor { Style::new().fg(Color::Cyan) } else { Style::new() };
        lines.push(Line::styled(line, style));
    }
    if let Some(note) = &pending.note {
        for line in wrap(&format!("> {note}▏"), width) {
            lines.push(Line::from(line).fg(Color::Cyan));
        }
    }
    let hint = match &pending.ask.kind {
        _ if pending.note.is_some() => "Tell the agent what to do instead · Enter sends · esc just says no",
        AskKind::Permission { .. } => "y yes · a always · n no · esc denies",
        AskKind::Questions(_) => "number or ↑↓ Enter · space toggles a multiple choice · esc skips",
        AskKind::Plan { .. } => "number or ↑↓ Enter · esc keeps planning · type after to say what to change",
    };
    lines.push(Line::from(hint).dim());
}
