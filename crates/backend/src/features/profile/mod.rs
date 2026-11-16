use domain::console::{image, issues_view, organization, profile};
use serde_json::{Value, json};
use store::Database;

use crate::feature::Context;

const KEY: &str = "profile";
const AVATAR: &str = "profile.avatar";
const ORGANIZATION: &str = "organization";

fn stored(database: &dyn Database, key: &str) -> Result<Value, String> {
    Ok(database.preference(key)?.unwrap_or(Value::Null))
}

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    let database = context.services.database.as_ref();
    let op = input["op"].as_str().ok_or("the profile feature needs an op")?;
    let profile = stored(database, KEY)?;
    let org_timezone = || stored(database, ORGANIZATION).map(|org| organization::timezone_of(&org).to_owned());
    match op {
        "prefs" => Ok(profile::prefs(&profile, &stored(database, AVATAR)?, &org_timezone()?)),
        "update_me" => {
            let changed = profile::with_profile_change(&profile, &input)?;
            database.keep_preference(KEY, &changed)?;
            Ok(profile::person(brand::NAME, &changed, &stored(database, AVATAR)?, &org_timezone()?))
        }
        "set_start" => {
            let start = profile::checked_start(&input)?;
            keep(database, profile, "start", start.clone())?;
            Ok(start)
        }
        "set_activity" => {
            let show = input["show"].as_bool().ok_or("show must be true or false")?;
            keep(database, profile, "showActivity", json!(show))?;
            Ok(json!({ "show": show }))
        }
        "set_agent_style" => {
            let style = profile::checked_agent_style(&input["style"])?;
            keep(database, profile, "agentStyle", style.clone())?;
            Ok(json!({ "style": style }))
        }
        "set_chat" => {
            let prefs = profile::merged_chat_prefs(&profile["chatPrefs"], &input)?;
            keep(database, profile, "chatPrefs", prefs.clone())?;
            Ok(prefs)
        }
        "issues_view" => Ok(json!({ "view": issues_view::issues_view_of(&profile["issuesView"]) })),
        "set_issues_view" => {
            let view = issues_view::checked_view(&input["view"])?;
            keep(database, profile, "issuesView", view.clone())?;
            Ok(json!({ "view": view }))
        }
        "set_avatar" => set_avatar(database, &input),
        "remove_avatar" => {
            database.keep_preference(AVATAR, &Value::Null)?;
            Ok(Value::Null)
        }
        other => Err(format!("the profile feature has no op {other}")),
    }
}

fn set_avatar(database: &dyn Database, input: &Value) -> Result<Value, String> {
    let portrait = match &input["portrait"] {
        Value::Null => None,
        Value::String(code) if image::portrait_allowed(code) => Some(code.as_str()),
        _ => return Err("That isn't a portrait's recipe".into()),
    };
    let base64 = input["imageBase64"].as_str().unwrap_or_default();
    let url = image::data_url(base64, input["contentType"].as_str().unwrap_or_default())?;
    let url = portrait.map_or_else(|| url.clone(), |code| format!("{url}#px={code}"));
    database.keep_preference(AVATAR, &json!(url))?;
    Ok(json!({ "avatarUrl": url }))
}

fn keep(database: &dyn Database, profile: Value, part: &str, value: Value) -> Result<(), String> {
    let mut profile = profile.as_object().cloned().unwrap_or_default();
    profile.insert(part.to_owned(), value);
    database.keep_preference(KEY, &Value::Object(profile))
}

pub fn forget(_client: u64) {}

#[cfg(test)]
mod tests;
