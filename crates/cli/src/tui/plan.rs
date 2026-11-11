use domain::command::mode_label;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use sdk::{AskKind, EngineEvent, PermissionMode};
use unicode_width::UnicodeWidthStr;

use super::markdown;
use super::transcript::{Entry, Transcript};

const EXIT_PLAN: &str = "ExitPlanMode";
const ENTER_PLAN: &str = "EnterPlanMode";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Waiting,
    Approved(PermissionMode),
    KeptPlanning,
}

impl Transcript {
    pub(super) fn planned(&mut self, event: &EngineEvent) -> bool {
        match event {
            EngineEvent::ToolStarted(call) if call.name == EXIT_PLAN => {
                self.plan_call = Some(call.id.clone());
                self.activity = Some("Waiting for the plan's approval".into());
            }
            EngineEvent::ToolStarted(call) if call.name == ENTER_PLAN => {}
            EngineEvent::Asked(ask) => match &ask.kind {
                AskKind::Plan { plan } => {
                    self.push(Entry::Plan { plan: plan.clone(), outcome: Outcome::Waiting });
                }
                _ => return false,
            },
            EngineEvent::ModeChanged(PermissionMode::Plan) => self.notice("Entered plan mode"),
            EngineEvent::ModeChanged(mode) => self.settle_plan(Outcome::Approved(*mode)),
            EngineEvent::ToolResult { id, failed, .. } if self.plan_call.as_ref() == Some(id) => {
                if *failed {
                    self.settle_plan(Outcome::KeptPlanning);
                }
                self.plan_call = None;
            }
            EngineEvent::Completed { .. } | EngineEvent::Failed(_) | EngineEvent::Cancelled => {
                self.settle_plan(Outcome::KeptPlanning);
                self.plan_call = None;
                return false;
            }
            _ => return false,
        }
        true
    }

    fn settle_plan(&mut self, settled: Outcome) {
        let waiting = self.entries.iter_mut().rev().find_map(|entry| match entry {
            Entry::Plan { outcome, .. } if *outcome == Outcome::Waiting => Some(outcome),
            _ => None,
        });
        if let Some(outcome) = waiting {
            *outcome = settled;
        }
    }
}

pub fn lines(plan: &str, outcome: Outcome, width: usize, out: &mut Vec<Line<'static>>) {
    let (mark, header) = match outcome {
        Outcome::Waiting => (Color::Cyan, "Plan ready for review".to_owned()),
        Outcome::Approved(mode) => (Color::Green, format!("User approved the plan ({} mode)", mode_label(mode))),
        Outcome::KeptPlanning => (Color::Yellow, "Kept planning: the plan was not approved".to_owned()),
    };
    out.push(Line::from(""));
    out.push(Line::from(vec!["● ".fg(mark), header.bold()]));
    let border = Style::new().fg(if outcome == Outcome::Waiting { Color::Cyan } else { Color::DarkGray });
    let inner = width.saturating_sub(6).max(10);
    out.push(Line::styled(format!("  ╭{}╮", "─".repeat(inner + 2)), border));
    for line in markdown::render(plan.trim(), inner) {
        let used: usize = line.spans.iter().map(|span| span.content.width()).sum();
        let style = line.style;
        let mut spans = vec![Span::styled("  │ ", border)];
        spans.extend(line.spans.into_iter().map(|span| Span::styled(span.content, style.patch(span.style))));
        spans.push(Span::raw(" ".repeat(inner.saturating_sub(used))));
        spans.push(Span::styled(" │", border));
        out.push(Line::from(spans));
    }
    out.push(Line::styled(format!("  ╰{}╯", "─".repeat(inner + 2)), border));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_is_boxed_with_even_sides() {
        let mut out = Vec::new();
        lines("## Steps\n1. A step long enough to wrap inside the box twice over", Outcome::Waiting, 40, &mut out);
        let rows: Vec<String> =
            out.iter().map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect()).collect();
        assert!(rows[1].contains("Plan ready for review"));
        assert!(rows[2].starts_with("  ╭") && rows.last().is_some_and(|row| row.starts_with("  ╰")));
        let widths: Vec<usize> = rows[2..].iter().map(|row| row.width()).collect();
        assert!(widths.iter().all(|&w| w == widths[0]), "{rows:#?}");
        assert!(rows.iter().any(|row| row.contains("Steps")));
    }
}
