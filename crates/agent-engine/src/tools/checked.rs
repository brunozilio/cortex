use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use serde_json::Value;

const MOST_PROBLEMS: usize = 5;
const MOST_KEPT: usize = 512;

enum Compiled {
    Checks(jsonschema::Validator),
    Unreadable,
}

static COMPILED: LazyLock<Mutex<HashMap<String, Arc<Compiled>>>> = LazyLock::new(Mutex::default);

fn compiled(schema: &Value) -> Arc<Compiled> {
    let key = schema.to_string();
    if let Some(found) = COMPILED.lock().ok().and_then(|kept| kept.get(&key).cloned()) {
        return found;
    }
    let made = Arc::new(match jsonschema::validator_for(schema) {
        Ok(validator) => Compiled::Checks(validator),
        Err(error) => {
            tracing::warn!(%error, "a tool's input schema cannot be compiled; its input is not checked");
            Compiled::Unreadable
        }
    });
    if let Ok(mut kept) = COMPILED.lock() {
        if kept.len() >= MOST_KEPT {
            kept.clear();
        }
        kept.insert(key, Arc::clone(&made));
    }
    made
}

fn without_optional_nulls(schema: &Value, input: &Value) -> Option<Value> {
    let fields = input.as_object()?;
    let required: Vec<&str> =
        schema["required"].as_array().map(|names| names.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let declared = |key: &String| schema["properties"].get(key).is_some();
    let left_out = |key: &String, value: &Value| value.is_null() && declared(key) && !required.contains(&key.as_str());
    if !fields.iter().any(|(key, value)| left_out(key, value)) {
        return None;
    }
    let kept = fields.iter().filter(|(key, value)| !left_out(key, value));
    Some(Value::Object(kept.map(|(key, value)| (key.clone(), value.clone())).collect()))
}

pub fn refusal(name: &str, schema: &Value, input: &Value) -> Option<String> {
    let compiled = compiled(schema);
    let Compiled::Checks(validator) = compiled.as_ref() else { return None };
    let trimmed = without_optional_nulls(schema, input);
    let problems: Vec<String> = validator
        .iter_errors(trimmed.as_ref().unwrap_or(input))
        .take(MOST_PROBLEMS)
        .map(|error| {
            let at = error.instance_path.as_str();
            if at.is_empty() { error.to_string() } else { format!("{at}: {error}") }
        })
        .collect();
    if problems.is_empty() {
        return None;
    }
    Some(format!(
        "Invalid input for {name}, so it did not run: {}. Check the tool's parameters and call it again.",
        problems.join("; ")
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": { "path": { "type": "string" }, "items": { "type": "array", "items": { "type": "integer" }, "maxItems": 2 } },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    #[test]
    fn a_valid_input_passes() {
        assert_eq!(refusal("T", &schema(), &json!({ "path": "a", "items": [1, 2] })), None);
    }

    #[test]
    fn each_problem_names_the_field_it_is_about() {
        let missing = refusal("T", &schema(), &json!({})).expect("refused");
        assert!(missing.starts_with("Invalid input for T") && missing.contains("\"path\""), "{missing}");
        let extra = refusal("T", &schema(), &json!({ "path": "a", "other": 1 })).expect("refused");
        assert!(extra.contains("'other'"), "{extra}");
        let nested = refusal("T", &schema(), &json!({ "path": "a", "items": [1, "x"] })).expect("refused");
        assert!(nested.contains("/items/1"), "{nested}");
        let long = refusal("T", &schema(), &json!({ "path": "a", "items": [1, 2, 3] })).expect("refused");
        assert!(long.contains("/items"), "{long}");
        assert!(refusal("T", &schema(), &json!("text")).is_some(), "not an object");
    }

    #[test]
    fn a_null_optional_field_counts_as_left_out_and_a_null_required_one_does_not() {
        assert_eq!(refusal("T", &schema(), &json!({ "path": "a", "items": null })), None);
        assert!(refusal("T", &schema(), &json!({ "path": null })).is_some());
        assert!(refusal("T", &schema(), &json!({ "path": "a", "other": null })).is_some(), "still not allowed");
    }

    #[test]
    fn a_schema_that_does_not_compile_checks_nothing() {
        assert_eq!(refusal("T", &json!({ "type": "nonsense" }), &json!({})), None);
    }

    #[test]
    fn a_schema_is_compiled_once() {
        let schema = json!({ "type": "object", "title": "compiled once" });
        let first = compiled(&schema);
        let again = compiled(&schema);
        assert!(Arc::ptr_eq(&first, &again));
    }
}
