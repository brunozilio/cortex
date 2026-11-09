//! The local provider's upstream: a Chat Completions server on this
//! computer (`mlx_lm.server`), found through [`Endpoints`].

use std::io::BufReader;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::chat;
use crate::messages::MessagesRequest;
use crate::server::{Upstream, UpstreamError};
use crate::sse;
use crate::stream::{ChatStream, Event};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub base_url: String,
    pub model: String,
}

pub trait Endpoints: Send + Sync {
    fn endpoint(&self, model: &str) -> Result<Endpoint, UpstreamError>;
}

pub struct ChatUpstream {
    endpoints: Arc<dyn Endpoints>,
    agent: ureq::Agent,
}

impl std::fmt::Debug for ChatUpstream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatUpstream").finish_non_exhaustive()
    }
}

impl ChatUpstream {
    pub fn new(endpoints: Arc<dyn Endpoints>) -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(5)))
            .build()
            .into();
        Self { endpoints, agent }
    }
}

impl Upstream for ChatUpstream {
    fn messages(&self, request: &MessagesRequest, emit: &mut dyn FnMut(Event)) -> Result<(), UpstreamError> {
        let endpoint = self.endpoints.endpoint(&request.model)?;
        let mut body = chat::request(request);
        body.model = endpoint.model;
        body.stream = true;
        body.stream_options = Some(json!({ "include_usage": true }));
        let url = format!("{}/v1/chat/completions", endpoint.base_url);
        let response =
            self.agent.post(&url).send_json(&body).map_err(|error| {
                UpstreamError::Unavailable(format!("the local model server did not answer: {error}"))
            })?;
        let status = response.status().as_u16();
        let mut body = response.into_body();
        if status != 200 {
            let text = body.read_to_string().unwrap_or_default();
            let reason = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
                .unwrap_or(text);
            return Err(match status {
                400..=499 => UpstreamError::Invalid(reason),
                503 => UpstreamError::Unavailable(reason),
                _ => UpstreamError::Failed(reason),
            });
        }
        let mut stream = ChatStream::new(&request.model);
        stream.start().into_iter().for_each(&mut *emit);
        sse::relay(BufReader::new(body.into_reader()), &mut stream, emit)?;
        stream.finish().into_iter().for_each(&mut *emit);
        Ok(())
    }
}
