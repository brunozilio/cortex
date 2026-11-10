//! The real [`Hub`]: Hugging Face over HTTPS. `HF_TOKEN`, when set, is
//! sent for gated and private models.

use std::io::{Read, Write};
use std::time::Duration;

use super::{Hub, LocalModelError, RemoteFile, RepoId};

const HUGGING_FACE: &str = "https://huggingface.co";
/// How long a request may wait for a connection, its headers or any one
/// read of its body; a download as a whole may take as long as it needs.
const STALL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct HfHub {
    base: String,
    token: Option<String>,
    client: reqwest::blocking::Client,
}

impl HfHub {
    /// Hugging Face, with the `HF_TOKEN` environment variable's token.
    pub fn from_env() -> Result<Self, LocalModelError> {
        let token = std::env::var("HF_TOKEN").ok().filter(|token| !token.trim().is_empty());
        Self::at(HUGGING_FACE, token)
    }

    fn at(base: &str, token: Option<String>) -> Result<Self, LocalModelError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("superfik/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(STALL)
            .timeout(STALL)
            .build()
            .map_err(|e| LocalModelError::Network { id: "models".into(), why: e.to_string() })?;
        Ok(Self { base: base.trim_end_matches('/').to_owned(), token, client })
    }

    fn get(&self, id: &RepoId, url: &str) -> Result<reqwest::blocking::Response, LocalModelError> {
        let mut request = self.client.get(url);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let response = request.send().map_err(|e| network(id, &e))?;
        match refusal(id, response.status().as_u16()) {
            Some(refused) => Err(refused),
            None => Ok(response),
        }
    }
}

fn network(id: &RepoId, why: &dyn std::fmt::Display) -> LocalModelError {
    LocalModelError::Network { id: id.to_string(), why: why.to_string() }
}

/// What an HTTP status other than success means for a model.
fn refusal(id: &RepoId, status: u16) -> Option<LocalModelError> {
    match status {
        200..=299 => None,
        401 | 403 => Some(LocalModelError::Unavailable(id.to_string())),
        404 => Some(LocalModelError::NotFound(id.to_string())),
        _ => Some(network(id, &format!("Hugging Face answered HTTP {status}"))),
    }
}

impl Hub for HfHub {
    fn files(&self, id: &RepoId) -> Result<Vec<RemoteFile>, LocalModelError> {
        let url = format!("{}/api/models/{id}?blobs=true", self.base);
        let text = self.get(id, &url)?.text().map_err(|e| network(id, &e))?;
        super::files::parse_listing(&text).map_err(|why| network(id, &why))
    }

    fn download(
        &self,
        id: &RepoId,
        file: &str,
        sink: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), LocalModelError> {
        let url = format!("{}/{id}/resolve/main/{file}", self.base);
        let mut response = self.get(id, &url)?;
        let mut buffer = vec![0; 256 * 1024];
        loop {
            let read = response.read(&mut buffer).map_err(|e| network(id, &format!("{file}: {e}")))?;
            if read == 0 {
                return Ok(());
            }
            sink.write_all(&buffer[..read]).map_err(|e| network(id, &format!("{file}: {e}")))?;
            progress(read as u64);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    use super::*;

    /// A one-request HTTP server: answers `status` and `body`, and hands
    /// back the request's head.
    fn serve_once(status: u16, body: &'static str) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let served = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream);
            let mut head = String::new();
            while reader.read_line(&mut head).expect("read") > 2 && !head.ends_with("\r\n\r\n") {}
            let reply =
                format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            reader.get_mut().write_all(reply.as_bytes()).expect("reply");
            head
        });
        (base, served)
    }

    fn hub(base: &str, token: Option<&str>) -> HfHub {
        let mut hub = HfHub::at(base, token.map(str::to_owned)).expect("hub");
        hub.client = reqwest::blocking::Client::builder().no_proxy().build().expect("client");
        hub
    }

    fn id() -> RepoId {
        RepoId::parse("mlx-community/Tiny-4bit").expect("id")
    }

    #[test]
    fn the_listing_is_asked_with_the_token() {
        let (base, served) = serve_once(200, r#"{"siblings":[{"rfilename":"config.json","size":2}]}"#);
        let files = hub(&base, Some("hf_secret")).files(&id()).expect("files");
        assert_eq!(files, [RemoteFile::new("config.json", Some(2))]);
        let head = served.join().expect("served").to_ascii_lowercase();
        assert!(head.starts_with("get /api/models/mlx-community/tiny-4bit?blobs=true "), "{head}");
        assert!(head.contains("authorization: bearer hf_secret"), "{head}");
    }

    #[test]
    fn a_file_is_streamed_from_its_resolve_url() {
        let (base, served) = serve_once(200, "{\"a\":1}");
        let (mut sink, mut seen) = (Vec::new(), 0);
        hub(&base, None).download(&id(), "config.json", &mut sink, &mut |n| seen += n).expect("download");
        assert_eq!((sink.as_slice(), seen), (&b"{\"a\":1}"[..], 7));
        let head = served.join().expect("served").to_ascii_lowercase();
        assert!(head.starts_with("get /mlx-community/tiny-4bit/resolve/main/config.json "), "{head}");
        assert!(!head.contains("authorization"), "no token, no header: {head}");
    }

    #[test]
    fn refusals_say_what_went_wrong() {
        let (base, served) = serve_once(401, "");
        let gated = hub(&base, None).files(&id()).expect_err("refused");
        assert!(matches!(gated, LocalModelError::Unavailable(_)), "{gated:?}");
        served.join().expect("served");
        assert!(matches!(refusal(&id(), 404), Some(LocalModelError::NotFound(_))));
        assert!(matches!(refusal(&id(), 500), Some(LocalModelError::Network { .. })));
        assert!(refusal(&id(), 200).is_none());
    }
}
