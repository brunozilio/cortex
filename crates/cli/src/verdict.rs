//! How a verification verdict reads, in the chat and in `-p`.

use domain::verification::MAX_ATTEMPTS;
use sdk::Verdict;

pub fn line(verdict: &Verdict) -> String {
    if verdict.passed {
        return format!("✓ verification passed (score {:.2})", verdict.score);
    }
    let failed: Vec<String> = verdict
        .checks
        .iter()
        .filter(|check| !check.passed)
        .map(|check| format!("\n  ✗ {}: {}", check.criterion, check.evidence))
        .collect();
    let next = if verdict.should_retry() { " · the agent tries again" } else { "" };
    format!(
        "✗ verification failed, attempt {}/{MAX_ATTEMPTS} (score {:.2}){next}{}",
        verdict.attempt,
        verdict.score,
        failed.concat()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_passed_verdict_is_one_line() {
        let verdict = Verdict::parse(r#"{"passed":true,"score":0.9}"#, 1).expect("verdict");
        assert_eq!(line(&verdict), "✓ verification passed (score 0.90)");
    }

    #[test]
    fn a_failed_verdict_lists_its_failed_checks_and_whether_the_agent_retries() {
        let reply = r#"{"passed":false,"checks":[{"criterion":"tests","passed":false,"evidence":"1 failed"},
            {"criterion":"lint","passed":true}]}"#;
        assert_eq!(
            line(&Verdict::parse(reply, 1).expect("verdict")),
            "✗ verification failed, attempt 1/3 (score 0.50) · the agent tries again\n  ✗ tests: 1 failed"
        );
        assert!(!line(&Verdict::parse(reply, 3).expect("verdict")).contains("tries again"));
    }
}
