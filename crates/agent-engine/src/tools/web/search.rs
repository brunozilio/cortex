//! Web search: Brave's API when `BRAVE_SEARCH_API_KEY` is set, else
//! DuckDuckGo's HTML results page (no key needed).

use std::io::Read as _;

use serde_json::Value;

use super::html::{decode, text};
use crate::model::http::Http;

/// Results returned at most.
const RESULTS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub fn search(http: &dyn Http, query: &str, brave_key: Option<&str>) -> Result<Vec<Found>, String> {
    let encoded = encode(query);
    match brave_key {
        Some(key) => {
            let url = format!("https://api.search.brave.com/res/v1/web/search?q={encoded}&count={RESULTS}");
            let headers = [
                ("Accept".to_owned(), "application/json".to_owned()),
                ("X-Subscription-Token".to_owned(), key.to_owned()),
            ];
            let body = fetched(http, &url, &headers)?;
            brave(&body)
        }
        None => {
            let url = format!("https://html.duckduckgo.com/html/?q={encoded}");
            let body = fetched(http, &url, &[("User-Agent".to_owned(), "Mozilla/5.0 (Superfik)".to_owned())])?;
            Ok(duckduckgo(&body))
        }
    }
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

/// The results of DuckDuckGo's HTML page: `result__a` links (their real
/// address in `uddg=`) and `result__snippet`s.
fn duckduckgo(page: &str) -> Vec<Found> {
    let mut found = Vec::new();
    for block in page.split("class=\"result__a\"").skip(1) {
        let Some(href) = block.split("href=\"").nth(1).and_then(|rest| rest.split('"').next()) else { continue };
        let url = match href.split("uddg=").nth(1) {
            Some(target) => decode_percent(target.split('&').next().unwrap_or_default()),
            None => decode(href),
        };
        let title = block.split_once('>').map(|(_, rest)| rest.split("</a>").next().unwrap_or_default()).unwrap_or("");
        let snippet = block
            .split("class=\"result__snippet\"")
            .nth(1)
            .and_then(|rest| rest.split_once('>'))
            .map(|(_, rest)| rest.split("</a>").next().unwrap_or_default())
            .unwrap_or("");
        if url.starts_with("http") {
            found.push(Found { title: text(title), url, snippet: text(snippet) });
        }
        if found.len() == RESULTS {
            break;
        }
    }
    found
}

/// The results as the agent reads them.
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

fn decode_percent(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'%' if at + 2 < bytes.len() => match u8::from_str_radix(&text[at + 1..at + 3], 16) {
                Ok(byte) => {
                    out.push(byte);
                    at += 3;
                }
                Err(_) => {
                    out.push(b'%');
                    at += 1;
                }
            },
            b'+' => {
                out.push(b' ');
                at += 1;
            }
            byte => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<div class="result"><h2><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fserde.rs%2F&amp;rut=x">Serde &amp; you</a></h2>
        <a class="result__snippet" href="x">A <b>framework</b> for serializing.</a></div>
        <div class="result"><a class="result__a" href="https://docs.rs/serde">serde - Rust</a>
        <a class="result__snippet" href="y">Docs.</a></div>"#;

    #[test]
    fn duckduckgo_results_are_read_with_their_real_addresses() {
        assert_eq!(
            duckduckgo(PAGE),
            [
                Found {
                    title: "Serde & you".into(),
                    url: "https://serde.rs/".into(),
                    snippet: "A framework for serializing.".into()
                },
                Found { title: "serde - Rust".into(), url: "https://docs.rs/serde".into(), snippet: "Docs.".into() },
            ]
        );
        assert_eq!(encode("rust serde & co"), "rust+serde+%26+co");
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
