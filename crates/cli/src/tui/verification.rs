//! `/verification` in the chat, and how the active criteria read. The
//! rules (precedence, attempts, plan section) are core's.

use domain::command::VerificationCommand;
use sdk::{Criteria, CriteriaSource};

use super::app::App;

pub fn run(app: &mut App, command: VerificationCommand) {
    let Some(session) = app.session.as_mut() else {
        return app.transcript.notice("Pick a model first: verification belongs to a conversation.");
    };
    let notice = match command {
        VerificationCommand::Show => match session.verification() {
            Some(criteria) => format!("Verification ({}):\n{}", source(&criteria), criteria.text),
            None => "No verification. /verification <criteria> judges each run against them; a plan's \
                     Verification section applies once you leave plan mode."
                .into(),
        },
        VerificationCommand::Clear => {
            session.set_verification(None);
            match session.verification() {
                Some(criteria) => format!("Your criteria are cleared; the plan's still apply:\n{}", criteria.text),
                None => "Verification off.".into(),
            }
        }
        VerificationCommand::Set(text) => {
            session.set_verification(Some(text));
            "Each run will be judged against these before it finishes (a judge agent runs the checks).".into()
        }
    };
    app.transcript.notice(notice);
}

/// The status line's marker: `verify` and where the criteria came from.
pub fn marker(criteria: &Criteria) -> String {
    format!("verify ({})", source(criteria))
}

fn source(criteria: &Criteria) -> &'static str {
    match criteria.source {
        CriteriaSource::User => "yours",
        CriteriaSource::Plan => "plan",
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyCode;
    use agent_engine::fake::FakeEngine;
    use sdk::{EngineEvent, Model, PermissionMode};

    use super::super::tests::{press, screen, superfik, type_text, until};
    use super::*;

    fn command(app: &mut App, text: &str) {
        type_text(app, text);
        press(app, KeyCode::Enter);
    }

    #[test]
    fn verification_is_set_shown_sent_with_the_next_prompt_and_cleared() {
        let data = tempfile::tempdir().expect("data");
        let model = Model::new("anthropic:scripted", "Scripted");
        let engine = FakeEngine::new(vec![model], vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }]);
        let mut app = App::new(superfik(&engine, data.path()), data.path().into(), None, Some(PermissionMode::Ask));
        until(&mut app, |app| app.session.is_some());
        command(&mut app, "/verification");
        assert!(screen(&app).contains("No verification."), "{}", screen(&app));
        command(&mut app, "/verification cargo test passes");
        assert!(screen(&app).contains("verify (yours)"), "{}", screen(&app));
        command(&mut app, "fix it");
        until(&mut app, |app| !app.busy());
        assert_eq!(engine.requests()[0].verification, Some(Criteria::user("cargo test passes")));
        command(&mut app, "/verification off");
        assert!(screen(&app).contains("Verification off.") && !screen(&app).contains("verify (yours)"));
    }

    #[test]
    fn the_marker_names_the_criterias_source() {
        assert_eq!(marker(&Criteria::user("x")), "verify (yours)");
        assert_eq!(marker(&Criteria::plan("x")), "verify (plan)");
    }
}
