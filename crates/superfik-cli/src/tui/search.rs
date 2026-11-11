//! Ctrl+R: searching the prompts sent before, newest first. Typing narrows
//! the query, Ctrl+R again goes to an older match, Enter takes it and Esc
//! leaves the prompt as it was.

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Search {
    pub query: String,
    /// The history index of the match shown.
    pub at: Option<usize>,
}

impl Search {
    /// The newest prompt holding the query, older than the one shown when
    /// `older`; it stays on the current match when there is none.
    pub fn find(&mut self, history: &[String], older: bool) {
        let before = match (self.at, older) {
            (Some(at), true) => at,
            _ => history.len(),
        };
        let query = self.query.to_lowercase();
        let found = history[..before].iter().rposition(|prompt| prompt.to_lowercase().contains(&query));
        if found.is_some() || !older {
            self.at = found;
        }
    }

    pub fn shown<'a>(&self, history: &'a [String]) -> Option<&'a str> {
        self.at.and_then(|at| history.get(at)).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_newest_match_then_older_ones() {
        let history: Vec<String> =
            ["run the tests", "fix the login", "run the linter", "explain"].map(str::to_owned).to_vec();
        let mut search = Search { query: "RUN".into(), at: None };
        search.find(&history, false);
        assert_eq!(search.shown(&history), Some("run the linter"));
        search.find(&history, true);
        assert_eq!(search.shown(&history), Some("run the tests"));
        search.find(&history, true);
        assert_eq!(search.shown(&history), Some("run the tests"), "no older match: it stays");
        search.query = "nothing like it".into();
        search.find(&history, false);
        assert_eq!(search.shown(&history), None);
    }
}
