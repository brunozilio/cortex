//! Side calls the chat makes by itself, each one model turn without tools
//! that never joins the conversation: a title once its first run ends, the
//! prompt you would likely type next (dimmed in the empty prompt; Tab takes
//! it), the rare heads-up worth knowing, and a recap when continuing an
//! earlier conversation.

use superfik_sdk::{ChoreAnswer, Error, HeadsUp, Run};

use super::app::{App, Msg};

impl App {
    /// After a run of the conversation completes.
    pub(super) fn after_run(&mut self) {
        if self.title.is_none()
            && let Some(prompt) = self.transcript.first_prompt()
            && let Some(session) = &self.session
        {
            let started = session.title(&prompt);
            self.chore(started);
        }
        if let Some(session) = &self.session {
            let (suggest, heads_up) = (session.suggest(), session.heads_up());
            self.chore(suggest);
            self.chore(heads_up);
        }
    }

    /// When the chat continues an earlier conversation.
    pub(super) fn recap(&mut self) {
        if let Some(session) = &self.session {
            let started = session.recap();
            self.chore(started);
        }
    }

    fn chore(&self, started: Result<Run, Error>) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let answer = started.and_then(Run::chore_answer).map_err(|error| error.to_string());
            let _ = tx.send(Msg::Chore(answer));
        });
    }

    pub(super) fn chore_answered(&mut self, answer: Result<Option<ChoreAnswer>, String>) {
        match answer {
            Ok(Some(ChoreAnswer::Title(title))) => {
                if let Some(id) = self.session.as_ref().and_then(superfik_sdk::Session::id) {
                    self.saver.remember_title(&self.cwd, &id, &title);
                }
                self.title = Some(title);
            }
            Ok(Some(ChoreAnswer::Suggestion(text))) if !self.busy() && self.input.is_empty() => {
                self.ghost = Some(text);
            }
            Ok(Some(ChoreAnswer::Recap(text))) => self.transcript.notice(format!("Where you left off: {text}")),
            Ok(Some(ChoreAnswer::HeadsUp(heads_up))) => self.transcript.notice(heads_up_text(&heads_up)),
            // A side call is a nicety: when it gives nothing or fails, the
            // chat goes on without it.
            Ok(_) | Err(_) => {}
        }
    }

    /// Tab on an empty prompt: takes the suggested next prompt.
    pub(super) fn take_ghost(&mut self) -> bool {
        match self.ghost.take() {
            Some(text) if self.input.is_empty() => {
                self.input.set(&text);
                true
            }
            _ => false,
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
