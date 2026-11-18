use super::body::Body;
use super::{ModelError, http::Response};
use std::{sync::OnceLock, time::Duration};
use tokio_util::sync::CancellationToken;

pub const STREAM_TIMEOUT: Duration = Duration::from_secs(900);

struct Shared {
    client: reqwest::Client,
    web: reqwest::Client,
}

static SECURE: OnceLock<Result<Shared, String>> = OnceLock::new();
static PLAIN: OnceLock<Result<Shared, String>> = OnceLock::new();
const REDIRECTS_AT_MOST: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Roots {
    Loaded,
    Skipped,
}

fn roots_for(url: &str, proxied: bool) -> Roots {
    let plain = url.get(..7).is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"));
    if plain && !proxied { Roots::Skipped } else { Roots::Loaded }
}

fn cell(roots: Roots) -> &'static OnceLock<Result<Shared, String>> {
    match roots {
        Roots::Loaded => &SECURE,
        Roots::Skipped => &PLAIN,
    }
}

fn plain_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.url().scheme() != "http" {
            attempt.stop()
        } else if attempt.previous().len() >= REDIRECTS_AT_MOST {
            attempt.error("too many redirects")
        } else {
            attempt.follow()
        }
    })
}

fn build(roots: Roots) -> Result<Shared, String> {
    let runtime = platform::runtime::shared().map_err(|error| error.to_string())?;
    let _entered = runtime.enter();
    let (client, web) = match roots {
        Roots::Loaded => (reqwest::Client::builder(), super::web_client::builder()),
        Roots::Skipped => (
            reqwest::Client::builder().tls_built_in_native_certs(false).redirect(plain_redirects()),
            super::web_client::builder().tls_built_in_native_certs(false),
        ),
    };
    let client = client.build().map_err(|error| described(&error))?;
    let web = web.build().map_err(|error| described(&error))?;
    Ok(Shared { client, web })
}

fn shared(roots: Roots) -> Result<&'static Shared, ModelError> {
    cell(roots).get_or_init(|| build(roots)).as_ref().map_err(|error| ModelError::Network(error.clone()))
}

async fn ready(roots: Roots, cancel: &CancellationToken) -> Result<&'static Shared, ModelError> {
    if cell(roots).get().is_none() {
        let built = tokio::task::spawn_blocking(move || shared(roots).map(|_| ()));
        tokio::select! {
            () = cancel.cancelled() => return Err(ModelError::Cancelled),
            done = built => done.map_err(|error| ModelError::Network(error.to_string()))??,
        }
    }
    shared(roots)
}

fn leaves_for_https(response: &reqwest::Response) -> bool {
    let location = response.headers().get(reqwest::header::LOCATION).and_then(|value| value.to_str().ok());
    response.status().is_redirection()
        && location
            .and_then(|location| response.url().join(location).ok())
            .is_some_and(|target| target.scheme() == "https")
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
    let roots = roots_for(ask.url, super::web_client::proxied());
    let mut response = sent(ask, roots, cancel).await?;
    if roots == Roots::Skipped && !ask.one_hop && leaves_for_https(&response) {
        drop(response);
        response = sent(ask, Roots::Loaded, cancel).await?;
    }
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

async fn sent(ask: Ask<'_>, roots: Roots, cancel: &CancellationToken) -> Result<reqwest::Response, ModelError> {
    let shared = ready(roots, cancel).await?;
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
    tokio::select! {
        biased;
        () = cancel.cancelled() => Err(ModelError::Cancelled),
        sent = sent => sent?.map_err(|error| ModelError::Network(described(&error))),
    }
}

#[cfg(test)]
#[path = "cancellable_http_tests.rs"]
mod tests;
