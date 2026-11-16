use serde_json::{Map, Value, json};

pub const ITEMS: [&str; 9] =
    ["waiting", "approvals", "proposals", "threads", "issues", "wiki", "drive", "dashboards", "taskManager"];
pub const PLACES: [&str; 3] = ["always", "more", "auto"];
pub const COUNTED: [&str; 4] = ["waiting", "approvals", "proposals", "threads"];
pub const SECTIONS: [&str; 3] = ["tools", "channels", "agents"];
pub const AGENT_SORTS: [&str; 3] = ["workspace", "mine", "working"];
pub const PIN_KINDS: [&str; 2] = ["agents", "channels"];
pub const PIN_LIMIT: usize = 5;

pub fn place_allowed(item: &str, place: &str) -> bool {
    ITEMS.contains(&item) && PLACES.contains(&place) && (place != "auto" || COUNTED.contains(&item))
}

pub fn items_of(raw: &Value) -> Value {
    let kept: Map<String, Value> = raw
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(item, place)| place.as_str().is_some_and(|place| place_allowed(item, place)))
        .map(|(item, place)| (item.clone(), place.clone()))
        .collect();
    Value::Object(kept)
}

pub fn with_item(items: &Value, item: &str, place: Option<&str>) -> Result<Value, String> {
    if let Some(place) = place
        && !place_allowed(item, place)
    {
        return Err(format!("{item} can't go to {place}"));
    }
    if place.is_none() && !ITEMS.contains(&item) {
        return Err(format!("{item} is not a sidebar tool"));
    }
    let mut items = items_of(items);
    if let Some(map) = items.as_object_mut() {
        match place {
            Some(place) => map.insert(item.to_owned(), json!(place)),
            None => map.remove(item),
        };
    }
    Ok(items)
}

pub fn order_of(raw: &Value) -> Value {
    let listed: Vec<&str> = raw.as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let valid =
        listed.len() == SECTIONS.len() && SECTIONS.iter().all(|s| listed.iter().filter(|x| *x == s).count() == 1);
    if valid { json!(listed) } else { json!(SECTIONS) }
}

pub fn checked_order(order: &Value) -> Result<Value, String> {
    if order.is_null() || order_of(order) == *order {
        return Ok(order_of(order));
    }
    Err("the order must list every section once".into())
}

pub fn tool_order_of(raw: &Value) -> Value {
    let stored = raw.as_array().into_iter().flatten().filter_map(Value::as_str).filter(|x| ITEMS.contains(x));
    let mut order: Vec<&str> = Vec::new();
    for item in stored.chain(ITEMS) {
        if !order.contains(&item) {
            order.push(item);
        }
    }
    json!(order)
}

pub fn view_of(raw: &Value) -> Value {
    let sort = raw["agentSort"].as_str().filter(|sort| AGENT_SORTS.contains(sort)).unwrap_or("workspace");
    let pins = raw["pins"].as_object().map(|pins| {
        let kinds: Map<String, Value> =
            PIN_KINDS.iter().map(|kind| ((*kind).to_owned(), pin_ids(&pins[*kind]))).collect();
        Value::Object(kinds)
    });
    json!({ "agentSort": sort, "showAllAgents": raw["showAllAgents"] == true, "pins": pins })
}

fn pin_ids(raw: &Value) -> Value {
    let mut ids: Vec<&str> = Vec::new();
    for id in raw.as_array().into_iter().flatten().filter_map(Value::as_str) {
        if !id.is_empty() && id.len() <= 64 && !ids.contains(&id) && ids.len() < PIN_LIMIT {
            ids.push(id);
        }
    }
    json!(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_counted_tools_may_show_automatically() {
        assert!(place_allowed("approvals", "auto"));
        assert!(!place_allowed("wiki", "auto"));
        assert!(place_allowed("wiki", "more"));
        assert!(!place_allowed("calendar", "always"), "calendar is not part of the console");
    }

    #[test]
    fn stored_places_keep_only_known_tools_in_allowed_places() {
        let raw = json!({ "wiki": "more", "issues": "auto", "nope": "always", "drive": 3 });
        assert_eq!(items_of(&raw), json!({ "wiki": "more" }));
        assert_eq!(items_of(&Value::Null), json!({}));
    }

    #[test]
    fn moving_one_tool_keeps_the_others_and_null_puts_it_back() {
        let items = json!({ "wiki": "more", "drive": "always" });
        assert_eq!(
            with_item(&items, "issues", Some("more")),
            Ok(json!({ "wiki": "more", "drive": "always", "issues": "more" }))
        );
        assert_eq!(with_item(&items, "wiki", None), Ok(json!({ "drive": "always" })));
        assert!(with_item(&items, "wiki", Some("auto")).is_err());
    }

    #[test]
    fn an_order_missing_a_section_falls_back_to_the_default() {
        assert_eq!(order_of(&json!(["agents", "tools", "channels"])), json!(["agents", "tools", "channels"]));
        assert_eq!(order_of(&json!(["agents", "agents", "channels"])), json!(["tools", "channels", "agents"]));
        assert_eq!(order_of(&Value::Null), json!(["tools", "channels", "agents"]));
        assert!(checked_order(&json!(["agents"])).is_err());
        assert_eq!(checked_order(&Value::Null), Ok(json!(["tools", "channels", "agents"])));
    }

    #[test]
    fn the_tool_order_puts_stored_tools_first_then_every_other_once() {
        let order = tool_order_of(&json!(["wiki", "nope", "wiki", "drive"]));
        assert_eq!(
            order,
            json!([
                "wiki",
                "drive",
                "waiting",
                "approvals",
                "proposals",
                "threads",
                "issues",
                "dashboards",
                "taskManager"
            ])
        );
    }

    #[test]
    fn the_view_defaults_and_trims_pins_to_five_distinct_ids() {
        assert_eq!(view_of(&Value::Null), json!({ "agentSort": "workspace", "showAllAgents": false, "pins": null }));
        let raw = json!({ "agentSort": "mine", "showAllAgents": true,
            "pins": { "agents": ["a", "a", "b", "c", "d", "e", "f"], "channels": "x", "people": ["p"] } });
        assert_eq!(
            view_of(&raw),
            json!({ "agentSort": "mine", "showAllAgents": true, "pins": { "agents": ["a", "b", "c", "d", "e"], "channels": [] } })
        );
    }
}
