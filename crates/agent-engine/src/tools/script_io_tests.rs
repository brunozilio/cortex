use std::io::BufReader;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::FETCHED;
use crate::model::http::{Http, Response, ScriptedHttp};
use crate::model::{BoxFuture, CancellationToken, ModelError};
use crate::tools::script_tool::Origin;

fn fetch_over(web: &dyn Http, arguments: &Value, origin: Origin) -> Result<Value, String> {
    super::fetch_over(web, arguments, origin, &CancellationToken::new())
}

#[derive(Debug)]
struct Stalled;

impl Http for Stalled {
    fn one_hop<'a>(
        &'a self,
        _url: &'a str,
        _headers: &'a [(String, String)],
        _body: Option<&'a str>,
        _cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Response, ModelError>> {
        Box::pin(std::future::pending())
    }

    fn request_cancellable<'a>(
        &'a self,
        _url: &'a str,
        _headers: &'a [(String, String)],
        _body: Option<&'a str>,
        _cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Response, ModelError>> {
        Box::pin(std::future::pending())
    }

    fn addresses(&self, _host: &str) -> Result<Vec<std::net::IpAddr>, String> {
        Ok(vec![std::net::IpAddr::from([93, 184, 216, 34])])
    }

    fn post(&self, _url: &str, _headers: &[(String, String)], _body: &str) -> Result<Response, String> {
        Err("not used".into())
    }
}

#[test]
fn a_stalled_script_fetch_ends_as_soon_as_its_token_is_cancelled() {
    for origin in [Origin::Owner, Origin::Project] {
        let cancel = CancellationToken::new();
        let canceller = cancel.clone();
        let (marked, cancelled_at) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            let _ = marked.send(Instant::now());
            canceller.cancel();
        });
        let refused = super::fetch_over(&Stalled, &json!({ "url": "https://slow.dev/" }), origin, &cancel);
        let ended = Instant::now();
        assert_eq!(refused, Err("cancelled".to_owned()), "{origin:?}");
        let cancelled = cancelled_at.recv().expect("cancelled");
        assert!(ended - cancelled < Duration::from_secs(1), "{:?}", ended - cancelled);
    }
}

fn urls(http: &ScriptedHttp) -> Vec<String> {
    http.sent().into_iter().map(|sent| sent.url).collect()
}

#[test]
fn a_project_s_script_fetches_only_the_public_web() {
    for url in ["http://localhost:8080/", "https://127.0.0.1/", "https://169.254.169.254/latest", "https://10.0.0.2/"] {
        let http = ScriptedHttp::with_headers([ScriptedHttp::page("inside")]);
        let refused = fetch_over(&http, &json!({ "url": url }), Origin::Project).expect_err("refused");
        assert!(refused.contains("private network"), "{url}: {refused}");
        assert!(urls(&http).is_empty(), "{url}");
    }
    let http = ScriptedHttp::with_headers([ScriptedHttp::page("hello")]);
    let fetched = fetch_over(&http, &json!({ "url": "http://docs.dev/a" }), Origin::Project).expect("fetched");
    assert_eq!(fetched, json!({ "status": 200, "body": "hello" }));
    assert_eq!(urls(&http), ["https://docs.dev/a"], "asked over https");
}

#[test]
fn a_project_s_script_is_not_redirected_or_resolved_into_the_private_network() {
    let http =
        ScriptedHttp::with_headers([ScriptedHttp::redirect("http://169.254.169.254/"), ScriptedHttp::page("secret")]);
    let refused = fetch_over(&http, &json!({ "url": "https://evil.dev/" }), Origin::Project).expect_err("refused");
    assert!(refused.contains("private network"), "{refused}");
    assert_eq!(urls(&http), ["https://evil.dev/"]);

    let loopback = "127.0.0.1".parse().expect("address");
    let http = ScriptedHttp::with_headers([ScriptedHttp::page("x")]).resolving("rebound.dev", loopback);
    let posted = json!({ "url": "https://rebound.dev/", "body": "{}" });
    for arguments in [json!({ "url": "https://rebound.dev/" }), posted] {
        let refused = fetch_over(&http, &arguments, Origin::Project).expect_err("refused");
        assert!(refused.contains("private network"), "{refused}");
    }
    assert!(urls(&http).is_empty());
}

#[test]
fn a_project_s_script_may_post_to_the_public_web() {
    let http = ScriptedHttp::with_headers([ScriptedHttp::page("made")]);
    let arguments = json!({ "url": "https://api.dev/items", "body": "{\"a\":1}", "headers": { "x-key": "k" } });
    let fetched = fetch_over(&http, &arguments, Origin::Project).expect("fetched");
    assert_eq!(fetched["body"], "made");
    let sent = &http.sent()[0];
    assert_eq!((sent.url.as_str(), sent.body.as_str()), ("https://api.dev/items", "{\"a\":1}"));
    assert_eq!(sent.headers, [("x-key".to_owned(), "k".to_owned())]);
}

#[test]
fn the_user_s_own_script_still_reaches_this_computer() {
    let http = ScriptedHttp::with_headers([ScriptedHttp::page("local")]);
    let fetched = fetch_over(&http, &json!({ "url": "http://localhost:3000/health" }), Origin::Owner).expect("fetched");
    assert_eq!(fetched["body"], "local");
    assert_eq!(urls(&http), ["http://localhost:3000/health"]);
    let refused = fetch_over(&http, &json!({ "url": "file:///etc/passwd" }), Origin::Owner).expect_err("refused");
    assert!(refused.contains("not an http(s) address"), "{refused}");
}

#[derive(Debug)]
struct Endless;

impl Http for Endless {
    fn get(&self, _url: &str, _headers: &[(String, String)]) -> Result<Response, String> {
        Ok(Response {
            status: 200,
            headers: Vec::new(),
            body: crate::model::http::Body::reader(BufReader::new(std::io::repeat(b'a'))),
        })
    }

    fn addresses(&self, _host: &str) -> Result<Vec<std::net::IpAddr>, String> {
        Ok(vec!["93.184.216.34".parse().map_err(|_| "address")?])
    }

    fn post(&self, _url: &str, _headers: &[(String, String)], _body: &str) -> Result<Response, String> {
        Err("not used".into())
    }
}

#[test]
fn a_fetched_body_is_cut_at_the_cap_whoever_asks() {
    for origin in [Origin::Owner, Origin::Project] {
        let fetched = fetch_over(&Endless, &json!({ "url": "https://big.dev/" }), origin).expect("fetched");
        assert_eq!(fetched["body"].as_str().map(str::len), Some(FETCHED), "{origin:?}");
    }
}
