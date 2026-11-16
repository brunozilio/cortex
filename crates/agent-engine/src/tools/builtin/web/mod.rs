mod attrs;
mod distill;
mod feed;
pub mod guarded;
mod html;
mod json;
mod links;
mod main_content;
mod page;
mod runs;
mod tree;

use std::io::Read as _;

use serde_json::Value;

use crate::model::ToolSpec;
use crate::tools::{Access, Outcome, Tool, Workspace, field, required};

const SHOWN: usize = 60_000;
const READ: u64 = 5 << 20;
const ACCEPT: &str = "text/markdown, text/plain;q=0.9, text/html;q=0.8, application/json;q=0.8, */*;q=0.5";

use domain::permission_rules::is_private;
use domain::permission_rules::site as host;

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(WebFetch), Box::new(WebSearch)]
}

#[derive(Debug)]
pub struct WebFetch;

impl Tool for WebFetch {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared("WebFetch", &serde_json::Value::Null)
    }

    fn access(&self) -> Access {
        crate::tools::declared_access("WebFetch")
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
        let Some(web) = &workspace.services.network else {
            return Outcome::error("The web is not available to this run.");
        };
        let reading = workspace.policy.web;
        let mut headers = vec![("User-Agent".into(), brand::NAME.into())];
        if reading.formats {
            headers.push(("Accept".into(), ACCEPT.into()));
        }
        let response = match web.0.get(&url, &headers) {
            Ok(response) => response,
            Err(error) => return Outcome::error(format!("Could not fetch {url}: {error}")),
        };
        if !(200..300).contains(&response.status) {
            return Outcome::error(format!("{url} answered {}", response.status));
        }
        let content_type = response.header("content-type");
        let mut bytes = Vec::new();
        if let Err(error) = response.body.take(READ).read_to_end(&mut bytes) {
            return Outcome::error(format!("Could not read {url}: {error}"));
        }
        match page::read(&String::from_utf8_lossy(&bytes), content_type.as_deref(), &url, reading) {
            Ok(text) => Outcome::ok(page::clipped(&text, SHOWN)),
            Err(page::Unreadable(why)) => Outcome::error(why),
        }
    }
}

#[derive(Debug)]
pub struct WebSearch;

impl Tool for WebSearch {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared(crate::tools::WEB_SEARCH, &serde_json::Value::Null)
    }

    fn offered(&self, workspace: &Workspace) -> bool {
        let model = &workspace.models.model;
        crate::providers::find(model.provider).is_some_and(|provider| provider.web_search(&model.id).is_some())
    }

    fn access(&self) -> Access {
        crate::tools::declared_access(crate::tools::WEB_SEARCH)
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "query").unwrap_or_default().to_owned()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error("Web search runs inside the model provider, and this model's provider has none.")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::model::http::ScriptedHttp;
    use crate::tools::Network;
    use std::sync::Arc;

    fn workspace(answers: Vec<(u16, String)>) -> Workspace {
        let mut workspace = Workspace::new("/tmp");
        workspace.services.network = Some(Network(Arc::new(ScriptedHttp::new(answers))));
        workspace
    }

    #[test]
    fn a_page_is_fetched_over_https_and_read_as_text() {
        let workspace = workspace(vec![(200, "<h1>Serde</h1><p>A framework.</p>".into())]);
        let out = WebFetch.run(&json!({"url": "http://serde.rs/", "prompt": "what it is"}), &workspace);
        assert!(!out.is_error, "{out:?}");
        assert_eq!(out.content, "# Serde\n\nA framework.", "the loop hands the page to a separate reader");
        let missing = workspace_answer(404);
        assert!(WebFetch.run(&json!({"url": "https://x.dev/a", "prompt": "p"}), &missing).is_error);
        assert!(WebFetch.run(&json!({"url": "file:///etc/passwd", "prompt": "p"}), &missing).is_error);
    }

    #[test]
    fn markdown_is_asked_for_before_html() {
        let http = Arc::new(ScriptedHttp::new([(200, "# Already markdown".to_owned())]));
        let mut workspace = Workspace::new("/tmp");
        workspace.services.network = Some(Network(http.clone()));
        let out = WebFetch.run(&json!({"url": "https://docs.dev/a.md", "prompt": "p"}), &workspace);
        assert_eq!(out.content, "# Already markdown");
        let accept = http.sent()[0].headers.iter().find(|(key, _)| key == "Accept").map(|(_, value)| value.clone());
        assert!(accept.is_some_and(|accept| accept.starts_with("text/markdown")));

        let http = Arc::new(ScriptedHttp::new([(200, "<p>Plain</p>".to_owned())]));
        workspace.services.network = Some(Network(http.clone()));
        workspace.policy.web.formats = false;
        WebFetch.run(&json!({"url": "https://docs.dev/a", "prompt": "p"}), &workspace);
        assert!(http.sent()[0].headers.iter().all(|(key, _)| key != "Accept"), "the flag turns it off");
    }

    #[test]
    fn a_page_with_no_text_is_an_error_not_an_answer() {
        let shell =
            format!("<html><head>{}</head><body><div id='root'></div></body></html>", "<script>x</script>".repeat(800));
        let out = WebFetch.run(&json!({"url": "https://app.dev/", "prompt": "p"}), &workspace(vec![(200, shell)]));
        assert!(out.is_error && out.content.contains("JavaScript"), "{out:?}");
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

#[cfg(test)]
mod offered_tests {
    use serde_json::json;

    use super::*;

    fn on(provider: &'static str, model: &str) -> Workspace {
        let mut workspace = Workspace::new("/tmp");
        workspace.models.model = domain::ModelOption {
            provider: domain::ModelProvider::new(provider),
            ..domain::ModelOption::new(model, model)
        };
        workspace
    }

    #[test]
    fn web_search_is_offered_only_where_the_provider_searches_itself() {
        assert!(WebSearch.offered(&on("anthropic", "claude-opus-5-5")));
        assert!(WebSearch.offered(&on("openai", "gpt-5.5")));
        assert!(!WebSearch.offered(&on("kiro", "claude-sonnet-4-6")));
        assert!(!WebSearch.offered(&on("nobody", "m")));
        let called = WebSearch.run(&json!({"query": "rust"}), &on("kiro", "m"));
        assert!(called.is_error && called.content.contains("has none"), "{}", called.content);
    }
}
