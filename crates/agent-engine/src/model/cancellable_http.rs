use super::{ModelError, http::Response};
use std::{
    io::{self, BufRead, Read},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    time::Duration,
};
const POLL: Duration = Duration::from_millis(25);
pub const STREAM_TIMEOUT: Duration = Duration::from_secs(900);

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
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let (data_tx, data_rx) = mpsc::sync_channel(8);
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    let url = url.to_owned();
    let headers = headers.to_vec();
    let body = body.map(str::to_owned);
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let runtime =
                tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let client = reqwest::Client::builder().timeout(timeout).build().map_err(|e| e.to_string())?;
                let mut request = match body {
                    Some(body) => {
                        let mut request = client.post(url).body(body);
                        if !headers.iter().any(|(key, _)| key.eq_ignore_ascii_case("content-type")) {
                            request = request.header("content-type", "application/json");
                        }
                        request
                    }
                    None => client.get(url),
                };
                for (key, value) in headers {
                    request = request.header(key, value);
                }
                let waiting = request.send();
                tokio::pin!(waiting);
                let mut response = loop {
                    tokio::select! {
                        result=&mut waiting=>break result.map_err(|e| described(&e))?,
                        _=tokio::time::sleep(POLL)=>if signal.load(Ordering::Relaxed){return Ok(());}
                    }
                };
                let status = response.status().as_u16();
                let headers = response
                    .headers()
                    .iter()
                    .filter_map(|(k, v)| Some((k.as_str().to_owned(), v.to_str().ok()?.to_owned())))
                    .collect();
                if ready_tx.send(Ok((status, headers))).is_err() {
                    return Ok(());
                }
                loop {
                    let chunk = loop {
                        tokio::select! {
                            result=response.chunk()=>break result.map_err(|e| described(&e))?,
                            _=tokio::time::sleep(POLL)=>if signal.load(Ordering::Relaxed){return Ok(());}
                        }
                    };
                    let Some(chunk) = chunk else {
                        return Ok(());
                    };
                    if data_tx.send(Ok(chunk.to_vec())).is_err() {
                        return Ok(());
                    }
                }
            })
        })();
        if let Err(error) = result {
            let _ = ready_tx.try_send(Err(error.clone()));
            let _ = data_tx.send(Err(error));
        }
    });
    loop {
        if stop.load(Ordering::Relaxed) {
            cancelled.store(true, Ordering::Relaxed);
            return Err(ModelError::Cancelled);
        }
        match ready_rx.recv_timeout(POLL) {
            Ok(Ok((status, headers))) => {
                return Ok(Response {
                    status,
                    headers,
                    body: Box::new(Body { data: data_rx, buffer: Vec::new(), at: 0, cancelled }),
                });
            }
            Ok(Err(error)) => return Err(ModelError::Network(error)),
            Err(RecvTimeoutError::Timeout) => {}
            Err(_) => return Err(ModelError::Network("request ended before response headers".into())),
        }
    }
}
struct Body {
    data: Receiver<Result<Vec<u8>, String>>,
    buffer: Vec<u8>,
    at: usize,
    cancelled: Arc<AtomicBool>,
}
impl Drop for Body {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
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
            match self.data.recv_timeout(POLL) {
                Ok(Ok(bytes)) => {
                    self.buffer = bytes;
                    self.at = 0;
                }
                Ok(Err(error)) => return Err(io::Error::other(error)),
                Err(RecvTimeoutError::Timeout) => {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "waiting for stream"));
                }
                Err(_) => {
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
}
