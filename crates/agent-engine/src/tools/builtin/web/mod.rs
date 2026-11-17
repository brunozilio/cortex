mod attrs;
mod distill;
mod feed;
pub mod guarded;
mod html;
mod json;
mod links;
mod main_content;
mod page;
pub mod reach;

#[doc(hidden)]
pub use page::{Unreadable, read as read_page};
mod runs;
mod search;
mod tree;

use serde_json::Value;

use crate::model::http::{Http, reason};
use crate::model::{CancellationToken, ModelError, ToolSpec};
use crate::tools::{Outcome, Tool, Workspace, field, required};

const SHOWN: usize = 60_000;
const READ: usize = 5 << 20;
const ACCEPT: &str = "text/markdown, text/plain;q=0.9, text/html;q=0.8, application/json;q=0.8, */*;q=0.5";

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(WebFetch), Box::new(WebSearch)]
}

pub fn read_by_a_reader(ran: &crate::tools::after::Ran<'_>, page: &str) -> String {
    let (url, looking_for) = (field(ran.input, "url"), field(ran.input, "prompt"));
    let (models, agent) = (&ran.workspace.models, ran.agent);
    let reader = guarded::reader(models.small_model.as_deref(), agent.request, models.model.provider);
    let read = if ran.workspace.policy.web.small_pages_direct { guarded::digest } else { guarded::read_apart };
    read(
        agent.api,
        &reader,
        &agent.request.model,
        url.unwrap_or_default(),
        looking_for.unwrap_or_default(),
        page,
        agent.stop,
    )
}

#[derive(Debug)]
pub struct WebFetch;

impl Tool for WebFetch {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared(crate::tools::names::WEB_FETCH, &serde_json::Value::Null)
    }

    fn effects(&self, input: &Value, _workspace: &Workspace) -> Vec<crate::tools::Effect> {
        let host = field(input, "url").and_then(domain::permission_rules::site);
        vec![crate::tools::Effect::Net { host }]
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "url").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let url = match required(input, "url") {
            Ok(url) => reach::secure(url),
            Err(error) => return error,
        };
        let Some(web) = &workspace.services.network else {
            return match reach::refusal_by_name(&url) {
                Some(why) => Outcome::error(why),
                None => Outcome::error("The web is not available to this run."),
            };
        };
        let reading = workspace.policy.web;
        let mut headers = vec![("User-Agent".into(), brand::NAME.into())];
        if reading.formats {
            headers.push(("Accept".into(), ACCEPT.into()));
        }
        let cancel = workspace.stop.token();
        let Fetched { url, content_type, bytes } = match under(cancel, fetched(&*web.0, &url, &headers, cancel)) {
            Ok(fetched) => fetched,
            Err(why) => return Outcome::error(why),
        };
        match page::read(&String::from_utf8_lossy(&bytes), content_type.as_deref(), &url, reading) {
            Ok(text) => Outcome::ok(page::clipped(&text, SHOWN)),
            Err(page::Unreadable(why)) => Outcome::error(why),
        }
    }
}

struct Fetched {
    url: String,
    content_type: Option<String>,
    bytes: Vec<u8>,
}

async fn fetched(
    web: &dyn Http,
    url: &str,
    headers: &[(String, String)],
    cancel: &CancellationToken,
) -> Result<Fetched, String> {
    let reach::Reached { url, response } = reach::fetch(web, url, headers, cancel).await?;
    if !(200..300).contains(&response.status) {
        return Err(format!("{url} answered {}", response.status));
    }
    let content_type = response.header("content-type");
    let bytes =
        response.body.capped(cancel, READ).await.map_err(|error| format!("Could not read {url}: {}", reason(error)))?;
    Ok(Fetched { url, content_type, bytes })
}

pub fn under<T: Send>(
    cancel: &CancellationToken,
    work: impl std::future::Future<Output = Result<T, String>> + Send,
) -> Result<T, String> {
    let raced = async {
        tokio::select! {
            biased;
            () = cancel.cancelled() => Err(ModelError::Cancelled.to_string()),
            done = work => done,
        }
    };
    platform::runtime::block_on(raced).unwrap_or_else(|error| Err(error.to_string()))
}

#[derive(Debug)]
pub struct WebSearch;

impl Tool for WebSearch {
    fn spec(&self) -> ToolSpec {
        crate::tools::declared(crate::tools::WEB_SEARCH, &serde_json::Value::Null)
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<crate::tools::Effect> {
        vec![crate::tools::Effect::Net { host: None }]
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "query").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        match required(input, "query") {
            Ok(query) => search::run(query, workspace),
            Err(error) => error,
        }
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

    #[derive(Debug)]
    struct Stalling {
        before_headers: bool,
        held_open: std::sync::Mutex<Vec<tokio::sync::mpsc::Sender<crate::model::http::Chunk>>>,
    }

    impl Http for Stalling {
        fn one_hop<'a>(
            &'a self,
            _url: &'a str,
            _headers: &'a [(String, String)],
            _body: Option<&'a str>,
            _cancel: &'a CancellationToken,
        ) -> crate::model::BoxFuture<'a, Result<crate::model::http::Response, ModelError>> {
            if self.before_headers {
                return Box::pin(std::future::pending());
            }
            let (sender, chunks) = tokio::sync::mpsc::channel(1);
            if let Ok(mut held) = self.held_open.lock() {
                held.push(sender);
            }
            let body = crate::model::http::Body::chunks(chunks);
            Box::pin(std::future::ready(Ok(crate::model::http::Response { status: 200, headers: Vec::new(), body })))
        }

        fn addresses(&self, _host: &str) -> Result<Vec<std::net::IpAddr>, String> {
            Ok(vec![std::net::IpAddr::from([93, 184, 216, 34])])
        }

        fn post(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _body: &str,
        ) -> Result<crate::model::http::Response, String> {
            Err("not used".into())
        }
    }

    #[test]
    fn a_stalled_fetch_ends_as_soon_as_the_run_is_cancelled() {
        for before_headers in [true, false] {
            let mut workspace = Workspace::new("/tmp");
            let stalling = Stalling { before_headers, held_open: std::sync::Mutex::new(Vec::new()) };
            workspace.services.network = Some(Network(Arc::new(stalling)));
            let workspace = Arc::new(workspace);
            let cancelling = Arc::clone(&workspace);
            let (marked, cancelled_at) = crossbeam_channel::unbounded();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(30));
                let _ = marked.send(std::time::Instant::now());
                cancelling.stop.cancel();
            });
            let out = WebFetch.run(&json!({"url": "https://slow.dev/", "prompt": "p"}), &workspace);
            let ended = std::time::Instant::now();
            assert!(out.is_error && out.content.contains("cancelled"), "{before_headers}: {out:?}");
            let cancelled = cancelled_at.recv().expect("cancelled");
            assert!(ended - cancelled < std::time::Duration::from_secs(1), "{:?}", ended - cancelled);
        }
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
    fn web_search_is_offered_whatever_the_provider() {
        for (provider, model) in [("anthropic", "claude-opus-5-5"), ("kiro-subscription", "auto"), ("nobody", "m")] {
            assert!(WebSearch.offered(&on(provider, model)), "{provider}");
        }
        let called = WebSearch.run(&json!({"query": "rust"}), &on("nobody", "m"));
        assert!(called.is_error && called.content.contains("not available"), "{}", called.content);
    }
}
