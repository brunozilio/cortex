use std::collections::BTreeMap;

use crate::usage::compact_tokens;

const CHARS_PER_TOKEN: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Saving {
    pub program: String,
    pub before: usize,
    pub after: usize,
}

#[derive(Default)]
struct Total {
    runs: usize,
    before: usize,
    after: usize,
}

impl Total {
    fn add(&mut self, saving: &Saving) {
        self.runs += 1;
        self.before += saving.before;
        self.after += saving.after.min(saving.before);
    }

    fn saved(&self) -> usize {
        self.before - self.after
    }

    fn percent(&self) -> usize {
        (self.saved() * 100).checked_div(self.before).unwrap_or(0)
    }
}

fn amount(chars: usize) -> String {
    compact_tokens(chars as u64)
}

pub fn report(savings: &[Saving]) -> String {
    if savings.is_empty() {
        return "No command output measured in this session yet.".into();
    }
    let mut all = Total::default();
    let mut by: BTreeMap<&str, Total> = BTreeMap::new();
    for saving in savings {
        all.add(saving);
        by.entry(&saving.program).or_default().add(saving);
    }
    let mut rows: Vec<(&str, Total)> = by.into_iter().collect();
    rows.sort_by(|a, b| b.1.saved().cmp(&a.1.saved()).then(a.0.cmp(b.0)));
    let header = format!(
        "Command output this session: {} → {} characters, {}% shorter (about {} tokens kept out of the conversation)",
        amount(all.before),
        amount(all.after),
        all.percent(),
        amount(all.saved() / CHARS_PER_TOKEN),
    );
    let lines = rows.iter().map(|(program, total)| {
        let runs = if total.runs == 1 { "run " } else { "runs" };
        format!(
            "  {program:<12} {:>4} {runs}  {:>6} → {:<6} {:>3}%",
            total.runs,
            amount(total.before),
            amount(total.after),
            total.percent()
        )
    });
    std::iter::once(header).chain(lines).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saving(program: &str, before: usize, after: usize) -> Saving {
        Saving { program: program.into(), before, after }
    }

    #[test]
    fn nothing_measured_says_so() {
        assert_eq!(report(&[]), "No command output measured in this session yet.");
    }

    #[test]
    fn the_report_adds_up_each_program_with_the_biggest_saving_first() {
        let shown =
            report(&[saving("git", 1_000, 900), saving("cargo", 40_000, 4_000), saving("cargo", 10_000, 6_000)]);
        let lines: Vec<&str> = shown.lines().collect();
        assert_eq!(
            lines[0],
            "Command output this session: 51k → 10.9k characters, 78% shorter (about 10k tokens kept out of the conversation)"
        );
        assert_eq!(lines[1], "  cargo           2 runs     50k → 10k     80%");
        assert_eq!(lines[2], "  git             1 run       1k → 900     10%");
    }

    #[test]
    fn longer_output_never_counts_as_a_negative_saving() {
        assert!(report(&[saving("ls", 10, 30)]).contains("0% shorter"));
        assert!(report(&[saving("true", 0, 0)]).contains("0% shorter"));
    }
}
