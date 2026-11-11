//! The Kiro subscription as Kiro CLI 3.1.0 speaks it: the bearer token from
//! Kiro's sign-in, the profile ARN that names the account (and its region),
//! `GenerateAssistantResponse` on the runtime service for turns and
//! `ListAvailableModels` on the management service for the catalog. Both
//! are AWS JSON 1.0 calls picked by `x-amz-target`.
pub mod auth;
pub mod idc;
pub mod models;

use serde_json::{Value, json};
use domain::ModelProvider;

pub const ORIGIN: &str = "KIRO_CLI";
pub const CHAT: &str = "AmazonCodeWhispererStreamingService.GenerateAssistantResponse";
pub const MODELS: &str = "KiroControlPlaneBearerService.ListAvailableModels";
const DEFAULT_REGION: &str = "us-east-1";

/// The region in a profile ARN (`arn:aws:codewhisperer:<region>:…`), or
/// `us-east-1` when it is missing or not a region name.
pub fn region(profile_arn: &str) -> &str {
    profile_arn
        .split(':')
        .nth(3)
        .filter(|region| {
            !region.is_empty() && region.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
        .unwrap_or(DEFAULT_REGION)
}

/// Where turns are served for this account.
pub fn runtime(profile_arn: &str) -> String {
    format!("https://runtime.{}.kiro.dev", region(profile_arn))
}

/// Where the account's model catalog is served.
pub fn management(profile_arn: &str) -> String {
    format!("https://management.{}.kiro.dev", region(profile_arn))
}

pub fn headers(access_token: &str, target: &str) -> Vec<(String, String)> {
    vec![
        ("authorization".into(), format!("Bearer {access_token}")),
        ("content-type".into(), "application/x-amz-json-1.0".into()),
        ("x-amz-target".into(), target.into()),
    ]
}

/// Leaves Kiro's own model name in every message of a turn.
pub fn shape(body: &mut Value) {
    let prefix = format!("{}:", ModelProvider::KiroSubscription.id());
    let state = &mut body["conversationState"];
    let strip = |message: &mut Value| {
        if let Some(name) = message["userInputMessage"]["modelId"].as_str() {
            let bare = name.strip_prefix(&prefix).unwrap_or(name).to_owned();
            message["userInputMessage"]["modelId"] = json!(bare);
        }
    };
    strip(&mut state["currentMessage"]);
    if let Some(history) = state["history"].as_array_mut() {
        history.iter_mut().for_each(strip);
    }
}

/// Names the account a turn is billed to.
pub fn bind(body: &mut Value, profile_arn: &str) {
    body["profileArn"] = json!(profile_arn);
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARN: &str = "arn:aws:codewhisperer:eu-central-1:699475941385:profile/EHGA3GRVQMUK";

    #[test]
    fn the_profile_arn_picks_the_region_of_both_services() {
        assert_eq!(runtime(ARN), "https://runtime.eu-central-1.kiro.dev");
        assert_eq!(management(ARN), "https://management.eu-central-1.kiro.dev");
        assert_eq!(region(""), "us-east-1");
        assert_eq!(region("arn:aws:codewhisperer:evil.host/x:1:p"), "us-east-1");
    }

    #[test]
    fn a_turn_names_the_bare_model_and_the_account() {
        let mut body = json!({"conversationState": {
            "currentMessage": {"userInputMessage": {"modelId": "kiro-subscription:claude-sonnet-4.5"}},
            "history": [{"userInputMessage": {"modelId": "kiro-subscription:auto"}}, {"assistantResponseMessage": {}}]
        }});
        shape(&mut body);
        bind(&mut body, ARN);
        assert_eq!(body["conversationState"]["currentMessage"]["userInputMessage"]["modelId"], "claude-sonnet-4.5");
        assert_eq!(body["conversationState"]["history"][0]["userInputMessage"]["modelId"], "auto");
        assert_eq!(body["profileArn"], ARN);
    }
}
