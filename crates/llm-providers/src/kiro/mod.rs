pub mod auth;
pub mod idc;
pub mod models;

use domain::ModelProvider;
use serde_json::{Value, json};

pub const ORIGIN: &str = "KIRO_CLI";
pub const CHAT: &str = "AmazonCodeWhispererStreamingService.GenerateAssistantResponse";
pub const MODELS: &str = "KiroControlPlaneBearerService.ListAvailableModels";
const DEFAULT_REGION: &str = "us-east-1";

pub fn region(profile_arn: &str) -> &str {
    profile_arn
        .split(':')
        .nth(3)
        .filter(|region| {
            !region.is_empty() && region.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
        .unwrap_or(DEFAULT_REGION)
}

pub fn runtime(profile_arn: &str) -> String {
    format!("https://runtime.{}.kiro.dev", region(profile_arn))
}

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
