#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DecisionNote {
    pub decided: String,
    pub ruled_out: Vec<String>,
    pub watch_out: Vec<String>,
}

const DECIDED: &str = "Decided:";
const RULED_OUT: &str = "Ruled out:";
const WATCH_OUT: &str = "Watch out:";

const DECIDED_HEADING: &str = "## Decided";
const RULED_OUT_HEADING: &str = "## Tried and ruled out";
const WATCH_OUT_HEADING: &str = "## Watch out";

impl DecisionNote {
    pub fn new(decided: &str, ruled_out: &[String], watch_out: &[String]) -> Option<Self> {
        let clean = |items: &[String]| -> Vec<String> {
            items.iter().map(|item| tidy(item)).filter(|item| !item.is_empty()).collect()
        };
        let decided = tidy(decided);
        (!decided.is_empty()).then(|| Self { decided, ruled_out: clean(ruled_out), watch_out: clean(watch_out) })
    }

    pub fn text(&self) -> String {
        let mut out = format!("{DECIDED} {}.", self.decided);
        if !self.ruled_out.is_empty() {
            out.push_str(&format!(" {RULED_OUT} {}.", self.ruled_out.join("; ")));
        }
        if !self.watch_out.is_empty() {
            out.push_str(&format!(" {WATCH_OUT} {}.", self.watch_out.join("; ")));
        }
        out
    }

    pub fn parse(text: &str) -> Option<Self> {
        let rest = text.trim().strip_prefix(DECIDED)?;
        let (decided, rest) = split_at_label(rest, &[RULED_OUT, WATCH_OUT]);
        let (ruled_out, rest) = match rest.strip_prefix(RULED_OUT) {
            Some(after) => split_at_label(after, &[WATCH_OUT]),
            None => ("", rest),
        };
        let watch_out = rest.strip_prefix(WATCH_OUT).unwrap_or_default();
        let items =
            |part: &str| -> Vec<String> { part.split("; ").map(tidy).filter(|item| !item.is_empty()).collect() };
        Self::new(decided, &items(ruled_out), &items(watch_out))
    }

    pub fn markdown(&self) -> String {
        let mut out = format!("{DECIDED_HEADING}\n- {}\n", self.decided);
        for (heading, items) in [(RULED_OUT_HEADING, &self.ruled_out), (WATCH_OUT_HEADING, &self.watch_out)] {
            if !items.is_empty() {
                out.push_str(&format!("\n{heading}\n"));
                for item in items {
                    out.push_str(&format!("- {item}\n"));
                }
            }
        }
        out
    }

    pub fn from_markdown(body: &str) -> Option<Self> {
        let mut sections: [Vec<String>; 3] = Default::default();
        let mut current = None;
        for line in body.lines().map(str::trim) {
            let heading = [DECIDED_HEADING, RULED_OUT_HEADING, WATCH_OUT_HEADING]
                .iter()
                .position(|heading| line.eq_ignore_ascii_case(heading));
            match (heading, current) {
                (Some(at), _) => current = Some(at),
                (None, Some(at)) if !line.is_empty() => {
                    sections[at].push(line.trim_start_matches(['-', '*', ' ']).to_owned())
                }
                _ => {}
            }
        }
        let [decided, ruled_out, watch_out] = sections;
        Self::new(&decided.join(" "), &ruled_out, &watch_out)
    }
}

fn split_at_label<'a>(text: &'a str, labels: &[&str]) -> (&'a str, &'a str) {
    let at = labels.iter().filter_map(|label| text.find(&format!(" {label}"))).min();
    match at {
        Some(at) => (&text[..at], text[at..].trim_start()),
        None => (text, ""),
    }
}

fn tidy(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").trim_end_matches(['.', ';']).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retries() -> DecisionNote {
        DecisionNote::new(
            "Keep defaultRetryMax = 4",
            &["Raising it to 10: one Do() call could block for over an hour.".into()],
            &["RetryWaitMax caps each wait, not the total".into(), "  ".into()],
        )
        .expect("decision")
    }

    #[test]
    fn a_decision_reads_as_one_memory_line_and_back() {
        let decision = retries();
        let text = decision.text();

        assert_eq!(
            text,
            "Decided: Keep defaultRetryMax = 4. Ruled out: Raising it to 10: one Do() call could block for over an \
             hour. Watch out: RetryWaitMax caps each wait, not the total."
        );
        assert_eq!(DecisionNote::parse(&text), Some(decision));
    }

    #[test]
    fn a_decision_without_what_was_ruled_out_still_reads_back() {
        let decision =
            DecisionNote::new("Use pnpm", &[], &["The lockfile is pnpm-lock.yaml".into()]).expect("decision");
        assert_eq!(decision.text(), "Decided: Use pnpm. Watch out: The lockfile is pnpm-lock.yaml.");
        assert_eq!(DecisionNote::parse(&decision.text()), Some(decision));
        assert_eq!(DecisionNote::parse("Use pnpm."), None);
        assert_eq!(DecisionNote::new("  ", &[], &[]), None);
    }

    #[test]
    fn a_decision_is_written_as_markdown_sections_that_read_back() {
        let decision = retries();
        let markdown = decision.markdown();

        assert!(markdown.starts_with("## Decided\n- Keep defaultRetryMax = 4\n\n## Tried and ruled out\n- Raising"));
        assert!(markdown.contains("## Watch out\n- RetryWaitMax caps each wait, not the total\n"));
        assert_eq!(DecisionNote::from_markdown(&markdown), Some(decision));
        assert_eq!(DecisionNote::from_markdown("Just a fact."), None);
    }
}
