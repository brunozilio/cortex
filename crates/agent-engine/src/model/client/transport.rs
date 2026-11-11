use super::super::{
    KeySource, ModelError,
    http::{Http, Response},
};
use super::{Config, oauth};
use llm_providers::{Provider, catalog, request};
use serde_json::Value;
use std::io::Read;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub(super) struct Transport {
    pub provider: &'static dyn Provider,
    pub http: Arc<dyn Http>,
    pub keys: Arc<dyn KeySource>,
    pub lock: Mutex<()>,
    pub cached: Mutex<Option<(String, Config)>>,
    pub model: Option<domain::ModelOption>,
}

impl Transport {
    pub(super) fn config(&self, force: bool) -> Result<Config, ModelError> {
        self.config_stopped(force, &AtomicBool::new(false))
    }

    fn config_stopped(&self, force: bool, stop: &AtomicBool) -> Result<Config, ModelError> {
        let key = self.keys.key()?;
        let raw = key.secret();
        let mut cached = self.cached.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut config = match cached.as_ref() {
            Some((old, config)) if old == raw => config.clone(),
            _ => Config::parse(self.provider, raw)?,
        };
        if config.needs_refresh(self.provider, force)
            && let Some(previous) = config.auth.as_ref()
        {
            config.auth = Some(oauth::refresh(
                &super::super::http::Stopped { http: self.http.as_ref(), stop },
                self.provider,
                previous,
            )?);
            self.keys.replace(&serde_json::to_string(&config).map_err(|e| ModelError::Provider(e.to_string()))?)?;
        }
        *cached = Some((raw.to_owned(), config.clone()));
        Ok(config)
    }

    pub(super) fn models(&self) -> Result<Vec<domain::ModelOption>, ModelError> {
        catalog::collect(self.provider, |after| {
            let _guard = self.lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let call = self.provider.catalog(&self.config(false)?, after);
            match &call.body {
                Some(body) => self.http.post(&call.url, &call.headers, body),
                None => self.http.get(&call.url, &call.headers),
            }
            .map_err(ModelError::Network)
            .and_then(json_response)
        })
    }
}

impl Http for Transport {
    fn post_stopped(
        &self,
        _url: &str,
        _headers: &[(String, String)],
        body: &str,
        stop: &AtomicBool,
    ) -> Result<Response, ModelError> {
        self.send_stopped(body, stop)
    }
    fn post(&self, _url: &str, _headers: &[(String, String)], body: &str) -> Result<Response, String> {
        self.send(body).map_err(|error| error.to_string())
    }
    fn post_model(&self, _url: &str, _headers: &[(String, String)], body: &str) -> Result<Response, ModelError> {
        self.send(body)
    }
}

impl Transport {
    fn send(&self, body: &str) -> Result<Response, ModelError> {
        self.send_stopped(body, &AtomicBool::new(false))
    }

    fn send_stopped(&self, body: &str, stop: &AtomicBool) -> Result<Response, ModelError> {
        let _guard = loop {
            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(ModelError::Cancelled);
            }
            match self.lock.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::WouldBlock) => std::thread::sleep(std::time::Duration::from_millis(25)),
                Err(std::sync::TryLockError::Poisoned(error)) => break error.into_inner(),
            }
        };
        let mut body: Value = serde_json::from_str(body).map_err(|e| ModelError::Provider(e.to_string()))?;
        request::shape(self.provider, self.model.as_ref(), &mut body)?;
        let turn_headers = self.provider.turn_headers(self.model.as_ref(), &body);
        let shaped = body;
        let bound = |config: &Config| {
            let mut body = shaped.clone();
            self.provider.bind(config, &mut body);
            body.to_string()
        };
        let path = self.provider.path(self.model.as_ref());
        let headers = |config: &Config| {
            let mut headers = config.headers(self.provider);
            headers.extend(turn_headers.iter().cloned());
            headers
        };
        let mut config = self.config_stopped(false, stop)?;
        let mut response = self.http.post_stopped(
            &format!("{}{path}", config.base(self.provider)),
            &headers(&config),
            &bound(&config),
            stop,
        )?;
        if self.provider.retries_after_refresh(response.status) {
            drop(response);
            config = self.config_stopped(true, stop)?;
            response = self.http.post_stopped(
                &format!("{}{path}", config.base(self.provider)),
                &headers(&config),
                &bound(&config),
                stop,
            )?;
        }
        let limit = if response.status < 300 { request::STREAM_LIMIT } else { request::ERROR_LIMIT };
        response.body = Box::new(Bounded { inner: response.body, remaining: limit });
        Ok(response)
    }
}

struct Bounded {
    inner: Box<dyn std::io::BufRead + Send>,
    remaining: usize,
}
impl std::io::Read for Bounded {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let buffer = std::io::BufRead::fill_buf(self)?;
        let count = buffer.len().min(output.len());
        output[..count].copy_from_slice(&buffer[..count]);
        std::io::BufRead::consume(self, count);
        Ok(count)
    }
}
impl std::io::BufRead for Bounded {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let buffer = self.inner.fill_buf()?;
        if self.remaining == 0 && !buffer.is_empty() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "provider stream exceeds size limit"));
        }
        Ok(&buffer[..buffer.len().min(self.remaining)])
    }
    fn consume(&mut self, amount: usize) {
        let count = amount.min(self.remaining);
        self.inner.consume(count);
        self.remaining -= count;
    }
}

pub(super) fn read(mut response: Response, limit: u64) -> Result<String, ModelError> {
    let mut text = String::new();
    response.body.by_ref().take(limit + 1).read_to_string(&mut text).map_err(|e| ModelError::Network(e.to_string()))?;
    if text.len() as u64 > limit {
        return Err(ModelError::Provider("provider response exceeds size limit".into()));
    }
    Ok(text)
}

pub(super) fn status(status: u16) -> Result<(), ModelError> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(ModelError::Unauthorized(format!("provider returned HTTP {status}"))),
        429 | 500..=599 => Err(ModelError::Overloaded(format!("provider returned HTTP {status}"))),
        _ => Err(ModelError::Provider(format!("provider returned HTTP {status}"))),
    }
}

fn json_response(response: Response) -> Result<Value, ModelError> {
    status(response.status)?;
    serde_json::from_str(&read(response, catalog::PAGE_LIMIT)?)
        .map_err(|_| ModelError::Provider("invalid model catalog JSON".into()))
}
