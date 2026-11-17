use app::{ChoreAnswer, Error, HeadsUp, Run};

use super::app::{App, Msg};

impl App {
    pub(super) fn after_run(&mut self) {
        if self.session.title.is_none()
            && let Some(prompt) = self.transcript.first_prompt()
            && let Some(session) = self.session.live()
        {
            let started = session.title(&prompt);
            self.chore(started);
        }
        if let Some(session) = self.session.live() {
            let (suggest, heads_up) = (session.suggest(), session.heads_up());
            let learn = super::features::on(self, domain::features::LEARNING).then(|| session.learn()).flatten();
            self.chore(suggest);
            self.chore(heads_up);
            if let Some(learn) = learn {
                self.chore(learn);
            }
        }
    }

    pub(super) fn recap(&mut self) {
        if let Some(session) = self.session.live() {
            let started = session.recap();
            self.chore(started);
        }
    }

    fn chore(&self, started: Result<Run, Error>) {
        let tx = self.tx.clone();
        let session = self.session.id();
        std::thread::spawn(move || {
            let answer = started.and_then(Run::chore_answer).map_err(|error| error.to_string());
            let _ = tx.send(Msg::Chore(session, answer));
        });
    }

    pub(super) fn chore_answered(&mut self, answer: Result<Option<ChoreAnswer>, String>) {
        match answer {
            Ok(Some(ChoreAnswer::Title(title))) => {
                if let Some(id) = self.session.id() {
                    self.saver.remember_title(&self.cwd, &id, &title);
                    let (client, named) = (self.client.clone(), title.clone());
                    std::thread::spawn(move || {
                        if let Err(error) = client.rename_session(&id, &named) {
                            tracing::warn!(%error, "could not keep the conversation's name in the catalog");
                        }
                    });
                }
                self.session.title = Some(title);
            }
            Ok(Some(ChoreAnswer::Suggestion(text))) if !self.busy() && self.prompt.input.is_empty() => {
                self.prompt.ghost = Some(text);
            }
            Ok(Some(ChoreAnswer::Recap(text))) => self.transcript.notice(format!("Where you left off: {text}")),
            Ok(Some(ChoreAnswer::HeadsUp(heads_up))) => self.transcript.notice(heads_up_text(&heads_up)),
            Ok(Some(ChoreAnswer::Learned(proposal))) => {
                self.transcript.notice(domain::learning::offered(&proposal));
                if let Some(waiting) =
                    self.features.get::<Option<domain::learning::Proposal>>(domain::learning::WAITING)
                {
                    *waiting = Some(proposal);
                }
            }
            Ok(_) | Err(_) => {}
        }
    }
}

fn heads_up_text(heads_up: &HeadsUp) -> String {
    let mark = if heads_up.urgent { "Heads-up (important)" } else { "Heads-up" };
    let explain = heads_up.explain.trim();
    if explain.is_empty() {
        format!("{mark}: {}", heads_up.title)
    } else {
        format!("{mark}: {}\n{explain}", heads_up.title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heads_up_reads_as_its_title_then_why() {
        let heads_up = |urgent| HeadsUp {
            urgent,
            learn: "x".into(),
            title: "The migration drops a column".into(),
            explain: "Existing rows lose their emails.".into(),
        };
        assert_eq!(
            heads_up_text(&heads_up(true)),
            "Heads-up (important): The migration drops a column\nExisting rows lose their emails."
        );
        assert!(heads_up_text(&heads_up(false)).starts_with("Heads-up: "));
    }
}
