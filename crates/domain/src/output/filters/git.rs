use super::{Filter, first_is};

const PROGRESS: [&str; 8] = [
    "Enumerating objects:",
    "Counting objects:",
    "Compressing objects:",
    "Writing objects:",
    "Receiving objects:",
    "Resolving deltas:",
    "Unpacking objects:",
    "Delta compression using",
];

pub(super) const GIT: Filter = Filter { name: "git", applies: |words| first_is(words, &["git"]), noise };

fn noise(line: &str) -> bool {
    let progress = line.strip_prefix("remote: ").unwrap_or(line);
    line.starts_with("(use \"git ")
        || line.starts_with("(commit or discard the untracked or modified content")
        || PROGRESS.iter().any(|step| progress.starts_with(step))
        || (progress.starts_with("Total ") && progress.contains("(delta "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_and_transfer_progress_are_noise_but_the_state_is_not() {
        for line in [
            "(use \"git restore <file>...\" to discard changes in working directory)",
            "remote: Counting objects: 100% (5/5), done.",
            "Writing objects: 100% (3/3), 300 bytes | 300.00 KiB/s, done.",
            "Total 3 (delta 2), reused 0 (delta 0), pack-reused 0",
        ] {
            assert!(noise(line), "{line}");
        }
        for line in ["modified:   src/a.rs", "Changes not staged for commit:", "Total cost: 3", "To github.com:o/r.git"]
        {
            assert!(!noise(line), "{line}");
        }
    }
}
