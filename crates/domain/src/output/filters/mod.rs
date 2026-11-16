mod git;
mod go;
mod js;
mod python;
mod rust;
mod warnings;

use super::invoked::invocations;

const WORTH_FILTERING: usize = 400;

struct Filter {
    name: &'static str,
    applies: fn(&[String]) -> bool,
    noise: fn(&str) -> bool,
}

const FILTERS: [Filter; 5] = [rust::CARGO, git::GIT, js::JS, python::PYTHON, go::GO_TEST];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filtered {
    pub text: String,
    pub hidden: usize,
    pub by: Vec<&'static str>,
}

impl Filtered {
    pub fn footer(&self, whole: &str) -> String {
        format!(
            "[{} lines of {} noise hidden (passing tests, progress, build steps, repeated warnings); the whole output is in {whole}]",
            self.hidden,
            self.by.join(", ")
        )
    }
}

pub fn filter(command: &str, output: &str) -> Option<Filtered> {
    let invocations = invocations(command)?;
    let chosen: Vec<&Filter> =
        FILTERS.iter().filter(|filter| invocations.iter().any(|words| (filter.applies)(words))).collect();
    if chosen.is_empty() {
        return None;
    }
    let mut hidden = 0;
    let kept: Vec<&str> = output
        .lines()
        .filter(|line| {
            let noise = chosen.iter().any(|filter| (filter.noise)(line.trim()));
            hidden += usize::from(noise);
            !noise
        })
        .collect();
    let mut text = kept.join("\n");
    if chosen.iter().any(|filter| filter.name == rust::CARGO.name) {
        text = warnings::grouped(&text);
        hidden += kept.len().saturating_sub(text.lines().count());
    }
    (output.len().saturating_sub(text.len()) >= WORTH_FILTERING).then(|| Filtered {
        text,
        hidden,
        by: chosen.iter().map(|filter| filter.name).collect(),
    })
}

fn first_is(words: &[String], names: &[&str]) -> bool {
    words.first().is_some_and(|first| names.contains(&first.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passing(count: usize) -> String {
        (0..count).map(|i| format!("test tools::case_{i} ... ok\n")).collect()
    }

    #[test]
    fn noise_is_hidden_and_failures_kept() {
        let output = format!(
            "   Compiling lite v0.1.0\n{}test tools::broken ... FAILED\n\nfailures:\n    panicked at src/a.rs:3\ntest result: FAILED. 40 passed; 1 failed",
            passing(40)
        );
        let filtered = filter("cargo test --workspace", &output).expect("filtered");
        assert_eq!(
            filtered.text,
            "test tools::broken ... FAILED\n\nfailures:\n    panicked at src/a.rs:3\ntest result: FAILED. 40 passed; 1 failed"
        );
        assert_eq!(filtered.hidden, 41);
        assert_eq!(filtered.by, ["cargo"]);
        assert_eq!(
            filtered.footer("/tmp/out.txt"),
            "[41 lines of cargo noise hidden (passing tests, progress, build steps, repeated warnings); the whole output is in /tmp/out.txt]"
        );
    }

    #[test]
    fn cargo_shows_a_repeated_warning_once() {
        let warning = |at: usize| {
            format!(
                "warning: unused variable: `v{at}`\n --> src/f{at}.rs:1:9\n  |\n1 | let v{at} = 1;\n  |     ^^ help: if this is intentional, prefix it with an underscore"
            )
        };
        let output = (0..6).map(warning).collect::<Vec<_>>().join("\n\n");
        let filtered = filter("cargo clippy --workspace", &output).expect("filtered");
        assert_eq!(
            filtered.text,
            format!(
                "{}\n… [5 more of this warning: src/f1.rs:1:9, src/f2.rs:1:9, src/f3.rs:1:9, src/f4.rs:1:9, src/f5.rs:1:9]",
                warning(0)
            )
        );
        assert_eq!(filtered.hidden, 29);
        assert_eq!(filter("npm test", &output), None);
    }

    #[test]
    fn a_small_saving_or_an_unknown_command_leaves_the_output_alone() {
        assert_eq!(filter("cargo test", &passing(3)), None);
        assert_eq!(filter("ls -la", &passing(40)), None);
        assert_eq!(filter("cargo test 2>&1 | grep ok", &passing(40)), None);
    }

    #[test]
    fn every_part_of_a_chained_command_brings_its_filter() {
        let output = format!("{}=== RUN   TestA\n--- PASS: TestA (0.00s)\nok  \tpkg\t0.1s", passing(40));
        let filtered = filter("cargo test && go test -v ./...", &output).expect("filtered");
        assert_eq!(filtered.text, "ok  \tpkg\t0.1s");
        assert_eq!(filtered.by, ["cargo", "go test"]);
    }
}
