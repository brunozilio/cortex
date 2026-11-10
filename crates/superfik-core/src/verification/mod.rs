//! Verification: the criteria a run's work is judged against before it
//! completes, where they come from, and the judge's verdict. Pure.

use serde::{Deserialize, Serialize};

/// What a run's work must satisfy before it completes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Criteria {
    pub text: String,
    pub source: CriteriaSource,
}

/// Who set the criteria: the user (`/verification`) or the last plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CriteriaSource {
    User,
    Plan,
}

impl Criteria {
    pub fn user(text: impl Into<String>) -> Self {
        Self { text: text.into(), source: CriteriaSource::User }
    }

    pub fn plan(text: impl Into<String>) -> Self {
        Self { text: text.into(), source: CriteriaSource::Plan }
    }

    /// The criteria a run is judged against: the user's, else the plan's.
    pub fn resolve(user: Option<&Criteria>, plan: Option<&Criteria>) -> Option<Criteria> {
        user.or(plan).cloned()
    }
}

/// A conversation's criteria: the user's, set with `/verification`, and the
/// last plan's `Verification` section.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Verifications {
    user: Option<Criteria>,
    plan: Option<Criteria>,
}

impl Verifications {
    /// Sets (or, with `None` or blank text, clears) the user's criteria.
    pub fn set_user(&mut self, text: Option<String>) {
        self.user = text.map(|text| text.trim().to_owned()).filter(|text| !text.is_empty()).map(Criteria::user);
    }

    /// A planning run finished with `plan`: its section replaces the last plan's.
    pub fn plan_finished(&mut self, plan: &str) {
        self.plan = plan_verification(plan).map(Criteria::plan);
    }

    pub fn current(&self) -> Option<Criteria> {
        Criteria::resolve(self.user.as_ref(), self.plan.as_ref())
    }

    /// The criteria a run in `mode` is judged against; a planning run never is.
    pub fn for_run(&self, mode: crate::PermissionMode) -> Option<Criteria> {
        (mode != crate::PermissionMode::Plan).then(|| self.current()).flatten()
    }
}

/// How many times the judge grades one run before it completes anyway.
pub const MAX_ATTEMPTS: u8 = 3;

const UNAVAILABLE: &str = "judge unavailable";

/// The judge's grade of a run against its criteria.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub passed: bool,
    /// 0 to 1.
    pub score: f32,
    pub checks: Vec<Check>,
    /// 1 for the first grade of the run.
    pub attempt: u8,
}

/// One criterion, whether it held, and what showed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub criterion: String,
    pub passed: bool,
    #[serde(default)]
    pub evidence: String,
}

#[derive(Deserialize)]
struct Reply {
    passed: bool,
    score: Option<f32>,
    #[serde(default)]
    checks: Vec<Check>,
}

impl Verdict {
    /// Reads the judge's reply: the first JSON object in it, fenced or not.
    pub fn parse(reply: &str, attempt: u8) -> Result<Self, String> {
        let start = reply.find('{').ok_or("the judge returned no JSON verdict")?;
        let end = reply.rfind('}').ok_or("the judge returned no JSON verdict")?;
        let parsed: Reply = serde_json::from_str(reply.get(start..=end).unwrap_or_default())
            .map_err(|error| format!("the judge's verdict is invalid: {error}"))?;
        let score = parsed.score.unwrap_or_else(|| match parsed.checks.len() {
            0 => f32::from(u8::from(parsed.passed)),
            n => parsed.checks.iter().filter(|c| c.passed).count() as f32 / n as f32,
        });
        Ok(Self { passed: parsed.passed, score: score.clamp(0.0, 1.0), checks: parsed.checks, attempt })
    }

    /// A failed verdict for a judge that could not grade (the run completes).
    pub fn unavailable(reason: impl Into<String>, attempt: u8) -> Self {
        let check = Check { criterion: UNAVAILABLE.into(), passed: false, evidence: reason.into() };
        Self { passed: false, score: 0.0, checks: vec![check], attempt }
    }

    /// Whether the judge could not grade at all.
    pub fn is_unavailable(&self) -> bool {
        matches!(&self.checks[..], [check] if check.criterion == UNAVAILABLE)
    }

    /// Whether the agent gets another try after this verdict: it failed a
    /// real grade and attempts remain.
    pub fn should_retry(&self) -> bool {
        !self.passed && !self.is_unavailable() && self.attempt < MAX_ATTEMPTS
    }

    /// The failed checks, as the agent reads them when it tries again.
    pub fn feedback(&self) -> String {
        let failed: Vec<String> = self
            .checks
            .iter()
            .filter(|check| !check.passed)
            .map(|check| format!("- {}: {}", check.criterion, check.evidence))
            .collect();
        format!(
            "Verification failed (attempt {}/{MAX_ATTEMPTS}). Fix these and finish again:\n{}",
            self.attempt,
            failed.join("\n")
        )
    }
}

const SECTION: &str = "verification";

/// The `Verification` section of a plan, as planning mode asks for it: a
/// `**Verification**:` list item (with its indented continuation) or a
/// `## Verification` heading (up to the next heading). `None` when the plan
/// has no such section or it is empty.
pub fn plan_verification(plan: &str) -> Option<String> {
    let lines: Vec<&str> = plan.lines().collect();
    let (start, body) = lines.iter().enumerate().find_map(|(index, line)| {
        heading_title(line)
            .filter(|title| title.eq_ignore_ascii_case(SECTION))
            .map(|_| (index, Section::Heading))
            .or_else(|| bullet_rest(line).map(|rest| (index, Section::Item(indent(line), rest))))
    })?;
    let mut out: Vec<String> = Vec::new();
    match body {
        Section::Heading => {
            for line in &lines[start + 1..] {
                if heading_title(line).is_some() {
                    break;
                }
                out.push(line.trim_end().to_owned());
            }
        }
        Section::Item(depth, rest) => {
            out.push(rest.to_owned());
            for line in &lines[start + 1..] {
                let blank = line.trim().is_empty();
                if !blank && (indent(line) <= depth || heading_title(line).is_some()) {
                    break;
                }
                out.push(line.trim().to_owned());
            }
        }
    }
    let text = out.join("\n").trim().to_owned();
    (!text.is_empty()).then_some(text)
}

enum Section<'a> {
    Heading,
    Item(usize, &'a str),
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn heading_title(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix('#')?.trim_start_matches('#');
    Some(rest.trim().trim_matches('*').trim())
}

/// What follows `**Verification**` in a list item, or `None`.
fn bullet_rest(line: &str) -> Option<&str> {
    let item = line.trim_start();
    let item = item
        .strip_prefix("- ")
        .or_else(|| item.strip_prefix("* "))
        .or_else(|| item.split_once(". ").filter(|(n, _)| n.chars().all(|c| c.is_ascii_digit())).map(|(_, r)| r))?;
    let rest = item.trim_start().strip_prefix("**")?;
    let (name, rest) = rest.split_once("**")?;
    let name = name.trim().trim_end_matches(':');
    name.eq_ignore_ascii_case(SECTION).then(|| rest.trim_start_matches(':').trim())
}

#[cfg(test)]
mod tests;
