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
#[path = "cancellable_http_tests.rs"]
mod tests;
