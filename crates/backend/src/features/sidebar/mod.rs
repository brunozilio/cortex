use domain::console::sidebar;
use serde_json::{Value, json};
use store::Database;

use crate::feature::Context;

const KEY: &str = "sidebar";

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    let database = context.services.database.as_ref();
    let stored = database.preference(KEY)?.unwrap_or_else(|| json!({}));
    let op = input["op"].as_str().ok_or("the sidebar feature needs an op")?;
    match op {
        "items" => Ok(json!({ "items": sidebar::items_of(&stored["items"]) })),
        "order" => Ok(json!({ "order": sidebar::order_of(&stored["order"]) })),
        "tools" => Ok(json!({ "order": sidebar::tool_order_of(&stored["tools"]) })),
        "view" => Ok(json!({ "view": sidebar::view_of(&stored["view"]) })),
        "set_item" => {
            let item = input["item"].as_str().ok_or("say which tool")?;
            let items = sidebar::with_item(&stored["items"], item, input["place"].as_str())?;
            keep(database, stored, "items", items.clone())?;
            Ok(json!({ "items": items }))
        }
        "set_items" => {
            let items = sidebar::items_of(&input["items"]);
            keep(database, stored, "items", items.clone())?;
            Ok(json!({ "items": items }))
        }
        "set_order" => {
            let order = sidebar::checked_order(&input["order"])?;
            keep(database, stored, "order", input["order"].clone())?;
            Ok(json!({ "order": order }))
        }
        "set_tools" => {
            let order = sidebar::tool_order_of(&input["order"]);
            let kept = if input["order"].is_null() { Value::Null } else { order.clone() };
            keep(database, stored, "tools", kept)?;
            Ok(json!({ "order": order }))
        }
        "set_view" => {
            let view = sidebar::view_of(&input["view"]);
            keep(database, stored, "view", view.clone())?;
            Ok(json!({ "view": view }))
        }
        other => Err(format!("the sidebar feature has no op {other}")),
    }
}

fn keep(database: &dyn Database, mut stored: Value, part: &str, value: Value) -> Result<(), String> {
    if let Some(map) = stored.as_object_mut() {
        if value.is_null() {
            map.remove(part);
        } else {
            map.insert(part.to_owned(), value);
        }
    }
    database.keep_preference(KEY, &stored)
}

pub fn forget(_client: u64) {}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::feature::testing;

    fn sidebar_with(database: Arc<store::Memory>) -> impl Fn(Value) -> Result<Value, String> {
        move |input| {
            let mut services = testing::services(None);
            services.database = database.clone();
            testing::with(1, &services, |context| handle(context, input))
        }
    }

    #[test]
    fn a_new_sidebar_shows_the_defaults() -> Result<(), String> {
        let call = sidebar_with(Arc::default());
        assert_eq!(call(json!({ "op": "items" })), Ok(json!({ "items": {} })));
        assert_eq!(call(json!({ "op": "order" })), Ok(json!({ "order": ["tools", "channels", "agents"] })));
        assert_eq!(call(json!({ "op": "view" }))?["view"]["agentSort"], "workspace");
        Ok(())
    }

    #[test]
    fn each_choice_is_kept_without_touching_the_others() -> Result<(), String> {
        let call = sidebar_with(Arc::default());
        call(json!({ "op": "set_item", "item": "wiki", "place": "more" })).expect("item");
        call(json!({ "op": "set_order", "order": ["agents", "channels", "tools"] })).expect("order");
        call(json!({ "op": "set_item", "item": "drive", "place": "always" })).expect("item");

        assert_eq!(call(json!({ "op": "items" })), Ok(json!({ "items": { "wiki": "more", "drive": "always" } })));
        assert_eq!(call(json!({ "op": "order" })), Ok(json!({ "order": ["agents", "channels", "tools"] })));

        call(json!({ "op": "set_order", "order": null })).expect("reset");
        assert_eq!(call(json!({ "op": "order" })), Ok(json!({ "order": ["tools", "channels", "agents"] })));
        assert_eq!(call(json!({ "op": "items" }))?["items"]["wiki"], "more");
        Ok(())
    }

    #[test]
    fn a_bad_choice_is_refused_and_nothing_changes() {
        let call = sidebar_with(Arc::default());
        assert!(call(json!({ "op": "set_item", "item": "wiki", "place": "auto" })).is_err());
        assert!(call(json!({ "op": "set_order", "order": ["tools"] })).is_err());
        assert_eq!(call(json!({ "op": "items" })), Ok(json!({ "items": {} })));
    }
}
