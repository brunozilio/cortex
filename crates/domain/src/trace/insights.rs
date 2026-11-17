mod report;

use std::cmp::Reverse;
use std::collections::BTreeMap;

pub use self::report::{
    CheckKind, Checks, Day, DayModel, Finding, Insights, ModelStat, Report, SessionRuns, TimeShare, ToolStat, Totals,
};
use super::runs::{JUDGE, REVIEW, Run, Span, SpanKind, dollars, marked};
use super::{Mark, Title};

pub const DAY_MS: u64 = 86_400_000;
pub const WINDOW_DAYS: u64 = 30;
const FINDINGS: usize = 50;

pub fn recent(run: &Run, now_ms: u64) -> bool {
    run.started_at_ms + WINDOW_DAYS * DAY_MS > now_ms
}

fn percentile(mut values: Vec<u64>, share: f64) -> u64 {
    values.sort_unstable();
    let last = values.len().saturating_sub(1);
    values.get((last as f64 * share).round() as usize).copied().unwrap_or_default()
}

fn ratio(part: u64, whole: u64) -> f64 {
    if whole == 0 { 0.0 } else { part as f64 / whole as f64 }
}

pub fn insights(runs: &[&Run], now_ms: u64) -> Insights {
    let runs: Vec<&Run> = runs.iter().copied().filter(|run| recent(run, now_ms)).collect();
    let spans = || runs.iter().flat_map(|run| run.spans.iter().map(move |span| (*run, span)));
    let tools: Vec<&Span> = spans().map(|(_, span)| span).filter(|span| span.kind == SpanKind::Tool).collect();
    let input: u64 = runs.iter().map(|run| run.input).sum();
    Insights {
        days: days(&runs, now_ms),
        models: models(&runs),
        tools: tool_stats(&runs),
        time: time(&runs),
        checks: checks(&runs),
        totals: Totals {
            runs: runs.len() as u64,
            cost_usd: dollars(runs.iter().map(|run| run.cost_usd)),
            tokens: runs.iter().map(|run| run.tokens).sum(),
            waiting_ms: runs.iter().map(|run| run.waiting_ms).sum(),
            p95_run_ms: percentile(runs.iter().map(|run| run.duration_ms).collect(), 0.95),
            tool_error_rate: ratio(tools.iter().filter(|span| span.failed).count() as u64, tools.len() as u64),
            cache_ratio: ratio(runs.iter().map(|run| run.cache_read).sum(), input),
        },
        findings: findings(&runs),
    }
}

fn days(runs: &[&Run], now_ms: u64) -> Vec<Day> {
    let today = now_ms / DAY_MS;
    let mut models: Vec<String> =
        runs.iter().flat_map(|run| run.calls().map(|(_, usage)| usage.model.clone())).collect();
    models.sort();
    models.dedup();
    (today + 1 - WINDOW_DAYS..=today)
        .map(|day| {
            let on: Vec<&&Run> = runs.iter().filter(|run| run.started_at_ms / DAY_MS == day).collect();
            let models = models
                .iter()
                .map(|model| {
                    let calls = || on.iter().flat_map(|run| run.calls()).filter(|(_, usage)| &usage.model == model);
                    DayModel {
                        model: model.clone(),
                        cost_usd: dollars(calls().map(|(_, usage)| usage.cost_usd)),
                        tokens: calls().map(|(_, usage)| usage.prompt() + usage.output).sum(),
                        runs: on.iter().filter(|run| &run.model == model).count() as u64,
                    }
                })
                .collect();
            Day { day, models }
        })
        .collect()
}

fn models(runs: &[&Run]) -> Vec<ModelStat> {
    let mut by: BTreeMap<String, Vec<(&Span, u64, u64, f64)>> = BTreeMap::new();
    for (span, usage) in runs.iter().flat_map(|run| run.calls()) {
        by.entry(usage.model.clone()).or_default().push((span, usage.cache_read, usage.prompt(), usage.cost_usd));
    }
    let mut stats: Vec<ModelStat> = by
        .into_iter()
        .map(|(model, calls)| ModelStat {
            model,
            calls: calls.len() as u64,
            p50_ms: percentile(calls.iter().map(|(span, ..)| span.duration()).collect(), 0.5),
            p95_ms: percentile(calls.iter().map(|(span, ..)| span.duration()).collect(), 0.95),
            cache_ratio: ratio(calls.iter().map(|c| c.1).sum(), calls.iter().map(|c| c.2).sum()),
            cost_usd: dollars(calls.iter().map(|c| c.3)),
        })
        .collect();
    stats.sort_by(|a, b| b.cost_usd.total_cmp(&a.cost_usd));
    stats
}

fn tool_stats(runs: &[&Run]) -> Vec<ToolStat> {
    let mut by: BTreeMap<String, Vec<(&Run, &Span)>> = BTreeMap::new();
    for run in runs {
        for span in run.spans.iter().filter(|span| span.kind == SpanKind::Tool) {
            by.entry(span.attr("tool").unwrap_or(&span.name).to_owned()).or_default().push((run, span));
        }
    }
    let mut stats: Vec<ToolStat> = by
        .into_iter()
        .map(|(name, calls)| ToolStat {
            name,
            calls: calls.len() as u64,
            failed: calls.iter().filter(|(_, span)| span.failed).count() as u64,
            p95_ms: percentile(calls.iter().map(|(_, span)| span.duration()).collect(), 0.95),
            asked: calls
                .iter()
                .map(|(run, tool)| {
                    run.spans.iter().filter(|span| span.kind == SpanKind::Wait && span.parent == tool.id).count() as u64
                })
                .sum(),
        })
        .collect();
    stats.sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.name.cmp(&b.name)));
    stats
}

fn time(runs: &[&Run]) -> Vec<TimeShare> {
    let mut by: BTreeMap<SpanKind, u64> = BTreeMap::new();
    for run in runs {
        for span in &run.spans {
            if span.kind == SpanKind::Wait {
                *by.entry(SpanKind::Wait).or_default() += span.duration();
                let tool = run.spans.iter().find(|parent| parent.id == span.parent && parent.parent == 1);
                if let Some(tool) = tool {
                    let spent = by.entry(tool.kind).or_default();
                    *spent = spent.saturating_sub(span.duration());
                }
            } else if span.parent == 1 && span.kind != SpanKind::Event {
                *by.entry(span.kind).or_default() += span.duration();
            }
        }
    }
    let mut shares: Vec<TimeShare> =
        by.into_iter().filter(|(_, ms)| *ms > 0).map(|(kind, ms)| TimeShare { kind, ms }).collect();
    shares.sort_by_key(|share| Reverse(share.ms));
    shares
}

fn checks(runs: &[&Run]) -> Checks {
    let spans = || runs.iter().flat_map(|run| &run.spans);
    let flagged = |title: Title| spans().flat_map(|span| &span.flags).filter(|flag| marked(flag, title)).count();
    let judged = |failed: bool| spans().filter(|span| span.name == JUDGE && span.failed == failed).count();
    Checks {
        verified_passed: judged(false) as u64,
        verified_failed: judged(true) as u64,
        review_findings: spans()
            .filter(|span| span.name == REVIEW)
            .map(|span| span.output.lines().count())
            .sum::<usize>() as u64,
        repeated: flagged(Title::Repeated) as u64,
        refused: flagged(Title::Refused) as u64,
        asked: spans().filter(|span| span.kind == SpanKind::Wait).count() as u64,
    }
}

fn findings(runs: &[&Run]) -> Vec<Finding> {
    let mut found = Vec::new();
    for run in runs {
        for span in &run.spans {
            let finding = |check: CheckKind, text: &str, severe: bool| Finding {
                session: run.session.clone(),
                run: run.index,
                span: span.id,
                check,
                text: text.to_owned(),
                at_ms: run.started_at_ms + span.start_ms,
                severe,
            };
            if span.name == JUDGE && span.failed {
                let failing = span.output.lines().find(|line| line.starts_with("fail")).unwrap_or(&span.output);
                found.push(finding(CheckKind::Judge, failing.trim_start_matches("fail ").trim(), true));
            }
            if span.name == REVIEW {
                for line in span.output.lines() {
                    let (severity, issue) = line.split_once(' ').unwrap_or(("low", line));
                    found.push(finding(CheckKind::Review, issue, severity == "high"));
                }
            }
            for mark in span.flags.iter().filter_map(|flag| Mark::of(flag)) {
                match mark {
                    Mark::Repeated => found.push(finding(
                        CheckKind::Repeated,
                        &format!("{} was repeated with the same failure", span.name),
                        false,
                    )),
                    Mark::Refused(reason) => found.push(finding(CheckKind::Refused, &reason, false)),
                    Mark::Edited(_) => {}
                }
            }
        }
    }
    found.sort_by_key(|finding| Reverse(finding.at_ms));
    found.truncate(FINDINGS);
    found
}

#[cfg(test)]
mod tests;
