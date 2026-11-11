use ratatui::style::{Color, Stylize};
use ratatui::text::Line;

use super::app::{App, Msg};
use super::markdown;
use super::wrap::wrap;

pub struct Side {
    pub question: String,
    pub answer: Option<Result<String, String>>,
}

pub fn ask(app: &mut App, question: Option<String>) {
    let Some(question) = question else {
        return app.transcript.notice("Ask a side question without interrupting: /btw <question>");
    };
    let Some(session) = &app.session else {
        return app.transcript.error("No model yet: /login connects a provider.");
    };
    let started = match session.side(&question) {
        Ok(run) => run,
        Err(error) => return app.transcript.error(error.to_string()),
    };
    app.side = Some(Side { question: question.clone(), answer: None });
    app.spawn(move |_| Msg::Side { question, answer: started.side_answer().map_err(|e| e.to_string()) });
}

pub fn answered(app: &mut App, question: &str, answer: Result<String, String>) {
    if let Some(side) = &mut app.side
        && side.question == question
        && side.answer.is_none()
    {
        side.answer = Some(answer);
    }
}

pub fn lines(side: &Side, busy: bool, width: usize, out: &mut Vec<Line<'static>>) {
    for line in wrap(&format!("btw: {}", side.question), width) {
        out.push(Line::from(line).bold().fg(Color::Cyan));
    }
    match &side.answer {
        None => out.push(Line::from("answering…").dim()),
        Some(Ok(answer)) => {
            out.extend(markdown::render(answer, width));
        }
        Some(Err(error)) => out.push(Line::from(error.clone()).fg(Color::Red)),
    }
    let hint = if busy {
        "esc, enter or space closes · sees the conversation up to the agent's last finished step"
    } else {
        "esc, enter or space closes · not added to the conversation"
    };
    out.push(Line::from(hint).dim());
}
