use domain::features::LEARNING_CHECK;

use super::{Feature, Slash};
use crate::tui::app::{App, Msg};

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["check"],
            about: "Have the agent check this branch's changes against the memories about the files they touch",
            run,
        }],
        ..Feature::default()
    }
}

fn run(app: &mut App, _argument: Option<String>) {
    if !super::on(app, LEARNING_CHECK) {
        return app.transcript.notice(format!("/check is turned off ({LEARNING_CHECK}); /features turns it on."));
    }
    let cwd = app.cwd.clone();
    app.spawn(move |client| match client.memory(cwd).check() {
        Ok(Some(prompt)) => super::reply(move |app: &mut App| app.send(prompt)),
        Ok(None) => Msg::Notice("Nothing to check: no change, or no memory is about the changed files.".into()),
        Err(error) => Msg::Error(error.to_string()),
    });
}

#[cfg(test)]
mod tests {
    use agent_engine::fake::FakeEngine;
    use app::{EngineEvent, Model};
    use ratatui::crossterm::event::KeyCode;

    use crate::tui::app::App;
    use crate::tui::tests::{client, press, screen, type_text, until};

    #[test]
    fn outside_git_the_check_says_why_it_cannot_run() {
        let data = tempfile::tempdir().expect("data");
        let model = Model::new("keyed:scripted", "Scripted");
        let engine = FakeEngine::new(vec![model], vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }]);
        let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
        until(&mut app, |app| app.session.is_some());

        type_text(&mut app, "/check");
        press(&mut app, KeyCode::Enter);

        until(&mut app, |app| screen(app).contains("needs a git repository"));
    }

    #[test]
    fn a_change_to_a_remembered_file_is_sent_to_the_agent_to_check() {
        let data = tempfile::tempdir().expect("data");
        let git = |args: &[&str]| {
            let done = std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
                .args(args)
                .current_dir(data.path())
                .output()
                .expect("git");
            assert!(done.status.success());
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(data.path().join("client.go"), "retries = 4\n").expect("write");
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "one"]);
        std::fs::write(data.path().join("client.go"), "retries = 10\n").expect("write");
        let model = Model::new("keyed:scripted", "Scripted");
        let engine = FakeEngine::new(vec![model], vec![EngineEvent::Completed { duration_ms: 1, cost_usd: None }]);
        let rememberer = client(&engine, data.path());
        let memory = rememberer.memory(data.path());
        let edit = app::MemoryEdit::Remember {
            kind: app::MemoryKind::Semantic,
            text: "Keep retries at 4".into(),
            tags: Vec::new(),
            touches: vec!["client.go".into()],
        };
        memory.edit(vec![edit]).expect("remember");
        let mut app = App::new(client(&engine, data.path()), data.path().into(), None, None);
        until(&mut app, |app| app.session.is_some());

        type_text(&mut app, "/check");
        press(&mut app, KeyCode::Enter);

        until(&mut app, |_| engine.requests().iter().any(|r| r.prompt.text.contains("Keep retries at 4")));
        let sent = engine.requests().into_iter().find(|r| r.prompt.text.contains("Keep retries at 4")).expect("sent");
        assert!(
            sent.prompt.text.contains("git diff HEAD") && sent.prompt.text.contains("- client.go"),
            "{}",
            sent.prompt.text
        );
    }
}
