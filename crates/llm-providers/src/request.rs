use super::{Provider, ProviderError};
use domain::ModelOption;
use serde_json::{Value, json};

pub const STREAM_LIMIT: usize = 8 << 20;
pub const ERROR_LIMIT: usize = 4 << 10;

pub fn shape(provider: &dyn Provider, model: Option<&ModelOption>, body: &mut Value) -> Result<(), ProviderError> {
    let prefix = format!("{}:", provider.id());
    if let Some(name) = body["model"].as_str() {
        body["model"] = json!(name.strip_prefix(&prefix).unwrap_or(name));
    }
    let effort = body["reasoning"]["effort"]
        .as_str()
        .or_else(|| body["output_config"]["effort"].as_str())
        .unwrap_or("medium")
        .to_owned();
    let named = body["model"].as_str().unwrap_or_default().to_owned();
    crate::web_search::put(provider.web_search(&named), body);
    if !provider.tools_in_conversation(&named) {
        crate::added_tools::into_tools(body);
    }
    provider.shape(model, body, &effort)
}

pub fn remove(body: &mut Value, keys: &[&str]) {
    if let Some(object) = body.as_object_mut() {
        for key in keys {
            object.remove(*key);
        }
    }
}

pub(crate) fn capabilities<'a>(
    body: &Value,
    selected: Option<&'a ModelOption>,
) -> Option<&'a domain::model::ModelCapabilities> {
    let model = body["model"].as_str().unwrap_or_default();
    selected.filter(|s| s.id.rsplit(':').next() == Some(model)).and_then(|s| s.capabilities.as_ref())
}

pub(crate) fn unsupported(caps: Option<&domain::model::ModelCapabilities>, what: &str) -> bool {
    caps.is_some_and(|c| !c.reasoning_levels.is_empty() && !c.reasoning_levels.iter().any(|l| l == what))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_provider_prefix_leaves_the_model_name() {
        let mut body = json!({"model": "keyed:some-model"});
        shape(&crate::testing::KEYED, None, &mut body).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(body["model"], "some-model");
    }

    #[test]
    fn a_provider_that_does_not_take_tools_in_the_conversation_gets_them_in_the_list() {
        let mut body = json!({
            "model": "keyed:gpt-6-astra",
            "tools": [],
            "input": [{"type": "additional_tools", "role": "developer", "tools": [{"name": "a"}]}],
        });
        shape(&crate::testing::KEYED, None, &mut body).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(body["tools"], json!([{"name": "a"}]));
        assert_eq!(body["input"], json!([]));
    }
}
