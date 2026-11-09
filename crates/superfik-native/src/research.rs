//! Extracts web sources from `WebSearch` tool results. Deliberately lenient:
//! the result format is not a stable contract, so several shapes are tried.

use serde_json::Value;
use superfik_core::{ResearchItem, ResearchKind};

pub fn extract_sources(content: &Value, tool_use_result: Option<&Value>) -> Vec<ResearchItem> {
    let mut items = Vec::new();
    if let Some(structured) = tool_use_result {
        collect_structured(structured, &mut items);
    }
    let text = content_text(content);
    collect_links_json(&text, &mut items);
    collect_markdown_links(&text, &mut items);
    dedupe(items)
}

fn collect_structured(value: &Value, items: &mut Vec<ResearchItem>) {
    let Some(results) = value.get("results").and_then(Value::as_array) else { return };
    for result in results {
        if let Some(links) = result.get("content").and_then(Value::as_array) {
            items.extend(links.iter().filter_map(link_from_object));
        }
    }
}

fn content_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => {
            blocks.iter().filter_map(|block| block.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n")
        }
        _ => String::new(),
    }
}

fn collect_links_json(text: &str, items: &mut Vec<ResearchItem>) {
    let mut rest = text;
    while let Some(start) = rest.find("Links:") {
        let after = &rest[start + "Links:".len()..];
        let Some(open) = after.find('[') else { return };
        let mut stream = serde_json::Deserializer::from_str(&after[open..]).into_iter::<Value>();
        if let Some(Ok(Value::Array(links))) = stream.next() {
            items.extend(links.iter().filter_map(link_from_object));
        }
        rest = &after[open + 1..];
    }
}

fn collect_markdown_links(text: &str, items: &mut Vec<ResearchItem>) {
    let mut rest = text;
    while let Some(marker) = rest.find("](http") {
        let before = &rest[..marker];
        let after = &rest[marker + 2..];
        let title = before.rfind('[').map(|open| &before[open + 1..]);
        let url = after.find(')').map(|close| &after[..close]);
        if let (Some(title), Some(url)) = (title, url)
            && !title.trim().is_empty()
            && !url.contains(char::is_whitespace)
        {
            items.push(item(title.trim(), url));
        }
        rest = after;
    }
}

fn link_from_object(link: &Value) -> Option<ResearchItem> {
    let url = link.get("url").and_then(Value::as_str).filter(|url| !url.is_empty())?;
    let title = link.get("title").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty()).unwrap_or(url);
    Some(item(title, url))
}

fn item(title: &str, url: &str) -> ResearchItem {
    ResearchItem { title: title.to_owned(), url: Some(url.to_owned()), kind: ResearchKind::from_url(url) }
}

fn dedupe(items: Vec<ResearchItem>) -> Vec<ResearchItem> {
    let mut unique: Vec<ResearchItem> = Vec::with_capacity(items.len());
    for candidate in items {
        if !unique.iter().any(|kept| kept.url == candidate.url) {
            unique.push(candidate);
        }
    }
    unique
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn urls(items: &[ResearchItem]) -> Vec<&str> {
        items.iter().filter_map(|item| item.url.as_deref()).collect()
    }

    #[test]
    fn parses_links_json_in_text() {
        let text = "Web search results for query: \"sky\"\n\nLinks: [{\"title\":\"Night sky\",\"url\":\"https://a.dev/sky\"},{\"title\":\"Talk\",\"url\":\"https://youtube.com/watch?v=1\"}]\n\nSummary...";
        let items = extract_sources(&json!(text), None);
        assert_eq!(urls(&items), ["https://a.dev/sky", "https://youtube.com/watch?v=1"]);
        assert_eq!(items[0].title, "Night sky");
        assert_eq!(items[1].kind, ResearchKind::Video);
    }

    #[test]
    fn parses_text_block_arrays() {
        let content =
            json!([{"type": "text", "text": "Links: [{\"title\":\"Docs\",\"url\":\"https://x.dev/docs/a\"}]"}]);
        let items = extract_sources(&content, None);
        assert_eq!(items[0].kind, ResearchKind::Document);
    }

    #[test]
    fn parses_structured_tool_use_result_and_dedupes() {
        let structured = json!({
            "query": "sky",
            "results": [{"tool_use_id": "s", "content": [{"title": "A", "url": "https://a.dev"}]}, "text summary"]
        });
        let content = json!("Links: [{\"title\":\"A again\",\"url\":\"https://a.dev\"}]");
        let items = extract_sources(&content, Some(&structured));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "A");
    }

    #[test]
    fn parses_markdown_links() {
        let items = extract_sources(&json!("See [Thread](https://reddit.com/r/x) and [](https://empty.dev)."), None);
        assert_eq!(urls(&items), ["https://reddit.com/r/x"]);
        assert_eq!(items[0].kind, ResearchKind::Discussion);
    }

    #[test]
    fn tolerates_garbage() {
        assert!(extract_sources(&json!("Links: [not json"), None).is_empty());
        assert!(extract_sources(&json!(42), Some(&json!({"results": "x"}))).is_empty());
        let untitled = extract_sources(&json!("Links: [{\"url\":\"https://u.dev\"}]"), None);
        assert_eq!(untitled[0].title, "https://u.dev");
    }
}
