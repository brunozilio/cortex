use std::time::Duration;

use super::Releases;

const REPO: &str = brand::REPOSITORY;
const AGENT: &str = concat!(concat!(brand::command!(), "/"), env!("CARGO_PKG_VERSION"));

#[derive(Debug)]
pub struct GitHub {
    http: reqwest::blocking::Client,
}

impl GitHub {
    pub fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .user_agent(AGENT)
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { http })
    }

    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        let response = self.http.get(url).send().map_err(|e| format!("{url}: {e}"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("{url}: {status}"));
        }
        response.bytes().map(|bytes| bytes.to_vec()).map_err(|e| format!("{url}: {e}"))
    }
}

impl Releases for GitHub {
    fn latest_tag(&self) -> Result<String, String> {
        let body = self.get(&format!("https://api.github.com/repos/{REPO}/releases/latest"))?;
        let release: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
        release["tag_name"].as_str().map(str::to_owned).ok_or_else(|| "the latest release has no tag".to_owned())
    }

    fn download(&self, tag: &str, name: &str) -> Result<Vec<u8>, String> {
        self.get(&format!("https://github.com/{REPO}/releases/download/{tag}/{name}"))
    }
}
