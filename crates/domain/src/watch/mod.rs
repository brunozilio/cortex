use std::time::Duration;

use serde_json::Value;

pub const LEAST_EVERY: u64 = 60;
pub const DEFAULT_EVERY: u64 = 300;
pub const MOST_EVERY: u64 = 86_400;
pub const LASTS: Duration = Duration::from_secs(7 * 24 * 60 * 60);
pub const FAILURES: u32 = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub tool: String,
    pub input: Value,
    pub every: Duration,
    pub path: Option<String>,
    pub until: Until,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Until {
    Equals(Vec<String>),
    Contains(String),
    Changes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Wait,
    Met(String),
    GaveUp(String),
}

impl Plan {
    pub fn parse(input: &Value) -> Result<Self, String> {
        let text = |key: &str| input.get(key).and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty());
        let tool = text("tool").ok_or("Watch needs `tool`: the name of the tool to call on each check.")?.to_owned();
        let arguments = input.get("input").cloned().unwrap_or_else(|| serde_json::json!({}));
        if !arguments.is_object() {
            return Err("Watch's `input` must be an object: the arguments for the watched tool.".into());
        }
        let seconds = match input.get("every_seconds") {
            None | Some(Value::Null) => DEFAULT_EVERY,
            Some(value) => value.as_u64().ok_or("`every_seconds` must be a whole number of seconds.")?,
        };
        if !(LEAST_EVERY..=MOST_EVERY).contains(&seconds) {
            return Err(format!("`every_seconds` must be between {LEAST_EVERY} and {MOST_EVERY}."));
        }
        let until = match (input.get("equals"), text("contains")) {
            (Some(_), Some(_)) => return Err("Give `equals` or `contains`, not both.".into()),
            (Some(equals), None) => Until::Equals(expected(equals)?),
            (None, Some(contains)) => Until::Contains(contains.to_owned()),
            (None, None) => Until::Changes,
        };
        let path = text("path").map(str::to_owned);
        let description = text("description").map_or_else(|| tool.clone(), str::to_owned);
        Ok(Self { tool, input: arguments, every: Duration::from_secs(seconds), path, until, description })
    }

    pub fn explained(&self) -> String {
        let looked_at = self.path.as_deref().map_or_else(|| "its result".to_owned(), |path| format!("`{path}`"));
        let when = match &self.until {
            Until::Equals(values) => format!("{looked_at} equals {}", quoted(values)),
            Until::Contains(text) => format!("{looked_at} contains \"{text}\""),
            Until::Changes => format!("{looked_at} changes"),
        };
        format!("{} every {}s until {when}", self.tool, self.every.as_secs())
    }
}

fn expected(equals: &Value) -> Result<Vec<String>, String> {
    let values: Vec<String> = match equals {
        Value::Array(items) => items.iter().map(shown).collect(),
        other => vec![shown(other)],
    };
    if values.iter().all(|value| value.trim().is_empty()) {
        return Err("`equals` needs at least one value.".into());
    }
    Ok(values)
}

fn quoted(values: &[String]) -> String {
    values.iter().map(|value| format!("\"{value}\"")).collect::<Vec<_>>().join(" or ")
}

fn shown(value: &Value) -> String {
    match value {
        Value::String(text) => text.trim().to_owned(),
        other => other.to_string(),
    }
}

pub fn read(output: &str, path: Option<&str>) -> Result<String, String> {
    let Some(path) = path else { return Ok(output.trim().to_owned()) };
    let parts = std::iter::once(output).chain(output.split("\n\n"));
    let after_heading = |part: &'_ str| part.split_once('\n').map(|(_, rest)| rest.to_owned());
    parts
        .flat_map(|part| std::iter::once(part.to_owned()).chain(after_heading(part)))
        .collect::<Vec<_>>()
        .iter()
        .filter_map(|document| serde_json::from_str::<Value>(document.trim()).ok())
        .find_map(|document| at(&document, path).map(shown))
        .ok_or_else(|| format!("the result has no `{path}`"))
}

fn at<'a>(document: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').filter(|key| !key.is_empty()).try_fold(document, |value, key| match value {
        Value::Array(items) => key.parse::<usize>().ok().and_then(|index| items.get(index)),
        other => other.get(key),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Watching {
    pub plan: Plan,
    first: Option<String>,
    failures: u32,
}

impl Watching {
    pub fn new(plan: Plan) -> Self {
        Self { plan, first: None, failures: 0 }
    }

    pub fn observe(&mut self, result: Result<&str, &str>) -> Step {
        let value = result.map_err(str::to_owned).and_then(|output| read(output, self.plan.path.as_deref()));
        let value = match value {
            Ok(value) => value,
            Err(error) => {
                self.failures += 1;
                return if self.failures >= FAILURES {
                    Step::GaveUp(format!("the last {FAILURES} checks failed: {error}"))
                } else {
                    Step::Wait
                };
            }
        };
        self.failures = 0;
        let first = self.first.get_or_insert_with(|| value.clone());
        let met = match &self.plan.until {
            Until::Equals(values) => values.iter().any(|expected| same(expected, &value)),
            Until::Contains(text) => value.to_lowercase().contains(&text.to_lowercase()),
            Until::Changes => *first != value,
        };
        if met { Step::Met(value) } else { Step::Wait }
    }

    pub fn told(&self, step: &Step) -> Option<String> {
        let description = &self.plan.description;
        match step {
            Step::Wait => None,
            Step::Met(value) => Some(format!(
                "{description}: the condition was met ({}). It now reads: {value}. The watch has ended.",
                self.plan.explained()
            )),
            Step::GaveUp(reason) => {
                Some(format!("{description}: the watch stopped without the condition being met, because {reason}."))
            }
        }
    }
}

fn same(expected: &str, value: &str) -> bool {
    expected.trim().to_lowercase() == value.trim().to_lowercase()
}

#[cfg(test)]
mod tests;
