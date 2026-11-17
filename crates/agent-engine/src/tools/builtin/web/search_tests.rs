use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde_json::json;

use super::super::WebSearch;
use crate::model::ModelError;
use crate::model::http::ScriptedHttp;
use crate::tools::{Network, WebSearcher};
use crate::tools::{Tool, Workspace};
use llm_providers::web_search::Hit;

struct Account(Result<Vec<Hit>, ModelError>);

impl crate::model::Searcher for Account {
    fn search(&self, _query: &str, _stop: &AtomicBool) -> Option<Result<Vec<Hit>, ModelError>> {
        Some(self.0.clone())
    }
}

fn rust_hit() -> Hit {
    Hit { title: "Rust 1.90".into(), url: "https://blog.rust-lang.org/1.90".into(), snippet: "Released today.".into() }
}

const RESULTS: &str = r#"<html><body>
<div class="result result--ad"><h2 class="result__title"><a class="result__a" href="//duckduckgo.com/y.js?ad=1">Buy Rust</a></h2></div>
<div class="result results_links web-result">
  <h2 class="result__title"><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fblog.rust-lang.org%2F1.90&amp;rut=abc">Rust <b>1.90</b> is out</a></h2>
  <a class="result__snippet" href="//duckduckgo.com/l/?uddg=x">Released <b>today</b> &amp; stable.</a>
</div>
<div class="result results_links web-result">
  <h2 class="result__title"><a rel="nofollow" class="result__a" href="https://doc.rust-lang.org/">The docs</a></h2>
</div>
</body></html>"#;

fn with(account: Option<Result<Vec<Hit>, ModelError>>, page: Option<&str>) -> (Workspace, Arc<ScriptedHttp>) {
    let http = Arc::new(ScriptedHttp::new(page.map(|page| (200, page.to_owned()))));
    let mut workspace = Workspace::new("/tmp");
    workspace.services.network = Some(Network(http.clone()));
    workspace.services.search = account.map(|found| WebSearcher(Arc::new(Account(found))));
    (workspace, http)
}

#[test]
fn the_account_s_own_search_answers_first() {
    let (workspace, http) = with(Some(Ok(vec![rust_hit()])), None);

    let out = WebSearch.run(&json!({"query": "rust 1.90"}), &workspace);

    assert!(!out.is_error, "{out:?}");
    assert!(out.content.contains("[Rust 1.90](https://blog.rust-lang.org/1.90)"), "{}", out.content);
    assert!(out.content.contains("Released today."), "{}", out.content);
    assert!(http.sent().is_empty(), "no page is fetched when the account searched");
}

#[test]
fn without_an_account_search_the_web_is_searched_over_http() {
    let (workspace, http) = with(None, Some(RESULTS));

    let out = WebSearch.run(&json!({"query": "rust 1.90"}), &workspace);

    assert!(!out.is_error, "{out:?}");
    assert!(out.content.contains("[Rust 1.90 is out](https://blog.rust-lang.org/1.90)"), "{}", out.content);
    assert!(out.content.contains("Released today & stable."), "{}", out.content);
    assert!(out.content.contains("[The docs](https://doc.rust-lang.org/)"), "{}", out.content);
    assert!(!out.content.contains("Buy Rust"), "ads are left out: {}", out.content);
    let asked = &http.sent()[0].url;
    assert!(asked.starts_with("https://html.duckduckgo.com/html/?q=rust+1.90"), "{asked}");
}

#[test]
fn an_account_search_that_fails_falls_back_to_http() {
    let (workspace, _) = with(Some(Err(ModelError::Unauthorized("HTTP 403".into()))), Some(RESULTS));

    let out = WebSearch.run(&json!({"query": "rust"}), &workspace);

    assert!(!out.is_error && out.content.contains("blog.rust-lang.org/1.90"), "{out:?}");
}

#[test]
fn a_search_with_no_results_says_so_and_both_failures_are_named() {
    let (workspace, _) = with(Some(Ok(Vec::new())), Some("<html><body>No results.</body></html>"));
    let out = WebSearch.run(&json!({"query": "zzqx"}), &workspace);
    assert!(!out.is_error && out.content.contains("No results"), "{out:?}");

    let (workspace, _) = with(Some(Err(ModelError::Unauthorized("HTTP 403".into()))), None);
    let out = WebSearch.run(&json!({"query": "rust"}), &workspace);
    assert!(out.is_error && out.content.contains("HTTP 403"), "{out:?}");
}
