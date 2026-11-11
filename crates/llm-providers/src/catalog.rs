use super::{Config, Provider, ProviderError};
use domain::ModelOption;
use domain::model::ModelCapabilities;
use serde_json::Value;

pub const MAX_PAGES: usize = 100;
pub const PAGE_LIMIT: u64 = 8 << 20;

#[derive(Clone, PartialEq, Eq)]
pub struct Call {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

impl std::fmt::Debug for Call {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Call").field("url", &self.url).finish_non_exhaustive()
    }
}

impl Call {
    pub fn get(provider: &dyn Provider, config: &Config, path: &str) -> Self {
        Self { url: format!("{}{path}", config.base(provider)), headers: config.headers(provider), body: None }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub models: Vec<ModelOption>,
    pub next: Option<String>,
}

pub fn collect<E: From<ProviderError>>(
    provider: &dyn Provider,
    mut fetch: impl FnMut(&str) -> Result<Value, E>,
) -> Result<Vec<ModelOption>, E> {
    let mut out = Vec::new();
    let mut after = String::new();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..MAX_PAGES {
        let page = provider.page(&fetch(&after)?)?;
        out.extend(page.models);
        let Some(next) = page.next else {
            return Ok(provider.finish(out));
        };
        if next.is_empty() || next == after || !seen.insert(next.clone()) {
            return Err(ProviderError::Invalid("model pagination cursor did not advance".into()).into());
        }
        after = next;
    }
    Err(ProviderError::Invalid("model pagination page limit exceeded".into()).into())
}

pub fn option(provider: &dyn Provider, id: &str, label: String) -> ModelOption {
    ModelOption::new(format!("{}:{id}", provider.id()), label).served_by(crate::id(provider))
}

pub fn items<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, ProviderError> {
    value[key].as_array().ok_or_else(|| ProviderError::Invalid("model catalog has no model array".into()))
}

#[derive(Debug, Clone, Copy)]
pub struct Fields {
    pub id: &'static str,
    pub window: &'static str,
}

pub const STANDARD: Fields = Fields { id: "id", window: "max_input_tokens" };

pub fn model(provider: &dyn Provider, item: &Value, fields: &Fields) -> Option<ModelOption> {
    let id = string(item, fields.id);
    if id.is_empty() {
        return None;
    }
    let label = string(item, "display_name");
    let mut model = option(provider, id, if label.is_empty() { id.to_owned() } else { label.to_owned() });
    model.window = item[fields.window].as_u64().filter(|n| *n > 0);
    model.capabilities = Some(ModelCapabilities {
        reasoning: item["capabilities"]["thinking"]["supported"] == true,
        adaptive_thinking: item["capabilities"]["thinking"]["types"]["adaptive"]["supported"] == true,
        max_output_tokens: item["max_tokens"].as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n > 0),
        auto_compact_token_limit: item["auto_compact_token_limit"].as_u64().filter(|n| *n > 0),
        effective_context_window_percent: item["effective_context_window_percent"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0),
        reasoning_levels: levels(item),
        endpoints: Vec::new(),
    });
    Some(model)
}

pub fn levels(item: &Value) -> Vec<String> {
    item["supported_reasoning_levels"]
        .as_array()
        .map(|levels| levels.iter().filter_map(|level| level["effort"].as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

pub fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default().trim()
}

pub fn reasoning(model: &mut ModelOption, reasoning: bool) {
    if let Some(capabilities) = model.capabilities.as_mut() {
        capabilities.reasoning = reasoning;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::KEYED;
    use serde_json::json;

    #[test]
    fn pages_are_followed_until_the_last_and_a_repeated_cursor_is_refused() {
        let mut asked = Vec::new();
        let models = collect(&KEYED, |after| {
            asked.push(after.to_owned());
            Ok::<_, ProviderError>(if after.is_empty() {
                json!({"data": [{"id": "a"}], "next": "p2"})
            } else {
                json!({"data": [{"id": "b"}]})
            })
        });
        assert_eq!(models.map(|m| m.len()), Ok(2));
        assert_eq!(asked, ["", "p2"]);
        let stuck = collect(&KEYED, |_| Ok::<_, ProviderError>(json!({"data": [], "next": "x"})));
        assert!(stuck.is_err());
    }

    #[test]
    fn a_standard_item_gives_its_window_and_thinking() {
        let item = json!({"id": "m", "display_name": "M", "max_input_tokens": 9,
            "capabilities": {"thinking": {"supported": true}}});
        let model = model(&KEYED, &item, &STANDARD).unwrap_or_else(|| panic!("a model"));
        assert_eq!((model.id.as_str(), model.label.as_str(), model.window), ("keyed:m", "M", Some(9)));
        assert!(model.capabilities.is_some_and(|c| c.reasoning));
    }
}
