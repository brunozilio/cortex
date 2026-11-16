use serde_json::{Map, Value, json};

pub const LAYOUTS: [&str; 2] = ["board", "list"];
pub const GROUPS: [&str; 3] = ["stage", "assignee", "priority"];
pub const SORTS: [&str; 4] = ["manual", "priority", "due", "updated"];
pub const CARD_FIELDS: [&str; 7] = ["project", "assignee", "priority", "due", "criteria", "pullRequest", "label"];
pub const PRIORITIES: [&str; 3] = ["high", "normal", "low"];
pub const DONE: [&str; 3] = ["all", "recent", "hidden"];
pub const MAX_ASSIGNEES: usize = 50;
pub const LABEL_MAX: usize = 60;
pub const MAX_LABELS: usize = 20;
pub const MAX_COLLAPSED: usize = 100;
pub const COLUMN_WIDTHS: [(&str, i64, i64); 8] = [
    ("title", 200, 960),
    ("project", 80, 320),
    ("label", 96, 320),
    ("priority", 72, 220),
    ("due", 80, 220),
    ("criteria", 72, 220),
    ("pullRequest", 80, 320),
    ("assignee", 120, 360),
];

pub fn default_view() -> Value {
    issues_view_of(&Value::Null)
}

fn pick(list: &[&str], value: &Value, fallback: &str) -> Value {
    json!(value.as_str().filter(|value| list.contains(value)).unwrap_or(fallback))
}

fn flag(value: &Value, fallback: bool) -> Value {
    json!(value.as_bool().unwrap_or(fallback))
}

fn strings(value: &Value) -> impl Iterator<Item = &str> {
    value.as_array().into_iter().flatten().filter_map(Value::as_str)
}

fn unique<'a>(items: impl Iterator<Item = &'a str>, limit: usize) -> Value {
    let mut kept: Vec<&str> = Vec::new();
    for item in items {
        if !kept.contains(&item) {
            kept.push(item);
        }
    }
    kept.truncate(limit);
    json!(kept)
}

fn id_part(part: &str) -> bool {
    (1..=100).contains(&part.len()) && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn assignee_allowed(assignee: &str) -> bool {
    assignee == "none"
        || assignee.strip_prefix("agent:").is_some_and(id_part)
        || assignee.strip_prefix("person:").is_some_and(id_part)
}

pub fn group_allowed(group: &str) -> bool {
    (1..=120).contains(&group.len()) && group.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '-'))
}

fn widths_of(raw: &Value) -> Value {
    let kept: Map<String, Value> = COLUMN_WIDTHS
        .iter()
        .filter_map(|(column, min, max)| {
            let width = raw.get(column)?.as_f64().filter(|width| width.is_finite())?;
            let clamped = (width.round() as i64).clamp(*min, *max);
            Some(((*column).to_owned(), json!(clamped)))
        })
        .collect();
    Value::Object(kept)
}

pub fn issues_view_of(raw: &Value) -> Value {
    let fields: Map<String, Value> =
        CARD_FIELDS.iter().map(|field| ((*field).to_owned(), flag(&raw["fields"][field], true))).collect();
    let priorities: Vec<&str> =
        PRIORITIES.iter().copied().filter(|p| strings(&raw["priorities"]).any(|given| given == *p)).collect();
    let labels = strings(&raw["labels"]).map(str::trim).filter(|l| !l.is_empty() && l.chars().count() <= LABEL_MAX);
    json!({
        "layout": pick(&LAYOUTS, &raw["layout"], "board"),
        "group": pick(&GROUPS, &raw["group"], "stage"),
        "sort": pick(&SORTS, &raw["sort"], "manual"),
        "fields": fields,
        "assignees": unique(strings(&raw["assignees"]).filter(|a| assignee_allowed(a)), MAX_ASSIGNEES),
        "priorities": priorities,
        "labels": unique(labels, MAX_LABELS),
        "done": pick(&DONE, &raw["done"], "all"),
        "showCanceled": flag(&raw["showCanceled"], false),
        "hideEmpty": flag(&raw["hideEmpty"], false),
        "collapsed": unique(strings(&raw["collapsed"]).filter(|g| group_allowed(g)), MAX_COLLAPSED),
        "widths": widths_of(&raw["widths"]),
    })
}

fn checked_enum(view: &Value, key: &str, list: &[&str]) -> Result<(), String> {
    match &view[key] {
        Value::Null => Ok(()),
        Value::String(value) if list.contains(&value.as_str()) => Ok(()),
        _ => Err(format!("view.{key} must be one of {}", list.join(", "))),
    }
}

pub fn checked_view(view: &Value) -> Result<Value, String> {
    if !view.is_object() {
        return Err("send the view as an object".into());
    }
    checked_enum(view, "layout", &LAYOUTS)?;
    checked_enum(view, "group", &GROUPS)?;
    checked_enum(view, "sort", &SORTS)?;
    checked_enum(view, "done", &DONE)?;
    for (column, width) in view["widths"].as_object().into_iter().flatten() {
        let Some((_, min, max)) = COLUMN_WIDTHS.iter().find(|(name, _, _)| name == column) else {
            return Err(format!("view.widths has no column {column}"));
        };
        let fits = width.as_f64().is_some_and(|width| width >= *min as f64 && width <= *max as f64);
        if !fits {
            return Err(format!("view.widths.{column} must be between {min} and {max}"));
        }
    }
    Ok(issues_view_of(view))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_stored_is_the_board_by_stage_with_every_field() {
        let view = default_view();
        assert_eq!(view["layout"], "board");
        assert_eq!(view["group"], "stage");
        assert_eq!(view["sort"], "manual");
        assert!(CARD_FIELDS.iter().all(|field| view["fields"][field] == true));
        assert_eq!(view["done"], "all");
        assert_eq!(view["widths"], json!({}));
        assert_eq!(view["assignees"], json!([]));
    }

    #[test]
    fn unknown_values_fall_back_and_filters_lose_repeats() {
        let raw = json!({
            "layout": "grid",
            "group": "assignee",
            "fields": { "due": false, "nope": false },
            "assignees": ["agent:a1", "agent:a1", "person:p", "robot:x", "none"],
            "priorities": ["low", "high", "urgent", "low"],
            "labels": ["  bug ", "bug", "", "x".repeat(61)],
            "collapsed": ["todo", "todo", "bad group!"],
        });
        let view = issues_view_of(&raw);
        assert_eq!(view["layout"], "board");
        assert_eq!(view["group"], "assignee");
        assert_eq!(view["fields"]["due"], false);
        assert_eq!(view["fields"]["project"], true);
        assert!(view["fields"].get("nope").is_none());
        assert_eq!(view["assignees"], json!(["agent:a1", "person:p", "none"]));
        assert_eq!(view["priorities"], json!(["high", "low"]));
        assert_eq!(view["labels"], json!(["bug"]));
        assert_eq!(view["collapsed"], json!(["todo"]));
    }

    #[test]
    fn stored_widths_are_held_between_each_columns_ends() {
        let view = issues_view_of(&json!({ "widths": { "title": 100, "due": 150.6, "nope": 10, "label": "x" } }));
        assert_eq!(view["widths"], json!({ "title": 200, "due": 151 }));
    }

    #[test]
    fn a_sent_view_with_an_unknown_choice_or_width_is_refused() {
        assert!(checked_view(&json!({ "layout": "grid" })).is_err());
        assert!(checked_view(&json!({ "widths": { "nope": 100 } })).is_err());
        assert!(checked_view(&json!({ "widths": { "title": 50 } })).is_err());
        assert!(checked_view(&json!("board")).is_err());
        let kept = checked_view(&json!({ "layout": "list", "assignees": ["none", "none"] }));
        assert_eq!(kept.map(|view| view["assignees"].clone()), Ok(json!(["none"])));
    }
}
