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

#[test]
fn a_request_that_cannot_connect_says_why() {
    let never = AtomicBool::new(false);
    let error = send("https://127.0.0.1:1/", &[], None, &never, Duration::from_secs(5)).expect_err("refused");
    let text = error.to_string();
    assert!(text.contains("error sending request") && text.contains("connect"), "{text}");
}

fn keep_alive_server(answers: usize) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = format!("http://{}/", listener.local_addr().expect("address"));
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = accepted.clone();
    std::thread::spawn(move || {
        let mut answered = 0;
        for stream in listener.incoming() {
            counted.fetch_add(1, Ordering::SeqCst);
            let Ok(mut stream) = stream else { return };
            let mut reader = io::BufReader::new(stream.try_clone().expect("clone"));
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
    let never = AtomicBool::new(false);
    for _ in 0..2 {
        let mut response = send(&address, &[], None, &never, Duration::from_secs(5)).expect("answer");
        let mut text = String::new();
        response.body.read_to_string(&mut text).expect("body");
        assert_eq!(text, "ok");
    }
    assert_eq!(accepted.load(Ordering::SeqCst), 1);
}

#[test]
fn a_stopped_request_is_cancelled_before_its_answer() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = format!("http://{}/", listener.local_addr().expect("address"));
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let stopper = stop.clone();
    std::thread::spawn(move || {
        let _held = listener.accept();
        stopper.store(true, Ordering::SeqCst);
        std::thread::park();
    });
    let error = send(&address, &[], None, &stop, Duration::from_secs(30)).expect_err("cancelled");
    assert!(matches!(error, ModelError::Cancelled), "{error}");
}
