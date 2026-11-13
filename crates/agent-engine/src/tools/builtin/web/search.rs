use std::io::Read as _;

use serde_json::Value;

use super::html::text;
use crate::model::http::Http;

const RESULTS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub const NO_SEARCH: &str = "no search service is set up: set BRAVE_SEARCH_API_KEY to a Brave Search API key";

pub fn search(http: &dyn Http, query: &str, brave_key: Option<&str>) -> Result<Vec<Found>, String> {
    let key = brave_key.ok_or(NO_SEARCH)?;
    let url = format!("https://api.search.brave.com/res/v1/web/search?q={}&count={RESULTS}", encode(query));
    let headers =
        [("Accept".to_owned(), "application/json".to_owned()), ("X-Subscription-Token".to_owned(), key.to_owned())];
    brave(&fetched(http, &url, &headers)?)
}

fn fetched(http: &dyn Http, url: &str, headers: &[(String, String)]) -> Result<String, String> {
    let response = http.get(url, headers)?;
    if !(200..300).contains(&response.status) {
        return Err(format!("the search answered {}", response.status));
    }
    let mut body = String::new();
    response.body.take(2 << 20).read_to_string(&mut body).map_err(|e| e.to_string())?;
    Ok(body)
}

fn brave(body: &str) -> Result<Vec<Found>, String> {
    let value: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    Ok(value["web"]["results"]
        .as_array()
        .into_iter()
        .flatten()
        .take(RESULTS)
        .map(|result| Found {
            title: text(result["title"].as_str().unwrap_or_default()),
            url: result["url"].as_str().unwrap_or_default().to_owned(),
            snippet: text(result["description"].as_str().unwrap_or_default()),
        })
        .collect())
}

pub fn render(query: &str, results: &[Found]) -> String {
    let mut out = format!("Results for \"{query}\":");
    for (at, result) in results.iter().enumerate() {
        out.push_str(&format!("\n\n{}. {}\n   {}\n   {}", at + 1, result.title, result.url, result.snippet));
    }
    out
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".to_owned(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_is_sent_url_encoded() {
        assert_eq!(encode("rust serde & co"), "rust+serde+%26+co");
    }

    #[test]
    fn without_a_search_service_nothing_is_fetched() {
        let http = crate::model::http::ScriptedHttp::new(Vec::new());
        assert_eq!(search(&http, "serde", None), Err(NO_SEARCH.to_owned()));
    }

    #[test]
    fn brave_results_are_read_from_its_json() {
        let body = r#"{"web":{"results":[{"title":"Serde","url":"https://serde.rs","description":"A <strong>framework</strong>"}]}}"#;
        assert_eq!(
            brave(body),
            Ok(vec![Found { title: "Serde".into(), url: "https://serde.rs".into(), snippet: "A framework".into() }])
        );
    }
}
