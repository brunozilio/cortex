use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use jsonschema::error::{TypeKind, ValidationErrorKind};
use serde_json::{Map, Value};

const SHOWN_AT_MOST: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub value: Value,
    pub problems: Vec<String>,
}

impl Checked {
    pub fn located(&self, file: &Path) -> Vec<String> {
        self.problems.iter().map(|problem| format!("{}: {problem}", file.display())).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Step {
    Key(String),
    Index(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Whole {
    ListItem,
    Entry { under: &'static str },
}

impl Whole {
    fn around(self, mut at: Vec<Step>) -> Vec<Step> {
        if let Self::Entry { under } = self
            && at.len() >= 2
            && at[0] == Step::Key(under.to_owned())
        {
            at.truncate(2);
            return at;
        }
        if let Some(item) = at.iter().rposition(|step| matches!(step, Step::Index(_))) {
            at.truncate(item + 1);
        }
        at
    }
}

pub(super) fn checked(validator: Option<&jsonschema::Validator>, whole: Whole, value: Value) -> Checked {
    let Some(validator) = validator else { return Checked { value, problems: Vec::new() } };
    let mut problems = Vec::new();
    let mut doomed = Vec::new();
    for error in validator.iter_errors(&value) {
        let at = steps(&value, error.instance_path.as_str());
        let found: Vec<(Vec<Step>, Vec<Step>, String)> = match &error.kind {
            ValidationErrorKind::AdditionalProperties { unexpected } => unexpected
                .iter()
                .map(|key| within(&at, Step::Key(key.clone())))
                .map(|key| (key.clone(), key, "unknown key, ignored".to_owned()))
                .collect(),
            kind => {
                let what = line_about(kind, &error.instance, &error.to_string());
                vec![(at.clone(), whole.around(at), what)]
            }
        };
        for (shown, left_out, what) in found {
            let problem = line(&shown, &what);
            if !problems.contains(&problem) {
                problems.push(problem);
            }
            doomed.push(left_out);
        }
    }
    Checked { value: pruned(value, doomed), problems }
}

pub fn logged_once(problems: &[String]) -> usize {
    static TOLD: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let Ok(mut told) = TOLD.get_or_init(Mutex::default).lock() else { return 0 };
    let fresh: Vec<&String> = problems.iter().filter(|problem| told.insert((*problem).clone())).collect();
    for problem in &fresh {
        tracing::warn!(%problem, "a configuration value is left out");
    }
    fresh.len()
}

fn within(at: &[Step], step: Step) -> Vec<Step> {
    let mut path = at.to_vec();
    path.push(step);
    path
}

fn steps(value: &Value, pointer: &str) -> Vec<Step> {
    let mut node = Some(value);
    let mut steps = Vec::new();
    for raw in pointer.split('/').skip(1) {
        let segment = raw.replace("~1", "/").replace("~0", "~");
        let index = segment.parse::<usize>().ok().filter(|_| node.is_some_and(Value::is_array));
        node = match index {
            Some(index) => node.and_then(|items| items.get(index)),
            None => node.and_then(|fields| fields.get(&segment)),
        };
        steps.push(index.map_or(Step::Key(segment), Step::Index));
    }
    steps
}

fn line(at: &[Step], what: &str) -> String {
    if at.is_empty() {
        return what.to_owned();
    }
    let mut shown = String::new();
    for step in at {
        match step {
            Step::Key(key) if shown.is_empty() => shown.push_str(key),
            Step::Key(key) => {
                shown.push('.');
                shown.push_str(key);
            }
            Step::Index(index) => shown.push_str(&format!("[{index}]")),
        }
    }
    format!("{shown}: {what}")
}

fn line_about(kind: &ValidationErrorKind, instance: &Value, otherwise: &str) -> String {
    let found = briefly(instance);
    match kind {
        ValidationErrorKind::Type { kind: TypeKind::Single(expected) } => format!("expected {expected}, found {found}"),
        ValidationErrorKind::Type { kind: TypeKind::Multiple(expected) } => {
            let expected: Vec<String> = expected.iter().map(|kind| kind.to_string()).collect();
            format!("expected {}, found {found}", expected.join(" or "))
        }
        ValidationErrorKind::Enum { options } => {
            let options = options.as_array().map(|options| options.iter().map(Value::to_string).collect::<Vec<_>>());
            format!("expected one of {}, found {found}", options.unwrap_or_default().join(", "))
        }
        ValidationErrorKind::Minimum { limit } => format!("expected at least {limit}, found {found}"),
        ValidationErrorKind::Maximum { limit } => format!("expected at most {limit}, found {found}"),
        ValidationErrorKind::Required { property } => format!("expected the key {property}"),
        _ => otherwise.to_owned(),
    }
}

fn briefly(value: &Value) -> String {
    let text = value.to_string();
    if text.chars().count() <= SHOWN_AT_MOST {
        return text;
    }
    let mut cut: String = text.chars().take(SHOWN_AT_MOST).collect();
    cut.push('…');
    cut
}

fn pruned(mut value: Value, mut doomed: Vec<Vec<Step>>) -> Value {
    doomed.sort();
    doomed.dedup();
    for path in doomed.iter().rev() {
        remove(&mut value, path);
    }
    value
}

fn remove(value: &mut Value, path: &[Step]) {
    let Some((last, parents)) = path.split_last() else {
        *value = Value::Object(Map::new());
        return;
    };
    let mut node = Some(value);
    for step in parents {
        node = match step {
            Step::Key(key) => node.and_then(|fields| fields.get_mut(key)),
            Step::Index(index) => node.and_then(|items| items.get_mut(*index)),
        };
    }
    match (node, last) {
        (Some(Value::Object(fields)), Step::Key(key)) => {
            fields.remove(key);
        }
        (Some(Value::Array(items)), Step::Index(index)) if *index < items.len() => {
            items.remove(*index);
        }
        _ => {}
    }
}
