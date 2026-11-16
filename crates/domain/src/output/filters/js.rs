use super::{Filter, first_is};

const PASSED: [&str; 4] = ["✓ ", "√ ", "✔ ", "PASS "];

pub(super) const JS: Filter = Filter {
    name: "javascript",
    applies: |words| first_is(words, &["npm", "pnpm", "yarn", "bun", "jest", "vitest"]),
    noise,
};

fn noise(line: &str) -> bool {
    PASSED.iter().any(|mark| line.starts_with(mark))
        || line.starts_with("Progress: resolved")
        || (line.len() > 3 && line.chars().all(|c| c == '+'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passing_tests_and_install_progress_are_noise_but_failures_are_not() {
        for line in
            ["✓ src/a.test.ts (5 tests) 12ms", "PASS src/b.test.js", "Progress: resolved 12, reused 10", "++++++++"]
        {
            assert!(noise(line), "{line}");
        }
        for line in ["✗ adds numbers", "FAIL src/c.test.js", "Packages: +12", "added 3 packages", "+++"] {
            assert!(!noise(line), "{line}");
        }
    }
}
