use domain::console::{image, organization, usage};
use serde_json::{Value, json};

use crate::feature::Context;

const KEY: &str = "organization";
const LOGO: &str = "organization.logo";

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    let database = context.services.database.as_ref();
    let op = input["op"].as_str().ok_or("the organization feature needs an op")?;
    let stored = database.preference(KEY)?.unwrap_or(Value::Null);
    match op {
        "settings" => Ok(organization::settings(&stored)),
        "update_settings" => {
            let next = organization::with_settings_change(&stored, &input)?;
            database.keep_preference(KEY, &next)?;
            Ok(organization::settings(&next))
        }
        "set_approvals_level" => {
            let level = organization::checked_approvals_level(&input)?.to_owned();
            let mut next = stored.as_object().cloned().unwrap_or_default();
            next.insert("approvalsLevel".into(), json!(level));
            database.keep_preference(KEY, &Value::Object(next))?;
            Ok(json!({ "level": level, "ownAgents": 0 }))
        }
        "overview" => Ok(usage::overview(&stored)),
        "limits" => Ok(usage::limits()),
        "logo" => Ok(json!({ "logo": database.preference(LOGO)?.unwrap_or(Value::Null) })),
        "set_logo" => {
            let base64 = input["imageBase64"].as_str().unwrap_or_default();
            let logo = image::data_url(base64, input["contentType"].as_str().unwrap_or_default())?;
            database.keep_preference(LOGO, &json!(logo))?;
            Ok(json!({ "logo": logo }))
        }
        "remove_logo" => {
            database.keep_preference(LOGO, &Value::Null)?;
            Ok(Value::Null)
        }
        other => Err(format!("the organization feature has no op {other}")),
    }
}

pub fn forget(_client: u64) {}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    use super::*;
    use crate::feature::testing;

    fn organization_with(database: Arc<store::Memory>) -> impl Fn(Value) -> Result<Value, String> {
        move |input| {
            let mut services = testing::services(None);
            services.database = database.clone();
            testing::with(1, &services, |context| handle(context, input))
        }
    }

    #[test]
    fn settings_change_and_reach_the_overview() -> Result<(), String> {
        let call = organization_with(Arc::default());
        assert_eq!(call(json!({ "op": "settings" }))?["name"], "Local");
        let changed = call(json!({ "op": "update_settings", "name": " Casa ", "mode": "personal", "gifs": true }))?;
        assert_eq!((changed["name"].clone(), changed["mode"].clone()), (json!("Casa"), json!("personal")));
        assert_eq!(changed["gifs"], false);
        assert_eq!(call(json!({ "op": "overview" }))?["name"], "Casa");
        assert_eq!(call(json!({ "op": "settings" }))?, changed);
        Ok(())
    }

    #[test]
    fn the_approvals_level_is_kept_in_the_settings() -> Result<(), String> {
        let call = organization_with(Arc::default());
        let set = call(json!({ "op": "set_approvals_level", "level": "always", "everyAgent": true }));
        assert_eq!(set, Ok(json!({ "level": "always", "ownAgents": 0 })));
        assert_eq!(call(json!({ "op": "settings" }))?["approvalsLevel"], "always");
        assert!(call(json!({ "op": "set_approvals_level", "level": "maybe" })).is_err());
        Ok(())
    }

    #[test]
    fn a_refused_change_keeps_the_settings() -> Result<(), String> {
        let call = organization_with(Arc::default());
        assert!(call(json!({ "op": "update_settings", "name": "Casa", "timezone": "not a zone" })).is_err());
        assert_eq!(call(json!({ "op": "settings" }))?["name"], "Local");
        Ok(())
    }

    #[test]
    fn the_logo_is_kept_as_a_data_url_until_removed() -> Result<(), String> {
        let call = organization_with(Arc::default());
        let jpeg = STANDARD.encode([0xff, 0xd8, 0xff, 0xe0, 0, 0x10]);
        let set = call(json!({ "op": "set_logo", "imageBase64": jpeg, "contentType": "image/jpeg" }))?;
        assert_eq!(set, json!({ "logo": format!("data:image/jpeg;base64,{jpeg}") }));
        assert_eq!(call(json!({ "op": "logo" }))?, set);
        assert_eq!(call(json!({ "op": "remove_logo" })), Ok(Value::Null));
        assert_eq!(call(json!({ "op": "logo" })), Ok(json!({ "logo": null })));
        assert!(call(json!({ "op": "set_logo", "imageBase64": jpeg, "contentType": "image/png" })).is_err());
        Ok(())
    }

    #[test]
    fn limits_are_the_owner_alone_with_no_caps() -> Result<(), String> {
        let call = organization_with(Arc::default());
        assert_eq!(call(json!({ "op": "limits" }))?, usage::limits());
        Ok(())
    }
}
