use std::io::BufReader;
use std::sync::Arc;

use serde_json::json;

use super::super::WebFetch;
use crate::model::http::{Http, Response, ScriptedHttp};
use crate::tools::{Network, Outcome, Tool, Workspace};

fn fetched(http: &Arc<ScriptedHttp>, url: &str) -> Outcome {
    let mut workspace = Workspace::new("/tmp");
    workspace.services.network = Some(Network(http.clone()));
    WebFetch.run(&json!({ "url": url, "prompt": "p" }), &workspace)
}

fn asked(http: &ScriptedHttp) -> Vec<String> {
    http.sent().into_iter().map(|sent| sent.url).collect()
}

#[test]
fn a_redirect_to_another_public_page_is_followed_one_hop_at_a_time() {
    let http = Arc::new(ScriptedHttp::with_headers([
        ScriptedHttp::redirect("/docs/"),
        ScriptedHttp::redirect("http://other.dev/page"),
        ScriptedHttp::page("# Moved here"),
    ]));
    let out = fetched(&http, "https://docs.dev/old");
    assert_eq!(out.content, "# Moved here", "{out:?}");
    assert_eq!(asked(&http), ["https://docs.dev/old", "https://docs.dev/docs/", "https://other.dev/page"]);
}

#[test]
fn a_redirect_to_the_cloud_metadata_address_is_refused_before_it_is_asked() {
    let http = Arc::new(ScriptedHttp::with_headers([
        ScriptedHttp::redirect("http://169.254.169.254/latest/meta-data/"),
        ScriptedHttp::page("secret"),
    ]));
    let out = fetched(&http, "https://evil.dev/");
    assert!(out.is_error && out.content.contains("private network"), "{out:?}");
    assert_eq!(asked(&http), ["https://evil.dev/"], "the private address is never asked");
}

#[test]
fn a_redirect_to_this_computer_is_refused_before_it_is_asked() {
    for target in ["https://localhost:8080/admin", "https://127.0.0.1/", "https://[::1]/", "//10.0.0.2/"] {
        let http = Arc::new(ScriptedHttp::with_headers([ScriptedHttp::redirect(target), ScriptedHttp::page("x")]));
        let out = fetched(&http, "https://evil.dev/");
        assert!(out.is_error && out.content.contains("private network"), "{target}: {out:?}");
        assert_eq!(asked(&http).len(), 1, "{target}");
    }
}

#[test]
fn a_redirect_to_something_that_is_not_the_web_is_refused() {
    let http = Arc::new(ScriptedHttp::with_headers([ScriptedHttp::redirect("file:///etc/passwd")]));
    let out = fetched(&http, "https://evil.dev/");
    assert!(out.is_error && out.content.contains("Not a web address"), "{out:?}");
}

#[test]
fn a_public_name_that_resolves_to_this_computer_is_never_asked() {
    let loopback = "127.0.0.1".parse().expect("address");
    let http = Arc::new(ScriptedHttp::with_headers([ScriptedHttp::page("x")]).resolving("rebound.dev", loopback));
    let out = fetched(&http, "https://rebound.dev/");
    assert!(out.is_error && out.content.contains("private network"), "{out:?}");
    assert!(out.content.contains("127.0.0.1"), "says what it resolved to: {}", out.content);
    assert!(asked(&http).is_empty());

    let metadata = "169.254.169.254".parse().expect("address");
    let http = Arc::new(
        ScriptedHttp::with_headers([ScriptedHttp::redirect("https://rebound.dev/"), ScriptedHttp::page("x")])
            .resolving("rebound.dev", metadata),
    );
    let out = fetched(&http, "https://fine.dev/");
    assert!(out.is_error && out.content.contains("private network"), "after a redirect too: {out:?}");
    assert_eq!(asked(&http), ["https://fine.dev/"]);
}

#[test]
fn five_redirects_are_followed_and_a_sixth_is_not() {
    let hops = |count: usize| {
        let redirects = (0..count).map(|at| ScriptedHttp::redirect(&format!("https://hop.dev/{at}")));
        Arc::new(ScriptedHttp::with_headers(redirects.chain([ScriptedHttp::page("# End")]).collect::<Vec<_>>()))
    };
    let http = hops(5);
    let out = fetched(&http, "https://hop.dev/start");
    assert_eq!(out.content, "# End", "{out:?}");
    let http = hops(6);
    let out = fetched(&http, "https://hop.dev/start");
    assert!(out.is_error && out.content.contains("redirect too many"), "{out:?}");
    assert_eq!(asked(&http).len(), 6, "the seventh address is never asked");
}

#[derive(Debug)]
struct Endless;

impl Http for Endless {
    fn get(&self, _url: &str, _headers: &[(String, String)]) -> Result<Response, String> {
        let body = crate::model::http::Body::reader(BufReader::new(std::io::repeat(b'a')));
        Ok(Response { status: 200, headers: vec![("content-type".into(), "text/plain".into())], body })
    }

    fn addresses(&self, _host: &str) -> Result<Vec<std::net::IpAddr>, String> {
        Ok(vec!["93.184.216.34".parse().map_err(|_| "address")?])
    }

    fn post(&self, _url: &str, _headers: &[(String, String)], _body: &str) -> Result<Response, String> {
        Err("not used".into())
    }
}

#[test]
fn a_body_larger_than_the_cap_stops_being_read_at_the_cap() {
    let mut workspace = Workspace::new("/tmp");
    workspace.services.network = Some(Network(Arc::new(Endless)));
    let out = WebFetch.run(&json!({ "url": "https://big.dev/", "prompt": "p" }), &workspace);
    assert!(!out.is_error, "{}", &out.content[..out.content.len().min(200)]);
    assert!(out.content.len() < 100_000, "{}", out.content.len());
}
