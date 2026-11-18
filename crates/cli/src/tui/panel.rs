use app::AskKind;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};

use super::app::App;
use super::ask::Pending;
use super::wrap::{clip, wrap};

pub fn panel(app: &App, width: u16) -> Vec<Line<'static>> {
    let width = usize::from(width).saturating_sub(4).max(10);
    let mut lines = Vec::new();
    let files = super::mentions::offered(app);
    if let Some(secret) = &app.overlays.secret {
        let dots = "•".repeat(secret.text.chars().count());
        lines.push(Line::from(vec![format!("{} API key: ", secret.name).bold(), dots.into()]));
        lines.push(Line::from("Paste or type it · Enter connects · Esc cancels").dim());
    } else if let Some(pending) = &app.ask {
        asking(pending, width, &mut lines);
    } else if let Some(picker) = &app.overlays.picker {
        lines.push(Line::from(picker.title.clone()).bold());
        let window = picker.window();
        let total = picker.items.len();
        for (i, (name, about)) in picker.items.iter().enumerate().skip(window.start).take(window.len()) {
            let marker = if i == picker.cursor { "❯ " } else { "  " };
            let style = if i == picker.cursor { Style::new().fg(Color::Cyan) } else { Style::new() };
            lines.push(Line::from(vec![Span::styled(format!("{marker}{name}"), style), format!("  {about}").dim()]));
        }
        if window.len() < total {
            lines.push(
                Line::from(format!("{} of {total} · ↑↓ more · Enter picks · esc closes", picker.cursor + 1)).dim(),
            );
        } else {
            lines.push(Line::from("↑↓ Enter picks · esc closes").dim());
        }
    } else if let Some(side) = &app.overlays.side {
        super::side::lines(side, app.busy(), width, &mut lines);
    } else if !files.is_empty() {
        let chosen = app.prompt.suggestion % files.len();
        for (i, file) in files.iter().enumerate() {
            let style = if i == chosen { Style::new().fg(Color::Cyan) } else { Style::new() };
            lines.push(Line::styled(format!("  @{file}"), style));
        }
        lines.push(Line::from("  ↑↓ picks · tab completes").dim());
    } else if let Some(typed) = app.prompt.input.slash_word() {
        let found = super::commands::suggestions(typed, &app.catalog.skills);
        let total = found.len();
        let chosen = app.prompt.suggestion % total.max(1);
        let window = super::commands::shown(total, chosen);
        let more = window.len() < total;
        let shown: Vec<(usize, (String, String))> =
            found.into_iter().enumerate().skip(window.start).take(window.len()).collect();
        let column = shown.iter().map(|(_, (name, _))| name.chars().count()).max().unwrap_or(0).clamp(6, 22) + 3;
        for (i, (name, about)) in shown {
            let picked = i == chosen;
            let style = if picked { Style::new().fg(Color::Cyan).bold() } else { Style::new() };
            let about = clip(&about, width.saturating_sub(column + 2));
            let name = format!("{}/{name}", if picked { "› " } else { "  " });
            let name = format!("{name:<width$}", width = column + 2);
            let used = name.chars().count() + about.chars().count();
            let mut row = Line::from(vec![
                Span::styled(name, style),
                about.dark_gray(),
                " ".repeat((width + 2).saturating_sub(used)).into(),
            ]);
            if picked {
                row = row.bg(super::theme::current().selection_bg());
            }
            lines.push(row);
        }
        if more {
            lines.push(Line::from(format!("  {} of {total} · ↑↓ more · tab completes", chosen + 1)).dark_gray());
        }
    } else if app.ui.show_tasks && !app.transcript.tasks.is_empty() {
        tasks(&app.transcript.tasks, width, &mut lines);
    } else if app.transcript.entries.is_empty() && app.ui.inline.is_none() && app.prompt.input.is_empty() && !app.busy()
    {
        for (key, what) in START {
            lines.push(Line::from(vec![format!(" {key:<10}").cyan(), what.dark_gray()]));
        }
    }
    lines
}

const START: [(&str, &str); 4] = [
    ("/resume", "pick up an earlier conversation"),
    ("/model", "switch the model; /model <plan|explore|small|review|advisor> sets one per role"),
    ("@file", "point the agent at a file"),
    ("!cmd", "run a shell command yourself"),
];

fn tasks(tasks: &[domain::events::TaskItem], width: usize, lines: &mut Vec<Line<'static>>) {
    let done = tasks.iter().filter(|task| task.status == "completed").count();
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        " Tasks".bold(),
        format!("  {done} of {} done · ctrl+t hides", tasks.len()).dark_gray(),
    ]));
    for task in tasks {
        let subject = clip(&task.subject, width.saturating_sub(5));
        lines.push(match task.status.as_str() {
            "completed" => Line::from(vec!["   ✓ ".green(), subject.dark_gray().crossed_out()]),
            "in_progress" => Line::from(vec!["   ● ".cyan(), subject.bold()]),
            _ => Line::from(vec!["   ○ ".dark_gray(), subject.into()]),
        });
    }
}

fn asking(pending: &Pending, width: usize, lines: &mut Vec<Line<'static>>) {
    let title = match &pending.ask.kind {
        AskKind::Permission { tool, subject, .. } => format!("Allow {tool}: {subject}?"),
        AskKind::Plan { .. } => "Ready to code? The plan is above. Would you like to proceed?".to_owned(),
        AskKind::Questions(questions) => questions.get(pending.question).map(|q| q.text.clone()).unwrap_or_default(),
    };
    for line in wrap(&title, width) {
        lines.push(Line::from(line).bold());
    }
    let picked = pending.picked.get(pending.question);
    let choices = pending.choices();
    let permission = matches!(pending.ask.kind, AskKind::Permission { .. });
    for (i, choice) in choices.iter().enumerate() {
        let check = if picked.is_some_and(|p| p.contains(&i)) { "[x] " } else { "" };
        let here = i == pending.cursor;
        let mut spans = vec![
            if here { "❯ ".fg(Color::Yellow) } else { "  ".into() },
            format!("{}. ", i + 1).dark_gray(),
            Span::styled(format!("{check}{choice}"), if here { Style::new().bold() } else { Style::new() }),
        ];
        if permission {
            let key = if i == 0 {
                "y"
            } else if i + 1 == choices.len() {
                "n"
            } else {
                "a"
            };
            spans.push("  ".into());
            spans.push(format!("({key})").cyan());
        }
        lines.push(Line::from(spans));
    }
    if let Some(note) = &pending.note {
        for line in wrap(&format!("> {note}▏"), width) {
            lines.push(Line::from(line).fg(Color::Cyan));
        }
    }
    let hint = match &pending.ask.kind {
        _ if pending.note.is_some() => "Tell the agent what to do instead · Enter sends · esc just says no",
        AskKind::Permission { .. } => "↑↓ select · enter confirms · esc denies",
        AskKind::Questions(_) => "number or ↑↓ Enter · space toggles a multiple choice · esc skips",
        AskKind::Plan { .. } => "number or ↑↓ Enter · esc keeps planning · type after to say what to change",
    };
    lines.push(Line::from(hint).dim());
}
