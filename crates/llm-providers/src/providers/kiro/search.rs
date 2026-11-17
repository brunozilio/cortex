use serde_json::{Value, json};

use llm_providers::web_search::Hit;
use llm_providers::{Call, Config, ProviderError};

pub const SEARCH: &str = "AmazonCodeWhispererStreamingService.InvokeMCP";

pub fn call(config: &Config, query: &str) -> Call {
    let body = json!({
        "jsonrpc": "2.0",
        "id": "1",
        "method": "tools/call",
        "params": {"name": "web_search", "arguments": {"query": query}},
    });
    Call {
        url: format!("{}/", super::runtime(config.account())),
        headers: super::headers(config.access_token(), SEARCH),
        body: Some(body.to_string()),
    }
}

pub fn hits(value: &Value) -> Result<Vec<Hit>, ProviderError> {
    if let Some(error) = value.get("error").filter(|error| !error.is_null()) {
        let why = error["message"].as_str().map_or_else(|| error.to_string(), str::to_owned);
        return Err(ProviderError::Invalid(format!("Kiro's web search refused: {why}")));
    }
    let result = &value["result"];
    let text: String =
        result["content"].as_array().into_iter().flatten().filter_map(|part| part["text"].as_str()).collect();
    if result["isError"].as_bool() == Some(true) {
        return Err(ProviderError::Invalid(format!("Kiro's web search failed: {text}")));
    }
    let found: Value = serde_json::from_str(&text)
        .map_err(|_| ProviderError::Invalid("Kiro's web search answered without results".into()))?;
    if let Some(why) = found["error"].as_str() {
        return Err(ProviderError::Invalid(format!("Kiro's web search failed: {why}")));
    }
    Ok(found["results"].as_array().into_iter().flatten().filter_map(hit).collect())
}

fn hit(item: &Value) -> Option<Hit> {
    let url = item["url"].as_str().filter(|url| !url.is_empty())?;
    Some(Hit {
        title: item["title"].as_str().unwrap_or(url).to_owned(),
        url: url.to_owned(),
        snippet: item["snippet"].as_str().unwrap_or_default().to_owned(),
    })
}
