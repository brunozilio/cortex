use super::{ModelError, http::Response};
use std::{
    io::{self, BufRead, Read},
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    runtime::{Handle, Runtime},
    sync::{mpsc, oneshot},
    task::AbortHandle,
};
const POLL: Duration = Duration::from_millis(25);
const CHUNKS_IN_FLIGHT: usize = 8;
pub const STREAM_TIMEOUT: Duration = Duration::from_secs(900);

struct Shared {
    runtime: Runtime,
    client: reqwest::Client,
}

static SHARED: OnceLock<Result<Shared, String>> = OnceLock::new();

fn shared() -> Result<&'static Shared, ModelError> {
    SHARED
        .get_or_init(|| {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("http")
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?;
            let client = {
                let _entered = runtime.enter();
                reqwest::Client::builder().build().map_err(|error| described(&error))?
            };
            Ok(Shared { runtime, client })
        })
        .as_ref()
        .map_err(|error| ModelError::Network(error.clone()))
}

pub(crate) fn described(error: &(dyn std::error::Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        let said = next.to_string();
        if !text.contains(&said) {
            text = format!("{text}: {said}");
        }
        cause = next.source();
    }
    text
}

pub fn post(url: &str, headers: &[(String, String)], body: &str, stop: &AtomicBool) -> Result<Response, ModelError> {
    send(url, headers, Some(body), stop, STREAM_TIMEOUT)
}

type Head = Result<(u16, Vec<(String, String)>), String>;

pub fn send(
    url: &str,
    headers: &[(String, String)],
    body: Option<&str>,
    stop: &AtomicBool,
    timeout: Duration,
) -> Result<Response, ModelError> {
    if stop.load(Ordering::Relaxed) {
        return Err(ModelError::Cancelled);
    }
    let shared = shared()?;
    let mut request = match body {
        Some(body) => {
            let request = shared.client.post(url).body(body.to_owned());
            if headers.iter().any(|(key, _)| key.eq_ignore_ascii_case("content-type")) {
                request
            } else {
                request.header("content-type", "application/json")
            }
        }
        None => shared.client.get(url),
    };
    for (key, value) in headers {
        request = request.header(key, value);
    }
    let request = request.timeout(timeout);
    let (head_tx, mut head_rx) = oneshot::channel::<Head>();
    let (data_tx, data_rx) = mpsc::channel::<Result<Vec<u8>, String>>(CHUNKS_IN_FLIGHT);
    let task = shared.runtime.spawn(async move {
        let mut response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                let _ = head_tx.send(Err(described(&error)));
                return;
            }
        };
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.as_str().to_owned(), v.to_str().ok()?.to_owned())))
            .collect();
        if head_tx.send(Ok((status, headers))).is_err() {
            return;
        }
        loop {
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => Ok(chunk.to_vec()),
                Ok(None) => return,
                Err(error) => Err(described(&error)),
            };
            let failed = chunk.is_err();
            if data_tx.send(chunk).await.is_err() || failed {
                return;
            }
        }
    });
    let task = Task(task.abort_handle());
    let handle = shared.runtime.handle();
    loop {
        if stop.load(Ordering::Relaxed) {
            return Err(ModelError::Cancelled);
        }
        match handle.block_on(async { tokio::time::timeout(POLL, &mut head_rx).await }) {
            Ok(Ok(Ok((status, headers)))) => {
                return Ok(Response {
                    status,
                    headers,
                    body: Box::new(Body {
                        data: data_rx,
                        handle: handle.clone(),
                        buffer: Vec::new(),
                        at: 0,
                        _running: task,
                    }),
                });
            }
            Ok(Ok(Err(error))) => return Err(ModelError::Network(error)),
            Ok(Err(_)) => return Err(ModelError::Network("request ended before response headers".into())),
            Err(_) => {}
        }
    }
}

struct Task(AbortHandle);

impl Drop for Task {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct Body {
    data: mpsc::Receiver<Result<Vec<u8>, String>>,
    handle: Handle,
    buffer: Vec<u8>,
    at: usize,
    _running: Task,
}
impl Read for Body {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let data = self.fill_buf()?;
        let count = data.len().min(out.len());
        out[..count].copy_from_slice(&data[..count]);
        self.consume(count);
        Ok(count)
    }
}
impl BufRead for Body {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.at == self.buffer.len() {
            let data = &mut self.data;
            let next = self.handle.block_on(async { tokio::time::timeout(POLL, data.recv()).await });
            match next {
                Ok(Some(Ok(bytes))) => {
                    self.buffer = bytes;
                    self.at = 0;
                }
                Ok(Some(Err(error))) => return Err(io::Error::other(error)),
                Err(_) => {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "waiting for stream"));
                }
                Ok(None) => {
                    self.buffer.clear();
                    self.at = 0;
                }
            }
        }
        Ok(&self.buffer[self.at..])
    }
    fn consume(&mut self, count: usize) {
        self.at = (self.at + count).min(self.buffer.len());
    }
}

#[cfg(test)]
mod tests {
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
}
