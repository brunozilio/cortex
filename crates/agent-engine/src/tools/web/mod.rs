//! `WebFetch` and `WebSearch`: the web, through the engine's [`Http`] (so
//! tests script it). Both reach outside the computer, so they ask like a
//! command does; rules name a site as `WebFetch(domain:docs.rs)`.

mod html;
mod search;

use std::io::Read as _;
use std::sync::Arc;

use serde_json::{Value, json};

use super::{Access, Outcome, Tool, Workspace, clip, field, required};
use crate::model::ToolSpec;
use crate::model::http::Http;

/// What a fetched page gives the agent at most.
const SHOWN: usize = 60_000;
/// What is read of a page at most.
const READ: u64 = 5 << 20;

/// The run's way to the web.
#[derive(Clone)]
pub struct Web(pub Arc<dyn Http>);

impl std::fmt::Debug for Web {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Web")
    }
}

use domain::permission_rules::is_private;
pub use domain::permission_rules::site as host;

#[derive(Debug)]
pub struct WebFetch;

impl Tool for WebFetch {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "WebFetch".into(),
            description: "Fetch a web page (http upgraded to https) and read it as text, with what you are \
                          looking for in `prompt`. Long pages are cut; fetch a narrower page then."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The page's full URL." },
                    "prompt": { "type": "string", "description": "What to find on it." }
                },
                "required": ["url", "prompt"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Executes
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "url").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let url = match required(input, "url") {
            Ok(url) => url.replacen("http://", "https://", 1),
            Err(error) => return error,
        };
        let Some(site) = host(&url).filter(|_| url.starts_with("https://")) else {
            return Outcome::error(format!("Not a web address: {url}"));
        };
        if is_private(&site) {
            return Outcome::error(format!("{site} is this computer or its private network; WebFetch reads the web."));
        }
        let Some(web) = &workspace.web else { return Outcome::error("The web is not available to this run.") };
        let response = match web.0.get(&url, &[("User-Agent".into(), "Superfik".into())]) {
            Ok(response) => response,
            Err(error) => return Outcome::error(format!("Could not fetch {url}: {error}")),
        };
        if !(200..300).contains(&response.status) {
            return Outcome::error(format!("{url} answered {}", response.status));
        }
        let html = response.header("content-type").is_none_or(|kind| kind.contains("html"));
        let mut bytes = Vec::new();
        if let Err(error) = response.body.take(READ).read_to_end(&mut bytes) {
            return Outcome::error(format!("Could not read {url}: {error}"));
        }
        // The cut may split a character; what is whole stays.
        let body = String::from_utf8_lossy(&bytes).into_owned();
        let text = if html { html::text(&body) } else { body };
        let prompt = field(input, "prompt").unwrap_or_default();
        Outcome::ok(format!("Content of {url} (you were looking for: {prompt}):\n\n{}", clip(&text, SHOWN)))
    }
}

#[derive(Debug)]
pub struct WebSearch;

impl Tool for WebSearch {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "WebSearch".into(),
            description: "Search the web and get the top results: title, URL and a snippet each. Fetch a \
                          result with WebFetch to read it."
                .into(),
            schema: json!({
                "type": "object",
                "properties": { "query": { "type": "string", "description": "What to search for." } },
                "required": ["query"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Executes
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "query").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let query = match required(input, "query") {
            Ok(query) => query,
            Err(error) => return error,
        };
        let Some(web) = &workspace.web else { return Outcome::error("The web is not available to this run.") };
        let key = std::env::var("BRAVE_SEARCH_API_KEY").ok().filter(|key| !key.is_empty());
        match search::search(web.0.as_ref(), query, key.as_deref()) {
            Ok(results) if results.is_empty() => Outcome::ok(format!("No results for \"{query}\".")),
            Ok(results) => Outcome::ok(search::render(query, &results)),
            Err(error) => Outcome::error(format!("The search failed: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::http::ScriptedHttp;

    fn workspace(answers: Vec<(u16, String)>) -> Workspace {
        let mut workspace = Workspace::new("/tmp");
        workspace.web = Some(Web(Arc::new(ScriptedHttp::new(answers))));
        workspace
    }

    #[test]
    fn a_page_is_fetched_over_https_and_read_as_text() {
        let workspace = workspace(vec![(200, "<h1>Serde</h1><p>A framework.</p>".into())]);
        let out = WebFetch.run(&json!({"url": "http://serde.rs/", "prompt": "what it is"}), &workspace);
        assert!(!out.is_error, "{out:?}");
        assert!(out.content.starts_with("Content of https://serde.rs/ (you were looking for: what it is)"));
        assert!(out.content.ends_with("# Serde\nA framework."), "{}", out.content);
        let missing = workspace_answer(404);
        assert!(WebFetch.run(&json!({"url": "https://x.dev/a", "prompt": "p"}), &missing).is_error);
        assert!(WebFetch.run(&json!({"url": "file:///etc/passwd", "prompt": "p"}), &missing).is_error);
    }

    fn workspace_answer(status: u16) -> Workspace {
        workspace(vec![(status, String::new())])
    }

    #[test]
    fn this_computer_and_its_network_are_out_of_reach() {
        let workspace = workspace_answer(200);
        for url in ["https://localhost:8080/", "http://169.254.169.254/latest", "https://[::1]/", "https://10.0.0.2/"] {
            let out = WebFetch.run(&json!({"url": url, "prompt": "p"}), &workspace);
            assert!(out.is_error && out.content.contains("private network"), "{url}: {out:?}");
        }
    }
}
