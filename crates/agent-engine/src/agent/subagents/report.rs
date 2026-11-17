use domain::AgentStatus;

use crate::agent::Ending;
use crate::tools::Outcome;
use crate::transcript::{Block, Message, Role};

pub(super) fn last_said(history: &[Message], own: usize) -> String {
    let report = history
        .get(if history.len() >= own { own } else { 0 }..)
        .unwrap_or_default()
        .iter()
        .rev()
        .find(|message| message.role == Role::Assistant)
        .map(Message::text)
        .unwrap_or_default();
    let read_outside = history
        .get(own.min(history.len())..)
        .unwrap_or_default()
        .iter()
        .flat_map(|message| &message.blocks)
        .any(|block| matches!(block, Block::ToolUse { name, .. } if domain::untrusted::reads_outside(name)));
    if read_outside && !report.trim().is_empty() { domain::untrusted::relayed(&report) } else { report }
}

pub(super) fn how_it_ran(unisolated: bool, unserved: Option<&str>, model: &str) -> String {
    let mut notes = String::new();
    if unisolated {
        notes.push_str(&format!("\n\n{}", crate::agent_runtime::RAN_WITHOUT_ISOLATION));
    }
    if let Some(asked) = unserved {
        notes.push_str(&format!("\n\n{asked} is not a model this provider serves; ran on {model}."));
    }
    notes
}

pub(super) fn stopped(ending: &Ending, unfinished: String, report: String) -> (AgentStatus, Outcome) {
    let status = if *ending == Ending::Cancelled { AgentStatus::Cancelled } else { AgentStatus::Failed };
    let said = if unfinished.trim().is_empty() { report } else { unfinished };
    (status, Outcome::error(format!("The sub-agent stopped: {ending:?}. What it said last: {said}")))
}
