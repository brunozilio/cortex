use super::body::Body;
use super::{ModelError, http::Response};
use std::{sync::OnceLock, time::Duration};
use tokio_util::sync::CancellationToken;

pub const STREAM_TIMEOUT: Duration = Duration::from_secs(900);

struct Shared {
    client: reqwest::Client,
    web: reqwest::Client,
}

static SHARED: OnceLock<Result<Shared, String>> = OnceLock::new();

fn shared() -> Result<&'static Shared, ModelError> {
    SHARED
        .get_or_init(|| {
            let runtime = platform::runtime::shared().map_err(|error| error.to_string())?;
            let _entered = runtime.enter();
            let client = reqwest::Client::builder().build().map_err(|error| described(&error))?;
            let web = super::web_client::client().map_err(|error| described(&error))?;
            Ok(Shared { client, web })
        })
        .as_ref()
        .map_err(|error| ModelError::Network(error.clone()))
}

async fn ready(cancel: &CancellationToken) -> Result<&'static Shared, ModelError> {
    if SHARED.get().is_none() {
        let built = tokio::task::spawn_blocking(|| shared().map(|_| ()));
        tokio::select! {
            () = cancel.cancelled() => return Err(ModelError::Cancelled),
            done = built => done.map_err(|error| ModelError::Network(error.to_string()))??,
        }
    }
    shared()
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct Ask<'a> {
    pub url: &'a str,
    pub headers: &'a [(String, String)],
    pub body: Option<&'a str>,
    pub total: Option<Duration>,
    pub idle: Option<Duration>,
    pub one_hop: bool,
}

impl<'a> Ask<'a> {
    pub(crate) fn new(url: &'a str, headers: &'a [(String, String)], body: Option<&'a str>) -> Self {
        Self { url, headers, body, total: None, idle: None, one_hop: false }
    }
}

pub(crate) async fn fetch(ask: Ask<'_>, cancel: &CancellationToken) -> Result<Response, ModelError> {
    if cancel.is_cancelled() {
        return Err(ModelError::Cancelled);
    }
    let shared = ready(cancel).await?;
    let client = if ask.one_hop { &shared.web } else { &shared.client };
    let mut request = match ask.body {
        Some(body) => {
            let request = client.post(ask.url).body(body.to_owned());
            if ask.headers.iter().any(|(key, _)| key.eq_ignore_ascii_case("content-type")) {
                request
            } else {
                request.header("content-type", "application/json")
            }
        }
        None => client.get(ask.url),
    };
    for (key, value) in ask.headers {
        request = request.header(key, value);
    }
    if let Some(total) = ask.total {
        request = request.timeout(total);
    }
    let sent = async {
        match ask.idle {
            Some(idle) => {
                tokio::time::timeout(idle, request.send()).await.map_err(|_| ModelError::Stalled(idle.as_secs()))
            }
            None => Ok(request.send().await),
        }
    };
    let response = tokio::select! {
        biased;
        () = cancel.cancelled() => return Err(ModelError::Cancelled),
        sent = sent => sent?.map_err(|error| ModelError::Network(described(&error)))?,
    };
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .filter_map(|(key, value)| Some((key.as_str().to_owned(), value.to_str().ok()?.to_owned())))
        .collect();
    let body = match ask.idle {
        Some(idle) => Body::remote(response).idle(idle),
        None => Body::remote(response),
    };
    Ok(Response { status, headers, body })
}

#[cfg(test)]
#[path = "cancellable_http_tests.rs"]
mod tests;
