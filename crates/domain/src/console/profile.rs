use serde_json::{Map, Value, json};

pub const OWNER_ID: &str = "local";
pub const DEFAULT_HANDLE: &str = "local";
pub const JOINED_AT: &str = "1970-01-01T00:00:00.000Z";
pub const START_PAGES: [&str; 3] = ["home", "last", "channel"];
pub const AGENT_STYLES: [&str; 2] = ["simple", "technical"];
pub const CHAT_STYLES: [&str; 4] = ["comfortable", "left", "bubbles", "compact"];
pub const CHAT_TONES: [&str; 2] = ["light", "strong"];
pub const CHAT_OPEN_AT: [&str; 2] = ["latest", "read"];
pub const CHAT_TEXT_SIZES: [&str; 5] = ["xs", "sm", "md", "lg", "xl"];
const CHAT_CHOICES: [(&str, &[&str]); 6] = [
    ("channelStyle", &CHAT_STYLES),
    ("dmStyle", &CHAT_STYLES),
    ("channelTone", &CHAT_TONES),
    ("dmTone", &CHAT_TONES),
    ("openAt", &CHAT_OPEN_AT),
    ("textSize", &CHAT_TEXT_SIZES),
];
const CHAT_FLAGS: [&str; 2] = ["fullMessages", "feedbackAuto"];
const HANDLE_INPUT_MAX: usize = 40;
const TIMEZONE_MAX: usize = 64;

pub fn handle_of(raw: &str) -> Result<String, String> {
    if raw.chars().count() > HANDLE_INPUT_MAX {
        return Err("handle: 2 to 32 lowercase letters, numbers, - or _".into());
    }
    let handle = raw.trim().trim_start_matches('@').to_lowercase();
    let mut chars = handle.chars();
    let first_fits = chars.next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let rest_fits = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if first_fits && rest_fits && (2..=32).contains(&handle.len()) {
        Ok(handle)
    } else {
        Err("handle: 2 to 32 lowercase letters, numbers, - or _".into())
    }
}

pub fn timezone_allowed(zone: &str) -> bool {
    let part_fits = |part: &str| {
        part.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && part.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+'))
    };
    zone.len() <= TIMEZONE_MAX && !zone.is_empty() && zone.split('/').all(part_fits)
}

pub fn start_of(stored: &Value) -> Value {
    let page = stored["page"].as_str().filter(|page| START_PAGES.contains(page)).unwrap_or("home");
    let channel = if page == "channel" { stored["channelId"].clone() } else { Value::Null };
    if page == "channel" && channel.as_str().is_none_or(str::is_empty) {
        return json!({ "page": "home", "channelId": null });
    }
    json!({ "page": page, "channelId": channel })
}

pub fn checked_start(input: &Value) -> Result<Value, String> {
    let page = input["page"].as_str().filter(|page| START_PAGES.contains(page));
    let page = page.ok_or("page must be home, last or channel")?;
    if page != "channel" {
        return Ok(json!({ "page": page, "channelId": null }));
    }
    match input["channelId"].as_str() {
        Some(channel) if !channel.trim().is_empty() => Ok(json!({ "page": page, "channelId": channel })),
        _ => Err("Not a channel you are in".into()),
    }
}

pub fn agent_style_of(stored: &Value) -> Value {
    stored.as_str().filter(|style| AGENT_STYLES.contains(style)).map_or(Value::Null, |style| json!(style))
}

pub fn checked_agent_style(style: &Value) -> Result<Value, String> {
    match agent_style_of(style) {
        Value::Null => Err("style must be simple or technical".into()),
        style => Ok(style),
    }
}

pub fn chat_prefs_of(raw: &Value) -> Value {
    let mut kept = Map::new();
    for (key, list) in CHAT_CHOICES {
        if let Some(choice) = raw[key].as_str().filter(|choice| list.contains(choice)) {
            kept.insert(key.to_owned(), json!(choice));
        }
    }
    for key in CHAT_FLAGS {
        if let Some(on) = raw[key].as_bool() {
            kept.insert(key.to_owned(), json!(on));
        }
    }
    Value::Object(kept)
}

pub fn merged_chat_prefs(stored: &Value, patch: &Value) -> Result<Value, String> {
    for (key, list) in CHAT_CHOICES {
        let value = &patch[key];
        if !value.is_null() && value.as_str().is_none_or(|choice| !list.contains(&choice)) {
            return Err(format!("{key} must be one of {}", list.join(", ")));
        }
    }
    for key in CHAT_FLAGS {
        if !patch[key].is_null() && !patch[key].is_boolean() {
            return Err(format!("{key} must be true or false"));
        }
    }
    let mut merged = chat_prefs_of(stored);
    if let (Some(merged), Value::Object(changes)) = (merged.as_object_mut(), chat_prefs_of(patch)) {
        merged.extend(changes);
    }
    Ok(merged)
}

pub fn with_profile_change(profile: &Value, input: &Value) -> Result<Value, String> {
    let mut changed = profile.as_object().cloned().unwrap_or_default();
    match &input["handle"] {
        Value::Null => {}
        Value::String(raw) => {
            changed.insert("handle".into(), json!(handle_of(raw)?));
        }
        _ => return Err("handle must be text".into()),
    }
    match input.get("timezone") {
        None => {}
        Some(Value::Null) => {
            changed.remove("timezone");
        }
        Some(Value::String(zone)) if timezone_allowed(zone) => {
            changed.insert("timezone".into(), json!(zone));
        }
        Some(_) => return Err("timezone must be an IANA time zone".into()),
    }
    Ok(Value::Object(changed))
}

pub fn person(name: &str, profile: &Value, avatar: &Value, org_timezone: &str) -> Value {
    let avatar = avatar.as_str().map_or(Value::Null, |url| json!(url));
    json!({
        "userId": OWNER_ID,
        "name": name,
        "email": "",
        "image": avatar,
        "avatarUrl": avatar,
        "role": "owner",
        "roleIds": [],
        "joinedAt": JOINED_AT,
        "lastSeenAt": null,
        "online": true,
        "presence": "online",
        "status": null,
        "availability": "auto",
        "handle": profile["handle"].as_str().unwrap_or(DEFAULT_HANDLE),
        "timezone": profile["timezone"].as_str().unwrap_or(org_timezone),
    })
}

pub fn prefs(profile: &Value, avatar: &Value, org_timezone: &str) -> Value {
    json!({
        "start": start_of(&profile["start"]),
        "showActivity": profile["showActivity"].as_bool().unwrap_or(false),
        "agentStyle": agent_style_of(&profile["agentStyle"]),
        "chatPrefs": chat_prefs_of(&profile["chatPrefs"]),
        "handle": profile["handle"].as_str().unwrap_or(DEFAULT_HANDLE),
        "timezone": profile["timezone"].as_str().unwrap_or(org_timezone),
        "avatarUrl": avatar.as_str().map_or(Value::Null, |url| json!(url)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handle_is_trimmed_lowercased_and_loses_its_at() {
        assert_eq!(handle_of("  @Ana_B "), Ok("ana_b".into()));
        assert!(handle_of("a").is_err());
        assert!(handle_of("-ana").is_err());
        assert!(handle_of("ana b").is_err());
        assert!(handle_of(&"a".repeat(33)).is_err());
    }

    #[test]
    fn time_zones_look_like_iana_names() {
        assert!(timezone_allowed("UTC"));
        assert!(timezone_allowed("America/Sao_Paulo"));
        assert!(timezone_allowed("Etc/GMT+3"));
        assert!(!timezone_allowed(""));
        assert!(!timezone_allowed("Mars/"));
        assert!(!timezone_allowed("not a zone"));
    }

    #[test]
    fn the_start_page_is_home_until_chosen_and_a_channel_needs_an_id() {
        assert_eq!(start_of(&Value::Null), json!({ "page": "home", "channelId": null }));
        assert_eq!(
            checked_start(&json!({ "page": "last", "channelId": "c1" })),
            Ok(json!({ "page": "last", "channelId": null }))
        );
        assert_eq!(
            checked_start(&json!({ "page": "channel", "channelId": "c1" })),
            Ok(json!({ "page": "channel", "channelId": "c1" }))
        );
        assert!(checked_start(&json!({ "page": "channel" })).is_err());
        assert!(checked_start(&json!({ "page": "inbox" })).is_err());
    }

    #[test]
    fn only_simple_or_technical_is_an_agent_style() {
        assert_eq!(agent_style_of(&Value::Null), Value::Null);
        assert_eq!(checked_agent_style(&json!("technical")), Ok(json!("technical")));
        assert!(checked_agent_style(&json!("loud")).is_err());
    }

    #[test]
    fn chat_prefs_change_only_the_keys_sent() {
        let stored = json!({ "channelStyle": "bubbles", "textSize": "lg", "junk": 1 });
        let merged = merged_chat_prefs(&stored, &json!({ "textSize": "sm", "fullMessages": true }));
        assert_eq!(merged, Ok(json!({ "channelStyle": "bubbles", "textSize": "sm", "fullMessages": true })));
        assert!(merged_chat_prefs(&stored, &json!({ "dmTone": "dark" })).is_err());
        assert!(merged_chat_prefs(&stored, &json!({ "feedbackAuto": "yes" })).is_err());
    }

    #[test]
    fn a_profile_change_sets_the_handle_and_null_puts_the_zone_back() {
        let profile = json!({ "timezone": "Europe/Lisbon" });
        let changed = with_profile_change(&profile, &json!({ "handle": "@Bruno", "timezone": null }));
        assert_eq!(changed, Ok(json!({ "handle": "bruno" })));
        assert!(with_profile_change(&profile, &json!({ "timezone": "nowhere at all" })).is_err());
        assert_eq!(with_profile_change(&profile, &json!({})), Ok(profile));
    }

    #[test]
    fn the_owner_is_one_person_with_the_org_zone_until_they_pick_one() {
        let person = person("Ana", &json!({}), &Value::Null, "UTC");
        assert_eq!(person["userId"], OWNER_ID);
        assert_eq!(person["role"], "owner");
        assert_eq!(person["handle"], DEFAULT_HANDLE);
        assert_eq!(person["timezone"], "UTC");
        assert_eq!(person["avatarUrl"], Value::Null);
    }

    #[test]
    fn prefs_carry_every_choice_with_its_default() {
        let prefs = prefs(&json!({ "showActivity": true }), &json!("data:image/png;base64,AA"), "UTC");
        assert_eq!(
            prefs,
            json!({
                "start": { "page": "home", "channelId": null },
                "showActivity": true,
                "agentStyle": null,
                "chatPrefs": {},
                "handle": "local",
                "timezone": "UTC",
                "avatarUrl": "data:image/png;base64,AA",
            })
        );
    }
}
