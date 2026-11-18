use super::*;

#[derive(Debug)]
struct Layer(&'static str, Option<Box<Layer>>);

impl std::fmt::Display for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Layer {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.1.as_deref().map(|layer| layer as &(dyn std::error::Error + 'static))
    }
}

#[test]
fn an_error_names_what_caused_it_once() {
    let error = Layer(
        "error sending request",
        Some(Box::new(Layer(
            "client error (Connect)",
            Some(Box::new(Layer("invalid peer certificate: UnknownIssuer", None))),
        ))),
    );
    assert_eq!(
        described(&error),
        "error sending request: client error (Connect): invalid peer certificate: UnknownIssuer"
    );
    let repeated = Layer("tcp connect error: refused", Some(Box::new(Layer("refused", None))));
    assert_eq!(described(&repeated), "tcp connect error: refused");
}

fn got(address: &str) -> Result<Response, ModelError> {
    let ask = Ask { total: Some(Duration::from_secs(5)), ..Ask::new(address, &[], None) };
    platform::runtime::block_on(fetch(ask, &CancellationToken::new())).expect("runtime")
}

#[test]
fn a_request_that_cannot_connect_says_why() {
    let error = got("https://127.0.0.1:1/").expect_err("refused");
    let text = error.to_string();
    assert!(text.contains("error sending request") && text.contains("connect"), "{text}");
}

fn keep_alive_server(answers: usize) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::{BufRead, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = format!("http://{}/", listener.local_addr().expect("address"));
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = accepted.clone();
    std::thread::spawn(move || {
        let mut answered = 0;
        for stream in listener.incoming() {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let Ok(mut stream) = stream else { return };
            let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone"));
            while answered < answers {
                let mut line = String::new();
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if line == "\r\n" {
                        break;
                    }
                }
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok");
                answered += 1;
            }
        }
    });
    (address, accepted)
}

#[test]
fn requests_share_one_pooled_connection() {
    let (address, accepted) = keep_alive_server(2);
    let never = CancellationToken::new();
    for _ in 0..2 {
        let response = got(&address).expect("answer");
        assert_eq!(platform::runtime::block_on(response.text(&never)).expect("runtime").expect("body"), "ok");
    }
    assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 1);
}

fn silent_server() -> (String, crossbeam_channel::Receiver<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = format!("http://{}/", listener.local_addr().expect("address"));
    let (accepted, heard) = crossbeam_channel::unbounded();
    std::thread::spawn(move || {
        let held = listener.accept();
        let _ = accepted.send(());
        std::thread::sleep(Duration::from_secs(30));
        drop(held);
    });
    (address, heard)
}

#[test]
fn a_cancelled_token_ends_a_request_waiting_for_its_answer_at_once() {
    let (address, heard) = silent_server();
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    let cancelled_at = std::sync::Arc::new(std::sync::Mutex::new(None));
    let marked = std::sync::Arc::clone(&cancelled_at);
    std::thread::spawn(move || {
        let _ = heard.recv();
        std::thread::sleep(Duration::from_millis(20));
        *marked.lock().expect("mark") = Some(std::time::Instant::now());
        canceller.cancel();
    });
    let fetched = platform::runtime::block_on(fetch(Ask::new(&address, &[], Some("{}")), &cancel)).expect("runtime");
    let ended = std::time::Instant::now();
    assert!(matches!(fetched, Err(ModelError::Cancelled)), "{fetched:?}");
    let cancelled = cancelled_at.lock().expect("mark").expect("the server heard the request");
    assert!(ended - cancelled < Duration::from_secs(1), "{:?}", ended - cancelled);
}

#[test]
fn every_cancellable_request_ends_when_its_token_is_cancelled() {
    use super::super::http::{Http, NetHttp};
    for asked in 0..4 {
        let (address, heard) = silent_server();
        let cancel = CancellationToken::new();
        let canceller = cancel.clone();
        let (marked, cancelled_at) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            let _ = heard.recv();
            std::thread::sleep(Duration::from_millis(20));
            let _ = marked.send(std::time::Instant::now());
            canceller.cancel();
        });
        let request = match asked {
            0 => NetHttp.post_cancellable(&address, &[], "{}", &cancel),
            1 => NetHttp.get_stream(&address, &[], &cancel),
            2 => NetHttp.one_hop(&address, &[], None, &cancel),
            _ => NetHttp.request_cancellable(&address, &[], Some("{}"), &cancel),
        };
        let fetched = platform::runtime::block_on(request).expect("runtime");
        let ended = std::time::Instant::now();
        assert!(matches!(fetched, Err(ModelError::Cancelled)), "{fetched:?}");
        let cancelled = marked_at(&cancelled_at);
        assert!(ended - cancelled < Duration::from_secs(1), "{:?}", ended - cancelled);
    }
}

fn marked_at(cancelled_at: &crossbeam_channel::Receiver<std::time::Instant>) -> std::time::Instant {
    cancelled_at.recv_timeout(Duration::from_secs(30)).expect("the server heard the request")
}

#[test]
fn a_server_that_never_answers_runs_into_the_idle_deadline() {
    let (address, _heard) = silent_server();
    let ask = Ask { idle: Some(Duration::from_millis(100)), ..Ask::new(&address, &[], Some("{}")) };
    let fetched = platform::runtime::block_on(fetch(ask, &CancellationToken::new())).expect("runtime");
    assert!(matches!(fetched, Err(ModelError::Stalled(_))), "{fetched:?}");
}

#[test]
fn only_a_plain_http_request_without_a_proxy_skips_loading_the_system_s_root_certificates() {
    assert_eq!(roots_for("http://127.0.0.1:4318/v1/traces", false), Roots::Skipped);
    assert_eq!(roots_for("HTTP://localhost/", false), Roots::Skipped);
    assert_eq!(roots_for("https://api.example/", false), Roots::Loaded);
    assert_eq!(roots_for("http://127.0.0.1:4318/", true), Roots::Loaded, "the proxy may itself be HTTPS");
    assert_eq!(roots_for("httpx://odd", false), Roots::Loaded);
}

fn answering(answers: Vec<String>) -> String {
    use std::io::{BufRead, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = format!("http://{}", listener.local_addr().expect("address"));
    std::thread::spawn(move || {
        for (stream, answer) in listener.incoming().zip(answers) {
            let Ok(mut stream) = stream else { return };
            let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone"));
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                line.clear();
            }
            let _ = stream.write_all(answer.as_bytes());
        }
    });
    address
}

fn moved_to(location: &str) -> String {
    format!("HTTP/1.1 302 Found\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
}

#[test]
fn a_plain_request_follows_plain_redirects_and_hands_one_to_https_to_the_client_that_has_the_roots() {
    let ok = "HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok".to_owned();
    let plain = answering(vec![moved_to("/next"), ok]);
    let never = CancellationToken::new();
    let followed = got(&format!("{plain}/first")).expect("followed");
    assert_eq!(platform::runtime::block_on(followed.text(&never)).expect("runtime").expect("body"), "ok");

    let secure = answering(vec![moved_to("https://127.0.0.1:1/"), moved_to("https://127.0.0.1:1/")]);
    let handed = got(&format!("{secure}/first")).expect_err("the https target refuses");
    assert!(handed.to_string().contains("connect"), "the https hop was taken, not returned as a 302: {handed}");
}
