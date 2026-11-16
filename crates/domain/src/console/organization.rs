use serde_json::{Map, Value, json};

use super::profile::timezone_allowed;

pub const DEFAULT_NAME: &str = "Local";
pub const DEFAULT_TIMEZONE: &str = "UTC";
pub const DEFAULT_AGENT_BUDGET_NANO_USD: i64 = 50_000_000_000;
pub const NANO_PER_USD: f64 = 1e9;
pub const MODES: [&str; 2] = ["team", "personal"];
pub const APPROVALS_LEVELS: [&str; 3] = ["always", "risky", "never"];
pub const DEFAULT_APPROVALS_LEVEL: &str = "risky";
pub const MODEL_TIERS: [&str; 3] = ["small", "medium", "large"];
const NAME_MAX: usize = 100;
const CONTEXT_MAX: usize = 20_000;
const MODEL_MAX: usize = 100;
const BUDGET_MAX_USD: f64 = 1_000_000.0;

pub fn name_of(stored: &Value) -> &str {
    stored["name"].as_str().unwrap_or(DEFAULT_NAME)
}

pub fn timezone_of(stored: &Value) -> &str {
    stored["timezone"].as_str().unwrap_or(DEFAULT_TIMEZONE)
}

pub fn approvals_level_of(stored: &Value) -> &str {
    stored["approvalsLevel"]
        .as_str()
        .filter(|level| APPROVALS_LEVELS.contains(level))
        .unwrap_or(DEFAULT_APPROVALS_LEVEL)
}

pub fn budget_nano_of(stored: &Value) -> i64 {
    stored["defaultAgentBudgetNanoUsd"].as_i64().filter(|nano| *nano > 0).unwrap_or(DEFAULT_AGENT_BUDGET_NANO_USD)
}

pub fn nano_to_usd(nano: i64) -> f64 {
    nano as f64 / NANO_PER_USD
}

pub fn settings(stored: &Value) -> Value {
    json!({
        "name": name_of(stored),
        "context": stored["context"].as_str().unwrap_or(""),
        "defaultCredentialId": null,
        "defaultCredentialIds": {},
        "routerModel": stored["routerModel"].clone(),
        "summaryModel": stored["summaryModel"].clone(),
        "timezone": timezone_of(stored),
        "teamSize": null,
        "defaultAgentBudgetUsd": nano_to_usd(budget_nano_of(stored)),
        "actionCheck": stored["actionCheck"].as_bool().unwrap_or(true),
        "autoModels": auto_models_of(&stored["autoModels"]),
        "gifs": false,
        "gifRating": "g",
        "agentStickers": false,
        "agentMedia": "never",
        "voiceMessages": false,
        "mode": stored["mode"].as_str().filter(|mode| MODES.contains(mode)).unwrap_or("team"),
        "teamWatch": false,
        "welcomePeople": false,
        "approvalsLevel": approvals_level_of(stored),
        "voiceAvailable": false,
        "gifsAvailable": false,
        "agentMediaAvailable": false,
    })
}

fn auto_models_of(raw: &Value) -> Value {
    let providers: Map<String, Value> = raw
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(provider, tiers)| {
            let tiers: Map<String, Value> = MODEL_TIERS
                .iter()
                .filter_map(|tier| tiers[tier].as_str().map(|model| ((*tier).to_owned(), json!(model))))
                .collect();
            (!tiers.is_empty()).then(|| (provider.clone(), Value::Object(tiers)))
        })
        .collect();
    Value::Object(providers)
}

fn merged_auto_models(stored: &Value, change: &Value) -> Result<Value, String> {
    let changes = change.as_object().ok_or("autoModels must be an object")?;
    let mut merged = auto_models_of(stored).as_object().cloned().unwrap_or_default();
    for (provider, tiers) in changes {
        let tiers = tiers.as_object().ok_or_else(|| format!("autoModels.{provider} must be an object"))?;
        let mut next = merged.get(provider).and_then(Value::as_object).cloned().unwrap_or_default();
        for (tier, model) in tiers {
            if !MODEL_TIERS.contains(&tier.as_str()) {
                return Err(format!("autoModels has no tier {tier}"));
            }
            match model {
                Value::Null => {
                    next.remove(tier);
                }
                Value::String(model) if !model.is_empty() && model.len() <= MODEL_MAX => {
                    next.insert(tier.clone(), json!(model));
                }
                _ => return Err(format!("autoModels.{provider}.{tier} must be a model")),
            }
        }
        if next.is_empty() {
            merged.remove(provider);
        } else {
            merged.insert(provider.clone(), Value::Object(next));
        }
    }
    Ok(Value::Object(merged))
}

fn text(input: &Value, key: &str, max: usize) -> Result<Option<String>, String> {
    match &input[key] {
        Value::Null => Ok(None),
        Value::String(text) if text.chars().count() <= max => Ok(Some(text.clone())),
        _ => Err(format!("{key} must be text up to {max} characters")),
    }
}

fn model(input: &Value, key: &str) -> Result<Option<Value>, String> {
    match input.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(Value::Null)),
        Some(Value::String(model)) if model.len() <= MODEL_MAX => Ok(Some(json!(model))),
        Some(_) => Err(format!("{key} must be a model or null")),
    }
}

pub fn with_settings_change(stored: &Value, input: &Value) -> Result<Value, String> {
    let mut next = stored.as_object().cloned().unwrap_or_default();
    if let Some(name) = text(input, "name", NAME_MAX)? {
        let name = name.trim();
        if name.is_empty() {
            return Err("name can't be empty".into());
        }
        next.insert("name".into(), json!(name));
    }
    if let Some(context) = text(input, "context", CONTEXT_MAX)? {
        next.insert("context".into(), json!(context));
    }
    for key in ["routerModel", "summaryModel"] {
        if let Some(model) = model(input, key)? {
            next.insert(key.into(), model);
        }
    }
    if let Some(zone) = text(input, "timezone", 64)? {
        if !timezone_allowed(&zone) {
            return Err("timezone must be an IANA time zone".into());
        }
        next.insert("timezone".into(), json!(zone));
    }
    if let Some(mode) = text(input, "mode", 16)? {
        if !MODES.contains(&mode.as_str()) {
            return Err(format!("mode must be one of {}", MODES.join(", ")));
        }
        next.insert("mode".into(), json!(mode));
    }
    match &input["actionCheck"] {
        Value::Null => {}
        Value::Bool(on) => {
            next.insert("actionCheck".into(), json!(on));
        }
        _ => return Err("actionCheck must be true or false".into()),
    }
    if !input["defaultAgentBudgetUsd"].is_null() {
        let usd = input["defaultAgentBudgetUsd"].as_f64().filter(|usd| *usd > 0.0 && *usd <= BUDGET_MAX_USD);
        let usd = usd.ok_or("the default agent budget must be above zero and at most 1000000")?;
        next.insert("defaultAgentBudgetNanoUsd".into(), json!((usd * NANO_PER_USD).round() as i64));
    }
    if !input["autoModels"].is_null() {
        let merged = merged_auto_models(&next.get("autoModels").cloned().unwrap_or(Value::Null), &input["autoModels"])?;
        next.insert("autoModels".into(), merged);
    }
    Ok(Value::Object(next))
}

pub fn checked_approvals_level(input: &Value) -> Result<&str, String> {
    if !input["everyAgent"].is_null() && !input["everyAgent"].is_boolean() {
        return Err("everyAgent must be true or false".into());
    }
    input["level"]
        .as_str()
        .filter(|level| APPROVALS_LEVELS.contains(level))
        .ok_or_else(|| format!("level must be one of {}", APPROVALS_LEVELS.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_workspace_has_the_original_defaults() {
        let settings = settings(&Value::Null);
        assert_eq!(settings["name"], DEFAULT_NAME);
        assert_eq!(settings["timezone"], "UTC");
        assert_eq!(settings["defaultAgentBudgetUsd"], 50.0);
        assert_eq!(settings["actionCheck"], true);
        assert_eq!(settings["mode"], "team");
        assert_eq!(settings["approvalsLevel"], "risky");
        assert_eq!(settings["routerModel"], Value::Null);
        assert_eq!(settings["autoModels"], json!({}));
        for media in ["gifsAvailable", "voiceAvailable", "agentMediaAvailable"] {
            assert_eq!(settings[media], false, "{media}");
        }
    }

    #[test]
    fn a_change_keeps_what_was_not_sent() {
        let stored = json!({ "name": "Casa", "context": "we sell bread" });
        let next =
            with_settings_change(&stored, &json!({ "timezone": "America/Sao_Paulo", "defaultAgentBudgetUsd": 12.5 }));
        let shown = settings(&next.unwrap_or_default());
        assert_eq!(shown["name"], "Casa");
        assert_eq!(shown["context"], "we sell bread");
        assert_eq!(shown["timezone"], "America/Sao_Paulo");
        assert_eq!(shown["defaultAgentBudgetUsd"], 12.5);
    }

    #[test]
    fn bad_settings_are_refused() {
        for input in [
            json!({ "name": "   " }),
            json!({ "timezone": "nowhere at all" }),
            json!({ "mode": "family" }),
            json!({ "defaultAgentBudgetUsd": 0 }),
            json!({ "actionCheck": "yes" }),
            json!({ "autoModels": { "openai": { "huge": "gpt" } } }),
        ] {
            assert!(with_settings_change(&Value::Null, &input).is_err(), "{input}");
        }
    }

    #[test]
    fn auto_models_merge_per_tier_and_null_clears_one() {
        let stored = json!({ "autoModels": { "openai": { "small": "a", "large": "b" } } });
        let next = with_settings_change(
            &stored,
            &json!({ "autoModels": { "openai": { "small": null }, "zai": { "medium": "c" } } }),
        );
        assert_eq!(
            settings(&next.unwrap_or_default())["autoModels"],
            json!({ "openai": { "large": "b" }, "zai": { "medium": "c" } })
        );
        let cleared =
            with_settings_change(&stored, &json!({ "autoModels": { "openai": { "small": null, "large": null } } }));
        assert_eq!(settings(&cleared.unwrap_or_default())["autoModels"], json!({}));
    }

    #[test]
    fn a_model_can_go_back_to_the_default_with_null() {
        let stored = json!({ "routerModel": "m1" });
        let next = with_settings_change(&stored, &json!({ "routerModel": null }));
        assert_eq!(settings(&next.unwrap_or_default())["routerModel"], Value::Null);
    }

    #[test]
    fn the_approvals_level_is_one_of_three() {
        assert_eq!(checked_approvals_level(&json!({ "level": "never", "everyAgent": true })), Ok("never"));
        assert!(checked_approvals_level(&json!({ "level": "sometimes" })).is_err());
        assert!(checked_approvals_level(&json!({ "level": "never", "everyAgent": 1 })).is_err());
    }
}
