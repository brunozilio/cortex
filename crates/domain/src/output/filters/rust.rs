use super::{Filter, first_is};

const STEPS: [&str; 9] = [
    "Compiling ",
    "Checking ",
    "Documenting ",
    "Downloaded ",
    "Downloading ",
    "Fresh ",
    "Blocking waiting for file lock",
    "Updating crates.io index",
    "Updating git repository",
];

pub(super) const CARGO: Filter = Filter { name: "cargo", applies: |words| first_is(words, &["cargo"]), noise };

fn noise(line: &str) -> bool {
    STEPS.iter().any(|step| line.starts_with(step))
        || (line.starts_with("test ") && line.ends_with(" ... ok"))
        || line.starts_with("PASS [")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_steps_and_passing_tests_are_noise_but_problems_are_not() {
        for line in ["Compiling serde v1.0.0", "Checking domain v0.1.0", "test a::b ... ok", "PASS [   0.004s] lite::x"]
        {
            assert!(noise(line), "{line}");
        }
        for line in [
            "test a::b ... FAILED",
            "warning: unused variable",
            "error[E0308]: mismatched types",
            "test result: ok. 3 passed",
            "Finished `test` profile",
        ] {
            assert!(!noise(line), "{line}");
        }
    }
}
