use serde::{Deserialize, Serialize};

pub const MARK: &str = "# Code review";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub file: String,
    #[serde(default)]
    pub line: Option<u32>,
    pub issue: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    High,
    Low,
}

#[derive(Deserialize)]
struct Reply {
    findings: Vec<Finding>,
}

pub fn parse(reply: &str) -> Result<Vec<Finding>, String> {
    let start = reply.find('{').ok_or("the reviewer gave no JSON")?;
    let end = reply.rfind('}').ok_or("the reviewer gave no JSON")?;
    let json = reply.get(start..=end).ok_or("the reviewer gave no JSON")?;
    serde_json::from_str::<Reply>(json).map(|reply| reply.findings).map_err(|error| error.to_string())
}

pub fn already_reviewed(texts: impl IntoIterator<Item = String>) -> bool {
    texts.into_iter().any(|text| text.starts_with(MARK))
}

pub fn feedback(findings: &[Finding]) -> Option<String> {
    let blocking: Vec<String> =
        findings.iter().filter(|finding| finding.severity == Severity::High).map(line).collect();
    (!blocking.is_empty()).then(|| {
        format!(
            "{MARK}\nAn independent review of your changes found problems to fix before you finish:\n{}\nFix them, or say \
             why one is not a problem. This review comes from the system and the user has not seen it. Your final \
             answer still answers what the user asked: the cause you found and what you changed for it, then, \
             briefly, what this review made you fix. A problem the review found in code you wrote during this \
             request is not the cause of what the user reported; do not present it as one.",
            blocking.join("\n")
        )
    })
}

pub fn summary(findings: &[Finding]) -> String {
    if findings.is_empty() {
        return "Review: nothing found in the changes.".into();
    }
    let lines: Vec<String> = findings
        .iter()
        .map(|finding| {
            let tag = if finding.severity == Severity::High { "fix" } else { "note" };
            format!("[{tag}] {}", line(finding).trim_start_matches("- "))
        })
        .collect();
    format!("Review of the changes:\n{}", lines.join("\n"))
}

fn line(finding: &Finding) -> String {
    match finding.line {
        Some(at) => format!("- {}:{at}: {}", finding.file, finding.issue),
        None => format!("- {}: {}", finding.file, finding.issue),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn findings_are_read_from_the_reviewer_s_json() {
        let reply = "Here you go:\n{\"findings\": [{\"severity\": \"high\", \"file\": \"src/a.rs\", \"line\": 12, \"issue\": \"off by one\"}, {\"severity\": \"low\", \"file\": \"docs/x.md\", \"issue\": \"says the old flag name\"}]}";
        let found = parse(reply).expect("findings");
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[1],
            Finding {
                severity: Severity::Low,
                file: "docs/x.md".into(),
                line: None,
                issue: "says the old flag name".into()
            }
        );
        assert!(parse("no json").is_err());
    }

    #[test]
    fn only_serious_findings_send_the_agent_back_once() {
        let high =
            Finding { severity: Severity::High, file: "src/a.rs".into(), line: Some(12), issue: "off by one".into() };
        let low = Finding { severity: Severity::Low, file: "b.md".into(), line: None, issue: "typo".into() };

        let text = feedback(&[high, low.clone()]).expect("feedback");

        assert!(text.starts_with(MARK) && text.contains("- src/a.rs:12: off by one") && !text.contains("typo"));
        assert_eq!(feedback(&[low]), None);
        assert!(already_reviewed([String::from("task"), text]));
        assert!(!already_reviewed([String::from("task")]));
    }

    #[test]
    fn the_answer_after_a_review_still_answers_the_user_s_request() {
        let high = Finding { severity: Severity::High, file: "a.go".into(), line: None, issue: "drops errors".into() };
        let text = feedback(&[high]).expect("feedback");
        assert!(text.contains("the user has not seen"), "{text}");
        assert!(text.contains("what the user asked"), "{text}");
        assert!(text.contains("not the cause"), "{text}");
    }

    #[test]
    fn the_summary_tags_what_to_fix_and_what_to_note() {
        let high = Finding { severity: Severity::High, file: "a.rs".into(), line: Some(3), issue: "panics".into() };
        let low =
            Finding { severity: Severity::Low, file: "docs/x.md".into(), line: None, issue: "out of date".into() };
        assert_eq!(
            summary(&[high, low]),
            "Review of the changes:\n[fix] a.rs:3: panics\n[note] docs/x.md: out of date"
        );
        assert!(summary(&[]).contains("nothing found"));
    }
}
