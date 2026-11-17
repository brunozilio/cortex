use crate::model::Effort;

pub const MAX_REJECTIONS: usize = 2;
pub const MAX_REFUSALS_IN_A_ROW: usize = 3;
pub const MAX_REFUSALS: usize = 20;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Refusals {
    in_a_row: usize,
    total: usize,
}

impl Refusals {
    pub fn refused(&mut self) {
        self.in_a_row += 1;
        self.total += 1;
    }

    pub fn went_ahead(&mut self) {
        self.in_a_row = 0;
    }

    pub fn stop(&self) -> Option<String> {
        if self.in_a_row >= MAX_REFUSALS_IN_A_ROW {
            return Some(format!(
                "{} actions in a row were refused and no work went ahead in between, so this agent was stopped.",
                self.in_a_row
            ));
        }
        (self.total >= MAX_REFUSALS)
            .then(|| format!("{} actions were refused in this run, so this agent was stopped.", self.total))
    }
}

pub fn effort(label: &str) -> Option<Effort> {
    Effort::ALL.into_iter().find(|effort| effort.cli_value().eq_ignore_ascii_case(label.trim()))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rejections {
    reasons: Vec<String>,
}

impl Rejections {
    pub fn count(&self) -> usize {
        self.reasons.len()
    }

    pub fn reject(&mut self, reason: String) -> bool {
        self.reasons.push(reason);
        self.reasons.len() <= MAX_REJECTIONS
    }

    pub fn note(&self) -> Option<String> {
        let reason = self.reasons.last()?;
        Some(format!("A hook rejected your previous reply and it was discarded. Answer again. The hook said: {reason}"))
    }

    pub fn accepted(&mut self) {
        self.reasons.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_effort_is_named_like_on_the_command_line() {
        assert_eq!(effort("low"), Some(Effort::Low));
        assert_eq!(effort(" XHigh "), Some(Effort::XHigh));
        assert_eq!(effort("turbo"), None);
    }

    #[test]
    fn a_reply_is_asked_again_at_most_max_rejections_times() {
        let mut rejections = Rejections::default();
        assert_eq!(rejections.note(), None);
        for _ in 0..MAX_REJECTIONS {
            assert!(rejections.reject("cite a file".into()));
        }
        assert!(!rejections.reject("cite a file".into()), "then the run gives up");
        assert!(rejections.note().is_some_and(|note| note.contains("cite a file")));
    }

    #[test]
    fn refusals_stop_an_agent_in_a_row_or_in_total() {
        let mut refusals = Refusals::default();
        for _ in 1..MAX_REFUSALS_IN_A_ROW {
            refusals.refused();
        }
        assert_eq!(refusals.stop(), None);
        refusals.went_ahead();
        refusals.refused();
        assert_eq!(refusals.stop(), None, "work in between starts the row again");
        refusals.refused();
        refusals.refused();
        assert!(refusals.stop().is_some_and(|why| why.starts_with("3 actions in a row")));
        let mut spread = Refusals::default();
        for _ in 0..MAX_REFUSALS {
            spread.refused();
            spread.went_ahead();
        }
        assert!(spread.stop().is_some_and(|why| why.starts_with("20 actions were refused")));
    }

    #[test]
    fn an_accepted_reply_forgets_the_rejections() {
        let mut rejections = Rejections::default();
        rejections.reject("no".into());
        rejections.accepted();
        assert_eq!((rejections.count(), rejections.note()), (0, None));
    }
}
