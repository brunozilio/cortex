use serde::Serialize;

use crate::trace::runs::{Run, SpanKind};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionRuns {
    pub id: String,
    pub runs: Vec<Run>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub project: String,
    pub current: String,
    pub generated_at_ms: u64,
    pub sessions: Vec<SessionRuns>,
    pub insights: Insights,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DayModel {
    pub model: String,
    pub cost_usd: f64,
    pub tokens: u64,
    pub runs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Day {
    pub day: u64,
    pub models: Vec<DayModel>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelStat {
    pub model: String,
    pub calls: u64,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub cache_ratio: f64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolStat {
    pub name: String,
    pub calls: u64,
    pub failed: u64,
    pub p95_ms: u64,
    pub asked: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Checks {
    pub verified_passed: u64,
    pub verified_failed: u64,
    pub review_findings: u64,
    pub repeated: u64,
    pub refused: u64,
    pub asked: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Totals {
    pub runs: u64,
    pub cost_usd: f64,
    pub tokens: u64,
    pub waiting_ms: u64,
    pub p95_run_ms: u64,
    pub tool_error_rate: f64,
    pub cache_ratio: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    Judge,
    Review,
    Repeated,
    Refused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub session: String,
    pub run: usize,
    pub span: u32,
    pub check: CheckKind,
    pub text: String,
    pub at_ms: u64,
    pub severe: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimeShare {
    pub kind: SpanKind,
    pub ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Insights {
    pub days: Vec<Day>,
    pub models: Vec<ModelStat>,
    pub tools: Vec<ToolStat>,
    pub time: Vec<TimeShare>,
    pub checks: Checks,
    pub totals: Totals,
    pub findings: Vec<Finding>,
}
