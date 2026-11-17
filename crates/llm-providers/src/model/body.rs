use std::io::{self, BufRead, Read};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::ModelError;

pub const TOO_LONG: &str = "provider stream exceeds size limit";
pub const LINE_AT_MOST: usize = 8 * 1024 * 1024;

pub type Chunk = Result<Vec<u8>, String>;

enum Source {
    Done,
    Reader(Box<dyn BufRead + Send>),
    Remote(reqwest::Response),
    Chunks(mpsc::Receiver<Chunk>),
}

pub struct Body {
    source: Source,
    buffer: Vec<u8>,
    at: usize,
    idle: Option<Duration>,
    remaining: Option<usize>,
}

impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Body").field("buffered", &(self.buffer.len() - self.at)).finish_non_exhaustive()
    }
}

impl From<Vec<u8>> for Body {
    fn from(bytes: Vec<u8>) -> Self {
        Self::of(Source::Done, bytes)
    }
}

impl From<String> for Body {
    fn from(text: String) -> Self {
        Self::from(text.into_bytes())
    }
}

impl From<&str> for Body {
    fn from(text: &str) -> Self {
        Self::from(text.as_bytes().to_vec())
    }
}

enum Next {
    Bytes(Vec<u8>),
    End,
    NotYet,
}

impl Body {
    fn of(source: Source, buffer: Vec<u8>) -> Self {
        Self { source, buffer, at: 0, idle: None, remaining: None }
    }

    pub fn reader(reader: impl BufRead + Send + 'static) -> Self {
        Self::of(Source::Reader(Box::new(reader)), Vec::new())
    }

    pub fn chunks(chunks: mpsc::Receiver<Chunk>) -> Self {
        Self::of(Source::Chunks(chunks), Vec::new())
    }

    pub(crate) fn remote(response: reqwest::Response) -> Self {
        Self::of(Source::Remote(response), Vec::new())
    }

    #[must_use]
    pub fn idle(mut self, deadline: Duration) -> Self {
        self.idle = Some(deadline);
        self
    }

    #[must_use]
    pub fn bounded(mut self, limit: usize) -> Self {
        self.remaining = Some(limit);
        self
    }

    fn buffered(&self) -> &[u8] {
        &self.buffer[self.at..]
    }

    fn take(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        if let Some(remaining) = self.remaining.as_mut() {
            if bytes.len() > *remaining {
                *remaining = 0;
                return Err(TOO_LONG.into());
            }
            *remaining -= bytes.len();
        }
        self.buffer = bytes;
        self.at = 0;
        Ok(())
    }

    fn read_reader(reader: &mut dyn BufRead) -> io::Result<Next> {
        let data = reader.fill_buf()?;
        if data.is_empty() {
            return Ok(Next::End);
        }
        let bytes = data.to_vec();
        reader.consume(bytes.len());
        Ok(Next::Bytes(bytes))
    }

    async fn arrived(&mut self, cancel: &CancellationToken) -> Result<Next, ModelError> {
        let idle = self.idle;
        let waited = async {
            match &mut self.source {
                Source::Done => Ok(Next::End),
                Source::Reader(reader) => match Self::read_reader(reader.as_mut()) {
                    Ok(next) => Ok(next),
                    Err(error) if is_pause(&error) => {
                        tokio::task::yield_now().await;
                        Ok(Next::NotYet)
                    }
                    Err(error) => Err(ModelError::Network(error.to_string())),
                },
                Source::Remote(response) => within(idle, response.chunk()).await.and_then(|chunk| match chunk {
                    Ok(Some(bytes)) => Ok(Next::Bytes(bytes.to_vec())),
                    Ok(None) => Ok(Next::End),
                    Err(error) => Err(ModelError::Network(super::cancellable_http::described(&error))),
                }),
                Source::Chunks(chunks) => within(idle, chunks.recv()).await.and_then(|chunk| match chunk {
                    Some(Ok(bytes)) => Ok(Next::Bytes(bytes)),
                    Some(Err(error)) => Err(ModelError::Network(error)),
                    None => Ok(Next::End),
                }),
            }
        };
        tokio::select! {
            biased;
            () = cancel.cancelled() => Err(ModelError::Cancelled),
            next = waited => next,
        }
    }

    pub async fn fill(&mut self, cancel: &CancellationToken) -> Result<&[u8], ModelError> {
        while self.at == self.buffer.len() {
            if cancel.is_cancelled() {
                return Err(ModelError::Cancelled);
            }
            match self.arrived(cancel).await? {
                Next::Bytes(bytes) => self.take(bytes).map_err(ModelError::Network)?,
                Next::End => {
                    self.source = Source::Done;
                    return Ok(&[]);
                }
                Next::NotYet => {}
            }
        }
        Ok(self.buffered())
    }

    pub fn consume_some(&mut self, count: usize) {
        self.at = (self.at + count).min(self.buffer.len());
    }

    pub async fn read_line(&mut self, line: &mut String, cancel: &CancellationToken) -> Result<usize, ModelError> {
        self.read_line_within(line, cancel, LINE_AT_MOST).await
    }

    pub async fn read_line_within(
        &mut self,
        line: &mut String,
        cancel: &CancellationToken,
        most: usize,
    ) -> Result<usize, ModelError> {
        let mut bytes = Vec::new();
        loop {
            let data = self.fill(cancel).await?;
            if data.is_empty() {
                break;
            }
            let (taken, ended) = match data.iter().position(|byte| *byte == b'\n') {
                Some(at) => (at + 1, true),
                None => (data.len(), false),
            };
            if bytes.len() + taken > most {
                return Err(ModelError::Oversized(most));
            }
            bytes.extend_from_slice(&data[..taken]);
            self.consume_some(taken);
            if ended {
                break;
            }
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| ModelError::Network("stream did not contain valid UTF-8".into()))?;
        line.push_str(text);
        Ok(bytes.len())
    }

    pub async fn read_full(&mut self, out: &mut [u8], cancel: &CancellationToken) -> Result<usize, ModelError> {
        let mut filled = 0;
        while filled < out.len() {
            let data = self.fill(cancel).await?;
            if data.is_empty() {
                break;
            }
            let count = data.len().min(out.len() - filled);
            out[filled..filled + count].copy_from_slice(&data[..count]);
            self.consume_some(count);
            filled += count;
        }
        Ok(filled)
    }

    pub async fn text(mut self, cancel: &CancellationToken, limit: usize) -> Result<String, ModelError> {
        let mut bytes = Vec::new();
        loop {
            let data = self.fill(cancel).await?;
            if data.is_empty() {
                break;
            }
            if bytes.len() + data.len() > limit {
                return Err(ModelError::Provider("provider response exceeds size limit".into()));
            }
            bytes.extend_from_slice(data);
            let count = data.len();
            self.consume_some(count);
        }
        String::from_utf8(bytes).map_err(|_| ModelError::Network("the answer was not valid UTF-8".into()))
    }

    pub async fn capped(mut self, cancel: &CancellationToken, cap: usize) -> Result<Vec<u8>, ModelError> {
        let mut bytes = Vec::new();
        while bytes.len() < cap {
            let data = self.fill(cancel).await?;
            if data.is_empty() {
                break;
            }
            let count = data.len().min(cap - bytes.len());
            bytes.extend_from_slice(&data[..count]);
            self.consume_some(count);
        }
        Ok(bytes)
    }

    fn arrived_blocking(&mut self) -> io::Result<Next> {
        let failed = |error: &dyn std::fmt::Display| io::Error::other(error.to_string());
        match &mut self.source {
            Source::Done => Ok(Next::End),
            Source::Reader(reader) => Self::read_reader(reader.as_mut()),
            Source::Remote(response) => match platform::runtime::block_on(response.chunk()).map_err(|e| failed(&e))? {
                Ok(Some(bytes)) => Ok(Next::Bytes(bytes.to_vec())),
                Ok(None) => Ok(Next::End),
                Err(error) => Err(failed(&super::cancellable_http::described(&error))),
            },
            Source::Chunks(chunks) => match platform::runtime::block_on(chunks.recv()).map_err(|e| failed(&e))? {
                Some(Ok(bytes)) => Ok(Next::Bytes(bytes)),
                Some(Err(error)) => Err(failed(&error)),
                None => Ok(Next::End),
            },
        }
    }
}

fn is_pause(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted)
}

async fn within<T>(idle: Option<Duration>, next: impl Future<Output = T>) -> Result<T, ModelError> {
    match idle {
        Some(deadline) => {
            tokio::time::timeout(deadline, next).await.map_err(|_| ModelError::Stalled(deadline.as_secs()))
        }
        None => Ok(next.await),
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
            match self.arrived_blocking()? {
                Next::Bytes(bytes) => {
                    self.take(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                }
                Next::End => self.source = Source::Done,
                Next::NotYet => {}
            }
        }
        Ok(self.buffered())
    }

    fn consume(&mut self, count: usize) {
        self.consume_some(count);
    }
}

#[cfg(test)]
#[path = "body_tests.rs"]
mod tests;
