#![cfg(feature = "subscription-providers")]

use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, unbounded};

use domain::ModelProvider;
use domain::provider::ProviderKey;
use serde_json::json;

use super::ProviderClient;
use crate::model::http::{Http, NetHttp, Response};
use crate::model::{BlockingTurn, BoxFuture, CancellationToken, ModelError};
use crate::providers::testing::request;

const GROK: ModelProvider = ModelProvider::new("grok-subscription");

#[derive(Debug)]
struct Silent {
    address: String,
}

impl Http for Silent {
    fn post_streaming<'a>(
        &'a self,
        _url: &'a str,
        headers: &'a [(String, String)],
        body: &'a str,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Response, ModelError>> {
        NetHttp.post_cancellable(&self.address, headers, body, cancel)
    }

    fn post_cancellable<'a>(
        &'a self,
        _url: &'a str,
        headers: &'a [(String, String)],
        body: &'a str,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Response, ModelError>> {
        NetHttp.post_cancellable(&self.address, headers, body, cancel)
    }

    fn post(&self, _url: &str, headers: &[(String, String)], body: &str) -> Result<Response, String> {
        NetHttp.post(&self.address, headers, body)
    }
}

fn silent_server() -> (String, Receiver<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = format!("http://{}/", listener.local_addr().expect("address"));
    let (accepted, heard) = unbounded();
    std::thread::spawn(move || {
        let held = listener.accept();
        let _ = accepted.send(());
        std::thread::sleep(Duration::from_secs(30));
        drop(held);
    });
    (address, heard)
}

#[test]
fn a_refresh_against_a_silent_server_ends_as_soon_as_the_call_is_cancelled() {
    let (address, heard) = silent_server();
    let expired = json!({"auth": {"access_token": "access-1", "refresh_token": "refresh-1", "expires_at": "2000-01-01T00:00:00Z"}});
    let client =
        ProviderClient::new(GROK, Arc::new(Silent { address }), Arc::new(ProviderKey::new(expired.to_string())));
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    let (marked, cancelled_at) = unbounded();
    std::thread::spawn(move || {
        if heard.recv_timeout(Duration::from_secs(30)).is_ok() {
            std::thread::sleep(Duration::from_millis(20));
            let _ = marked.send(Instant::now());
            canceller.cancel();
        }
    });
    let turned = client.turn_blocking(&request("grok-subscription:grok-4.6"), &mut |_| {}, &cancel);
    let ended = Instant::now();
    assert_eq!(turned.map(|_| ()), Err(ModelError::Cancelled));
    let cancelled = cancelled_at.recv_timeout(Duration::from_secs(1)).expect("the refresh reached the server");
    assert!(ended - cancelled < Duration::from_secs(1), "{:?}", ended - cancelled);
}
