use super::{Filter, first_is};

const PIP_STEPS: [&str; 5] = [
    "Requirement already satisfied:",
    "Collecting ",
    "Downloading ",
    "Using cached ",
    "Obtaining dependency information",
];

pub(super) const PYTHON: Filter = Filter {
    name: "python",
    applies: |words| {
        first_is(words, &["pytest", "py.test", "pip", "pip3"])
            || (first_is(words, &["uv"]) && words.get(1).is_some_and(|w| w == "pip"))
    },
    noise,
};

fn noise(line: &str) -> bool {
    (line.contains("::") && line.split_whitespace().any(|word| word == "PASSED"))
        || only_passed_marks(line)
        || PIP_STEPS.iter().any(|step| line.starts_with(step))
}

fn only_passed_marks(line: &str) -> bool {
    let Some((head, percent)) = line.rsplit_once('[') else { return false };
    let mut words = head.split_whitespace();
    let (Some(path), Some(marks), None) = (words.next(), words.next(), words.next()) else { return false };
    percent.trim_end().ends_with("%]") && path.ends_with(".py") && marks.chars().all(|c| c == '.' || c == 's')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passing_tests_and_pip_steps_are_noise_but_failures_are_not() {
        for line in [
            "tests/test_a.py::test_one PASSED                [ 10%]",
            "tests/test_a.py ....s...                       [ 50%]",
            "Requirement already satisfied: requests in ./venv",
            "Collecting httpx",
        ] {
            assert!(noise(line), "{line}");
        }
        for line in [
            "tests/test_a.py::test_two FAILED                [ 20%]",
            "tests/test_b.py ..F.                            [100%]",
            "E       assert 1 == 2",
            "Successfully installed httpx-0.27.0",
            "=== 3 passed in 0.1s ===",
        ] {
            assert!(!noise(line), "{line}");
        }
    }
}
