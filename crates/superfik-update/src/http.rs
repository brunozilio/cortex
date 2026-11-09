//! The latest release on GitHub and the archive's download over HTTPS.
//! Each release carries one manifest per platform
//! (`superfik-<platform>.json`, written by `.github/workflows/scripts/announce-update.sh`),
//! so `releases/latest/download/<that>` is always the newest one.

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::{Manifest, Platform, UpdateError, Version};

pub const SERVICE: &str = "https://github.com/brunozilio/superfik/releases";

const MAX_ARCHIVE: u64 = 4 << 30;

pub trait Releases: Send + Sync {
    fn latest(&self, platform: Platform, current: &Version) -> Result<Option<Manifest>, UpdateError>;
}

pub trait Fetch: Send + Sync {
    fn download(&self, url: &str, to: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<(), UpdateError>;
}

#[derive(Debug, Clone)]
pub struct HttpReleases {
    base: String,
}

impl HttpReleases {
    pub fn new(base: &str) -> Self {
        Self { base: base.trim_end_matches('/').to_owned() }
    }

    pub fn from_env() -> Self {
        Self::new(&std::env::var("SUPERFIK_RELEASES").unwrap_or_else(|_| SERVICE.to_owned()))
    }

    pub fn latest_url(&self, platform: Platform) -> String {
        format!("{}/latest/download/superfik-{}.json", self.base, platform.as_str())
    }
}

impl Releases for HttpReleases {
    fn latest(&self, platform: Platform, _current: &Version) -> Result<Option<Manifest>, UpdateError> {
        let mut response = ureq::get(&self.latest_url(platform))
            .config()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .call()
            .map_err(|error| UpdateError::Unreachable(error.to_string()))?;
        let status = response.status().as_u16();
        let answer: Value = response.body_mut().read_json().unwrap_or(Value::Null);
        answered(status, answer)
    }
}

pub(crate) fn answered(status: u16, answer: Value) -> Result<Option<Manifest>, UpdateError> {
    match status {
        200 => serde_json::from_value(answer).map(Some).map_err(|_| UpdateError::Refused("unreadable".into())),
        404 => Ok(None),
        _ => Err(UpdateError::Refused(
            answer["error"].as_str().map_or_else(|| format!("status {status}"), str::to_owned),
        )),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HttpFetch;

impl Fetch for HttpFetch {
    fn download(&self, url: &str, to: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<(), UpdateError> {
        let unreachable = |error: &dyn std::fmt::Display| UpdateError::Unreachable(error.to_string());
        let mut response = ureq::get(url)
            .config()
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(30)))
            .timeout_recv_body(None)
            .build()
            .call()
            .map_err(|error| unreachable(&error))?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(UpdateError::Refused(format!("download answered {status}")));
        }
        let total = response.body().content_length().unwrap_or(0);
        let mut reader = response.body_mut().with_config().limit(MAX_ARCHIVE).reader();
        let mut file = std::fs::File::create(to).map_err(UpdateError::install)?;
        copy(&mut reader, &mut file, total, progress).map_err(|error| unreachable(&error))?;
        file.sync_all().map_err(UpdateError::install)
    }
}

pub(crate) fn copy(
    from: &mut dyn Read,
    to: &mut dyn Write,
    total: u64,
    progress: &mut dyn FnMut(u64, u64),
) -> std::io::Result<u64> {
    let mut buffer = vec![0; 1 << 16];
    let (mut done, mut reported) = (0u64, 0u64);
    progress(0, total);
    loop {
        let read = from.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        to.write_all(&buffer[..read])?;
        done += read as u64;
        if done - reported >= 1 << 18 {
            reported = done;
            progress(done, total);
        }
    }
    if reported != done {
        progress(done, total);
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn asks_the_latest_release_for_the_platforms_manifest() {
        let releases = HttpReleases::new("https://github.com/acme/superfik/releases/");
        assert_eq!(
            releases.latest_url(Platform::LinuxX86_64),
            "https://github.com/acme/superfik/releases/latest/download/superfik-linux-x86_64.json"
        );
    }

    #[test]
    fn reads_the_services_answers() {
        let manifest = json!({
            "version": "0.15.0", "minimum": null, "notes_url": null, "published_unix": 1,
            "asset": { "url": "u", "size": 1, "sha256": "a", "signature": "b" }
        });
        assert_eq!(answered(200, manifest).ok().flatten().map(|m| m.version), Some(Version::new(0, 15, 0)));
        assert_eq!(answered(404, Value::Null), Ok(None));
        assert_eq!(answered(200, json!({"version": 3})), Err(UpdateError::Refused("unreadable".into())));
        assert_eq!(answered(500, json!({"error": "down"})), Err(UpdateError::Refused("down".into())));
        assert_eq!(answered(502, Value::Null), Err(UpdateError::Refused("status 502".into())));
    }

    #[test]
    fn copies_and_reports_progress() {
        let bytes = vec![7u8; 600 * 1024];
        let mut out = Vec::new();
        let mut seen = Vec::new();
        let done =
            copy(&mut bytes.as_slice(), &mut out, bytes.len() as u64, &mut |d, t| seen.push((d, t))).expect("copy");
        assert_eq!(done, bytes.len() as u64);
        assert_eq!(out, bytes);
        assert_eq!(seen.first(), Some(&(0, bytes.len() as u64)));
        assert_eq!(seen.last(), Some(&(bytes.len() as u64, bytes.len() as u64)));
        assert!(seen.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }
}
