use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use super::*;
use crate::feature::testing;

const PNG: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

fn profile_with(database: Arc<store::Memory>) -> impl Fn(Value) -> Result<Value, String> {
    move |input| {
        let mut services = testing::services(None);
        services.database = database.clone();
        testing::with(1, &services, |context| handle(context, input))
    }
}

#[test]
fn a_new_owner_has_the_default_prefs() {
    let call = profile_with(Arc::default());
    assert_eq!(
        call(json!({ "op": "prefs" })),
        Ok(json!({
            "start": { "page": "home", "channelId": null },
            "showActivity": false,
            "agentStyle": null,
            "chatPrefs": {},
            "handle": "local",
            "timezone": "UTC",
            "avatarUrl": null,
        }))
    );
}

#[test]
fn each_choice_is_kept_and_shows_in_the_prefs() -> Result<(), String> {
    let call = profile_with(Arc::default());
    assert_eq!(call(json!({ "op": "set_start", "page": "channel", "channelId": "c1" }))?["channelId"], "c1");
    assert_eq!(call(json!({ "op": "set_activity", "show": true })), Ok(json!({ "show": true })));
    assert_eq!(call(json!({ "op": "set_agent_style", "style": "technical" })), Ok(json!({ "style": "technical" })));
    call(json!({ "op": "set_chat", "textSize": "lg" }))?;
    assert_eq!(
        call(json!({ "op": "set_chat", "dmTone": "strong" })),
        Ok(json!({ "textSize": "lg", "dmTone": "strong" }))
    );

    let prefs = call(json!({ "op": "prefs" }))?;
    assert_eq!(prefs["start"], json!({ "page": "channel", "channelId": "c1" }));
    assert_eq!(prefs["showActivity"], true);
    assert_eq!(prefs["agentStyle"], "technical");
    assert_eq!(prefs["chatPrefs"], json!({ "textSize": "lg", "dmTone": "strong" }));
    Ok(())
}

#[test]
fn updating_me_answers_the_person_with_the_new_handle_and_zone() -> Result<(), String> {
    let call = profile_with(Arc::default());
    let person = call(json!({ "op": "update_me", "handle": "@Bruno", "timezone": "Europe/Lisbon" }))?;
    assert_eq!(person["userId"], "local");
    assert_eq!(person["name"], brand::NAME);
    assert_eq!(person["handle"], "bruno");
    assert_eq!(person["timezone"], "Europe/Lisbon");
    assert_eq!(person["presence"], "online");

    let back = call(json!({ "op": "update_me", "timezone": null }))?;
    assert_eq!((back["handle"].clone(), back["timezone"].clone()), (json!("bruno"), json!("UTC")));
    Ok(())
}

#[test]
fn the_issues_view_is_the_default_until_kept_and_comes_back_clean() -> Result<(), String> {
    let call = profile_with(Arc::default());
    assert_eq!(call(json!({ "op": "issues_view" }))?["view"]["layout"], "board");
    let kept = call(json!({ "op": "set_issues_view", "view": { "layout": "list", "assignees": ["none", "none"] } }))?;
    assert_eq!(kept["view"]["assignees"], json!(["none"]));
    assert_eq!(call(json!({ "op": "issues_view" }))?, kept);
    Ok(())
}

#[test]
fn a_photo_is_kept_as_a_data_url_until_removed() -> Result<(), String> {
    let call = profile_with(Arc::default());
    let encoded = STANDARD.encode(PNG);
    let set =
        call(json!({ "op": "set_avatar", "imageBase64": encoded, "contentType": "image/png", "portrait": "1.f.s3" }))?;
    let url = format!("data:image/png;base64,{encoded}#px=1.f.s3");
    assert_eq!(set, json!({ "avatarUrl": url }));
    assert_eq!(call(json!({ "op": "prefs" }))?["avatarUrl"], json!(url));
    assert_eq!(call(json!({ "op": "update_me" }))?["image"], json!(url));

    assert_eq!(call(json!({ "op": "remove_avatar" })), Ok(Value::Null));
    assert_eq!(call(json!({ "op": "prefs" }))?["avatarUrl"], Value::Null);
    Ok(())
}

#[test]
fn bad_choices_are_refused_and_nothing_changes() -> Result<(), String> {
    let call = profile_with(Arc::default());
    for input in [
        json!({ "op": "set_start", "page": "channel" }),
        json!({ "op": "set_activity", "show": "yes" }),
        json!({ "op": "set_agent_style", "style": "loud" }),
        json!({ "op": "set_chat", "textSize": "huge" }),
        json!({ "op": "update_me", "handle": "a" }),
        json!({ "op": "set_issues_view", "view": { "widths": { "title": 5 } } }),
        json!({ "op": "set_avatar", "imageBase64": STANDARD.encode(PNG), "contentType": "image/jpeg" }),
        json!({ "op": "set_avatar", "imageBase64": STANDARD.encode(PNG), "contentType": "image/png", "portrait": "x" }),
    ] {
        assert!(call(input.clone()).is_err(), "{input}");
    }
    assert_eq!(call(json!({ "op": "prefs" }))?["start"]["page"], "home");
    assert_eq!(call(json!({ "op": "prefs" }))?["avatarUrl"], Value::Null);
    Ok(())
}
