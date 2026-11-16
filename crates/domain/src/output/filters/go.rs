use super::{Filter, first_is};

const STEPS: [&str; 4] = ["=== RUN ", "=== PAUSE ", "=== CONT ", "--- PASS: "];

pub(super) const GO_TEST: Filter = Filter {
    name: "go test",
    applies: |words| first_is(words, &["go"]) && words.get(1).is_some_and(|word| word == "test"),
    noise: |line| STEPS.iter().any(|step| line.starts_with(step)),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passing_steps_are_noise_but_failures_and_package_results_are_not() {
        for line in ["=== RUN   TestA", "=== PAUSE TestA", "--- PASS: TestA (0.00s)"] {
            assert!((GO_TEST.noise)(line), "{line}");
        }
        for line in ["--- FAIL: TestB (0.01s)", "ok  \tpkg\t0.1s", "FAIL\tpkg\t0.2s", "a_test.go:12: want 1"] {
            assert!(!(GO_TEST.noise)(line), "{line}");
        }
        assert!((GO_TEST.applies)(&["go".into(), "test".into()]));
        assert!(!(GO_TEST.applies)(&["go".into(), "build".into()]));
    }
}
