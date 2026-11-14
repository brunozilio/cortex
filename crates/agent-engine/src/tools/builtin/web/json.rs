use serde_json::{Map, Value};

pub fn compact(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body.trim()).ok()?;
    let (items, meta) = records(&value);
    let Some(items) = items else { return Some(without_nulls(value).to_string()) };
    let rows: Vec<Vec<(String, String)>> = items.iter().map(flatten).collect();
    let shared: Vec<(String, String)> = match rows.as_slice() {
        [first, _, ..] => first.iter().filter(|field| rows.iter().all(|row| row.contains(field))).cloned().collect(),
        _ => Vec::new(),
    };
    let mut lines: Vec<String> = meta.iter().map(|(key, value)| format!("{key}: {value}")).collect();
    if !shared.is_empty() {
        lines.push(format!("every item: {}", pairs(&shared)));
    }
    for (at, row) in rows.iter().enumerate() {
        let own: Vec<(String, String)> = row.iter().filter(|field| !shared.contains(field)).cloned().collect();
        lines.push(format!("[{}] {}", at + 1, pairs(&own)));
    }
    Some(lines.join("\n"))
}

fn records(value: &Value) -> (Option<&Vec<Value>>, Vec<(String, String)>) {
    let objects = |list: &Vec<Value>| list.len() > 1 && list.iter().all(Value::is_object);
    match value {
        Value::Array(list) if objects(list) => (Some(list), Vec::new()),
        Value::Object(fields) => {
            let main = fields
                .iter()
                .filter_map(|(key, value)| value.as_array().filter(|list| objects(list)).map(|list| (key, list)))
                .max_by_key(|(_, list)| list.len());
            let Some((main_key, list)) = main else { return (None, Vec::new()) };
            let meta = fields
                .iter()
                .filter(|(key, _)| *key != main_key)
                .filter_map(|(key, value)| scalar(value).map(|text| (key.clone(), text)))
                .collect();
            (Some(list), meta)
        }
        _ => (None, Vec::new()),
    }
}

fn flatten(item: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(fields) = item.as_object() else { return out };
    for (key, value) in fields {
        match value {
            Value::Object(inner) => out.extend(nested(key, inner)),
            _ => out.extend(scalar(value).map(|text| (key.clone(), text))),
        }
    }
    out
}

fn nested(key: &str, inner: &Map<String, Value>) -> Vec<(String, String)> {
    inner.iter().filter_map(|(name, value)| scalar(value).map(|text| (format!("{key}.{name}"), text))).collect()
}

fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) if text.trim().is_empty() => None,
        Value::String(text) => Some(text.clone()),
        Value::Bool(_) | Value::Number(_) => Some(value.to_string()),
        Value::Array(list) if list.is_empty() => None,
        Value::Array(list) if list.iter().all(|item| !item.is_object() && !item.is_array()) => {
            Some(list.iter().filter_map(scalar).collect::<Vec<_>>().join(", "))
        }
        Value::Array(list) => Some(format!("[{} items]", list.len())),
        Value::Object(_) => None,
    }
}

fn pairs(fields: &[(String, String)]) -> String {
    fields.iter().map(|(key, value)| format!("{key}={value}")).collect::<Vec<_>>().join("; ")
}

fn without_nulls(value: Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields.into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k, without_nulls(v))).collect(),
        ),
        Value::Array(list) => Value::Array(list.into_iter().map(without_nulls).collect()),
        other => other,
    }
}
