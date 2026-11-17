use super::{Builder, node};
use crate::review::{Finding, Severity};
use crate::trace::{Kind, Mark, Node, Title};
use crate::verification::Verdict;
use crate::{AskId, Decision};

pub fn decided(decision: &Decision) -> String {
    match decision {
        Decision::Allow => "allowed once".into(),
        Decision::AllowAlways => "always allowed".into(),
        Decision::Deny => "denied".into(),
        Decision::DenyWith(note) => format!("denied: {note}"),
        Decision::Answer(_) => "answered".into(),
        Decision::Approve(mode) => format!("approved in {mode:?}"),
    }
}

impl Builder<'_> {
    fn latest_tool(&mut self, matches: impl Fn(&Node) -> bool) -> Option<&mut Node> {
        let earlier = self.steps.iter_mut().rev().flat_map(|step| step.nodes.iter_mut().rev());
        self.group.iter_mut().rev().chain(earlier).find(|node| matches!(node.kind, Kind::Tool { .. }) && matches(node))
    }

    pub(super) fn mark(&mut self, at: u64, tool: &str, mark: Mark) {
        let named = |node: &Node| tool.is_empty() || matches!(&node.kind, Kind::Tool { call, .. } if call.name == tool);
        match self.latest_tool(named).map(|node| &mut node.kind) {
            Some(Kind::Tool { notes, .. }) => notes.push(mark),
            _ => self.note(at, mark.title(), mark.to_string()),
        }
    }

    pub(super) fn answered(&mut self, at: u64, ask: &AskId, decision: &Decision) {
        let asked = |node: &Node| matches!(&node.kind, Kind::Tool { asks, .. } if asks.iter().any(|a| &a.id == ask));
        if let Some(Kind::Tool { asks, waits, .. }) = self.latest_tool(asked).map(|node| &mut node.kind)
            && let Some(wait) = asks.iter().position(|a| &a.id == ask).and_then(|index| waits.get_mut(index))
        {
            wait.answered_ms = Some(at);
            wait.decision = Some(decided(decision));
        }
    }

    pub(super) fn verified(&mut self, at: u64, verdict: &Verdict) {
        let lines: Vec<String> = verdict
            .checks
            .iter()
            .map(|check| {
                let result = if check.passed { "pass" } else { "fail" };
                if check.evidence.is_empty() {
                    format!("{result} {}", check.criterion)
                } else {
                    format!("{result} {}: {}", check.criterion, check.evidence)
                }
            })
            .collect();
        let kind = Kind::Note { title: Title::Verified, text: lines.join("\n") };
        self.single(node(at, !verdict.passed, kind));
    }

    pub(super) fn reviewed(&mut self, at: u64, findings: &[Finding]) {
        let lines: Vec<String> = findings
            .iter()
            .map(|finding| {
                let severity = if finding.severity == Severity::High { "high" } else { "low" };
                let place = finding.line.map_or(finding.file.clone(), |line| format!("{}:{line}", finding.file));
                format!("{severity} {place} {}", finding.issue)
            })
            .collect();
        let severe = findings.iter().any(|finding| finding.severity == Severity::High);
        self.single(node(at, severe, Kind::Note { title: Title::Reviewed, text: lines.join("\n") }));
    }
}
