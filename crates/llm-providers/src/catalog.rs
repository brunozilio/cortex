//! Each provider's model catalog: which page to ask for and what a page
//! lists.
use super::ProviderError;
use serde_json::Value;
use domain::model::ModelCapabilities;
use domain::{ModelOption, ModelProvider};

pub const MAX_PAGES: usize = 100;
pub const PAGE_LIMIT: u64 = 8 << 20;
pub const CLAUDE_FALLBACK: [&str; 4] = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-fable-5-1"];

pub fn path(provider: ModelProvider, after: &str) -> String {
    match provider {
        ModelProvider::ChatGptSubscription => "/codex/models?client_version=0.0.0".to_owned(),
        ModelProvider::OpenAi | ModelProvider::CopilotSubscription => "/models".to_owned(),
        _ if after.is_empty() => "/models?limit=1000".to_owned(),
        _ => format!("/models?limit=1000&after_id={}", super::oauth::form(&[("v", after)]).trim_start_matches("v=")),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub models: Vec<ModelOption>,
    pub next: Option<String>,
}

pub fn page(provider: ModelProvider, value: &Value) -> Result<Page, ProviderError> {
    if provider == ModelProvider::KiroSubscription {
        let (models, next) = super::kiro::models::page(value)
            .ok_or_else(|| ProviderError::Invalid("model catalog has no model array".into()))?;
        return Ok(Page { models, next });
    }
    let codex = provider == ModelProvider::ChatGptSubscription;
    let items = value[if codex { "models" } else { "data" }]
        .as_array()
        .ok_or_else(|| ProviderError::Invalid("model catalog has no model array".into()))?;
    let models = items.iter().filter_map(|item| model(provider, item)).collect();
    let next = (provider == ModelProvider::Anthropic && value["has_more"] == true)
        .then(|| string(value, "last_id").to_owned());
    Ok(Page { models, next })
}

/// Every page of a catalog asked for by its [`path`].
pub fn collect<E: From<ProviderError>>(
    provider: ModelProvider,
    mut fetch: impl FnMut(&str) -> Result<Value, E>,
) -> Result<Vec<ModelOption>, E> {
    collect_pages(provider, |after| fetch(&path(provider, after)))
}

/// Every page of a catalog; `fetch` gets the cursor of the page to ask
/// for, empty for the first.
pub fn collect_pages<E: From<ProviderError>>(
    provider: ModelProvider,
    mut fetch: impl FnMut(&str) -> Result<Value, E>,
) -> Result<Vec<ModelOption>, E> {
    let mut out = Vec::new();
    let mut after = String::new();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..MAX_PAGES {
        let page = page(provider, &fetch(&after)?)?;
        out.extend(page.models);
        let Some(next) = page.next else {
            return Ok(finish(provider, out));
        };
        if next.is_empty() || next == after || !seen.insert(next.clone()) {
            return Err(ProviderError::Invalid("model pagination cursor did not advance".into()).into());
        }
        after = next;
    }
    Err(ProviderError::Invalid("model pagination page limit exceeded".into()).into())
}

fn finish(provider: ModelProvider, mut out: Vec<ModelOption>) -> Vec<ModelOption> {
    if provider == ModelProvider::OpenAi {
        out.sort_by(|a, b| a.label.cmp(&b.label));
    }
    if out.is_empty() && provider == ModelProvider::ClaudeSubscription {
        out.extend(CLAUDE_FALLBACK.map(|id| option(provider, id, id.to_owned())));
    }
    out
}

fn option(provider: ModelProvider, id: &str, label: String) -> ModelOption {
    ModelOption::new(format!("{}:{id}", provider.id()), label).served_by(provider)
}

fn model(provider: ModelProvider, item: &Value) -> Option<ModelOption> {
    if provider == ModelProvider::CopilotSubscription {
        return super::copilot::models::model(item);
    }
    let codex = provider == ModelProvider::ChatGptSubscription;
    let id = string(item, if codex { "slug" } else { "id" });
    if id.is_empty() || (provider == ModelProvider::OpenAi && !is_chat_model(id)) {
        return None;
    }
    let label = string(item, "display_name");
    let label = if provider == ModelProvider::OpenAi {
        format!("{id} (OpenAI)")
    } else if label.is_empty() {
        id.to_owned()
    } else {
        label.to_owned()
    };
    let mut model = option(provider, id, label);
    model.window = item[if codex { "context_window" } else { "max_input_tokens" }].as_u64().filter(|n| *n > 0);
    let levels = item["supported_reasoning_levels"]
        .as_array()
        .map(|levels| levels.iter().filter_map(|level| level["effort"].as_str().map(str::to_owned)).collect::<Vec<_>>())
        .unwrap_or_default();
    let lower = id.to_ascii_lowercase();
    model.capabilities = Some(ModelCapabilities {
        reasoning: match provider {
            ModelProvider::ChatGptSubscription => !levels.is_empty(),
            ModelProvider::OpenAi => REASONING_PREFIXES.iter().any(|p| lower.starts_with(p)),
            ModelProvider::ClaudeSubscription => true,
            _ => item["capabilities"]["thinking"]["supported"] == true,
        },
        adaptive_thinking: item["capabilities"]["thinking"]["types"]["adaptive"]["supported"] == true,
        max_output_tokens: item["max_tokens"].as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n > 0),
        auto_compact_token_limit: item["auto_compact_token_limit"].as_u64().filter(|n| *n > 0),
        effective_context_window_percent: item["effective_context_window_percent"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0),
        reasoning_levels: levels,
        endpoints: Vec::new(),
    });
    Some(model)
}

pub const REASONING_PREFIXES: [&str; 5] = ["o1", "o3", "o4", "gpt-5", "gpt-6"];

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default().trim()
}

pub fn is_chat_model(id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    ![
        "embedding",
        "whisper",
        "tts",
        "transcribe",
        "dall-e",
        "moderation",
        "image",
        "realtime",
        "audio",
        "search",
        "codex",
        "babbage",
        "davinci",
    ]
    .iter()
    .any(|part| lower.contains(part))
        && ["gpt", "o1", "o3", "o4", "chatgpt"].iter().any(|prefix| lower.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read(provider: ModelProvider, mut fetch: impl FnMut(&str) -> Value) -> Result<Vec<ModelOption>, ProviderError> {
        collect(provider, |path| Ok::<_, ProviderError>(fetch(path)))
    }

    #[test]
    fn openai_keeps_chat_models_sorted() {
        let page = json!({"data": [{"id": "o3"}, {"id": "gpt-4o"}, {"id": "whisper-1"}, {"id": "text-embedding-3"}]});
        let models = read(ModelProvider::OpenAi, |_| page.clone()).unwrap_or_default();
        let ids: Vec<_> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["openai:gpt-4o", "openai:o3"]);
    }

    #[test]
    fn anthropic_follows_its_cursor_and_refuses_a_repeated_one() {
        let mut calls = Vec::new();
        let models = read(ModelProvider::Anthropic, |path| {
            calls.push(path.to_owned());
            if calls.len() == 1 {
                json!({"data": [{"id": "claude-a"}], "has_more": true, "last_id": "claude-a"})
            } else {
                json!({"data": [{"id": "claude-b"}], "has_more": false})
            }
        });
        assert_eq!(models.map(|m| m.len()), Ok(2));
        assert_eq!(calls[1], "/models?limit=1000&after_id=claude-a");
        let stuck = json!({"data": [], "has_more": true, "last_id": "x"});
        assert!(read(ModelProvider::Anthropic, |_| stuck.clone()).is_err());
    }

    #[test]
    fn an_empty_claude_subscription_catalog_falls_back() {
        let models = read(ModelProvider::ClaudeSubscription, |_| json!({"data": []})).unwrap_or_default();
        assert_eq!(models.len(), CLAUDE_FALLBACK.len());
        assert!(read(ModelProvider::ClaudeSubscription, |_| json!({})).is_err());
    }

    #[test]
    fn the_claude_fallback_is_the_current_family_and_priced() {
        assert_eq!(CLAUDE_FALLBACK, ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-fable-5-1"]);
        for id in CLAUDE_FALLBACK {
            assert!(domain::billing::rates(ModelProvider::ClaudeSubscription, id, 0).is_some(), "{id}");
        }
    }

    #[test]
    fn copilot_lists_its_data_page_once() {
        let mut asked = Vec::new();
        let models = read(ModelProvider::CopilotSubscription, |path| {
            asked.push(path.to_owned());
            json!({"data": [{"id": "claude-sonnet-4.5", "name": "Claude Sonnet 4.5"}, {"id": "x", "custom_model": true}]})
        });
        assert_eq!(
            models.map(|m| m.into_iter().map(|m| m.id).collect::<Vec<_>>()),
            Ok(vec!["copilot-subscription:claude-sonnet-4.5".to_owned()])
        );
        assert_eq!(asked, ["/models"]);
    }

    #[test]
    fn codex_reads_slugs_windows_and_levels() {
        let page = json!({"models": [{"slug": "gpt-5", "context_window": 272000,
            "supported_reasoning_levels": [{"effort": "low"}, {"effort": "high"}]}]});
        let models = read(ModelProvider::ChatGptSubscription, |_| page.clone()).unwrap_or_default();
        assert_eq!(models[0].id, "openai-subscription:gpt-5");
        assert_eq!(models[0].window, Some(272000));
        let caps = models[0].capabilities.clone().unwrap_or_default();
        assert!(caps.reasoning);
        assert_eq!(caps.reasoning_levels, ["low", "high"]);
    }
}
