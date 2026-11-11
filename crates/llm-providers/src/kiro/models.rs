//! Kiro's `ListAvailableModels`: the models this account may pick, a page
//! at a time, each with its token limits.
use domain::model::ModelCapabilities;
use domain::{ModelOption, ModelProvider};
use serde_json::{Value, json};

/// The POST that asks for a catalog page: where, with which headers and
/// body.
#[derive(Clone, PartialEq, Eq)]
pub struct Call {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl std::fmt::Debug for Call {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Call").field("url", &self.url).finish_non_exhaustive()
    }
}

/// The call for the page after `next` with this account's credential.
pub fn call(config: &crate::Config, next: &str) -> Call {
    let (token, arn) = config.auth.as_ref().map_or(("", ""), |t| (t.access_token.as_str(), t.account_id.as_str()));
    Call {
        url: format!("{}/", super::management(arn)),
        headers: super::headers(token, super::MODELS),
        body: request(arn, next),
    }
}

/// The request for the page after `next` (empty for the first).
pub fn request(profile_arn: &str, next: &str) -> String {
    let mut body = json!({"origin": super::ORIGIN, "profileArn": profile_arn});
    if !next.is_empty() {
        body["nextToken"] = json!(next);
    }
    body.to_string()
}

/// A page's models, or `None` when it has no model list; and the next
/// page's token.
pub fn page(value: &Value) -> Option<(Vec<ModelOption>, Option<String>)> {
    let models = value["models"].as_array()?.iter().filter_map(model).collect();
    let next = value["nextToken"].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    Some((models, next))
}

fn model(item: &Value) -> Option<ModelOption> {
    let id = item["modelId"].as_str().map(str::trim).filter(|id| !id.is_empty())?;
    let label = item["modelName"].as_str().map(str::trim).filter(|name| !name.is_empty()).unwrap_or(id);
    let provider = ModelProvider::KiroSubscription;
    let mut model = ModelOption::new(format!("{}:{id}", provider.id()), label).served_by(provider);
    let limits = &item["tokenLimits"];
    model.window = limits["maxInputTokens"].as_u64().filter(|n| *n > 0);
    model.capabilities = Some(ModelCapabilities {
        max_output_tokens: limits["maxOutputTokens"].as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n > 0),
        ..ModelCapabilities::default()
    });
    Some(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_lists_ids_names_and_limits() {
        let page = page(&json!({"defaultModel": {"modelId": "auto"}, "models": [
            {"modelId": "auto", "modelName": "auto", "tokenLimits": {"maxInputTokens": 1000000, "maxOutputTokens": 64000}},
            {"modelId": "glm-5", "modelName": "glm-5", "tokenLimits": {"maxInputTokens": 200000}},
            {"modelName": "nameless"}
        ], "nextToken": "n2"}));
        let (models, next) = page.unwrap_or_default();
        let ids: Vec<_> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["kiro-subscription:auto", "kiro-subscription:glm-5"]);
        assert_eq!(models[0].window, Some(1_000_000));
        assert_eq!(models[0].capabilities.as_ref().and_then(|c| c.max_output_tokens), Some(64_000));
        assert_eq!(next.as_deref(), Some("n2"));
    }

    #[test]
    fn the_catalog_is_asked_of_the_profiles_management_service() {
        let tokens = crate::Tokens {
            access_token: "tok".into(),
            refresh_token: "r".into(),
            account_id: "arn:aws:codewhisperer:us-east-1:1:profile/P".into(),
            expires_at: None,
            idc: None,
        };
        let config = crate::Config { auth: Some(tokens), ..crate::Config::default() };
        let call = call(&config, "");
        assert_eq!(call.url, "https://management.us-east-1.kiro.dev/");
        assert!(call.headers.contains(&("x-amz-target".into(), super::super::MODELS.into())));
        assert!(call.headers.contains(&("authorization".into(), "Bearer tok".into())));
    }

    #[test]
    fn the_request_names_the_cli_the_account_and_the_page() {
        let first: Value = serde_json::from_str(&request("arn:p", "")).unwrap_or_default();
        assert_eq!(first, json!({"origin": "KIRO_CLI", "profileArn": "arn:p"}));
        let second: Value = serde_json::from_str(&request("arn:p", "n2")).unwrap_or_default();
        assert_eq!(second["nextToken"], "n2");
        assert!(page(&json!({})).is_none());
    }
}
