mod client;
#[cfg(any(test, feature = "test-support"))]
pub mod fake;
pub mod framing;
mod pool;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use domain::lsp::ServerSpec;
use serde_json::Value;

pub use client::LspClient;
pub use pool::{LspPool, Processes, Starter};

pub trait LanguageServer: Send + Sync {
    fn request(&self, method: &str, params: Value, stop: &AtomicBool, wait: Duration) -> Result<Value, String>;
    fn sync(&self, file: &Path, language: &str, text: &str) -> Result<i64, String>;
    fn diagnostics(&self, file: &Path, version: i64, wait: Duration) -> Option<Vec<Value>>;
    fn is_alive(&self) -> bool;
}

#[derive(Clone)]
pub struct Opened {
    pub server: Arc<dyn LanguageServer>,
    pub language: String,
    pub root: PathBuf,
}

impl std::fmt::Debug for Opened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opened").field("language", &self.language).field("root", &self.root).finish_non_exhaustive()
    }
}

pub const DIAGNOSTICS_WAIT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug)]
pub struct Lsp {
    pub servers: Arc<dyn LanguageServers>,
    pub specs: Arc<Vec<ServerSpec>>,
}

impl Lsp {
    pub fn open(&self, file: &Path, cwd: &Path, stop: &AtomicBool) -> Result<Opened, String> {
        self.servers.open(file, &self.specs, cwd, stop)
    }

    pub fn problems(&self, file: &Path, text: &str, cwd: &Path, stop: &AtomicBool) -> Option<String> {
        domain::lsp::server_for(file, &self.specs)?;
        let opened = self.open(file, cwd, stop).ok()?;
        let version = opened.server.sync(file, &opened.language, text).ok()?;
        let found = opened.server.diagnostics(file, version, DIAGNOSTICS_WAIT)?;
        let shown = file.strip_prefix(cwd).unwrap_or(file).display().to_string();
        domain::lsp::diagnostics::report(&shown, &found)
    }
}

pub trait LanguageServers: Send + Sync + std::fmt::Debug {
    fn open(&self, file: &Path, specs: &[ServerSpec], cwd: &Path, stop: &AtomicBool) -> Result<Opened, String>;
    fn any_installed(&self, specs: &[ServerSpec]) -> bool;
}

impl LanguageServer for LspClient {
    fn request(&self, method: &str, params: Value, stop: &AtomicBool, wait: Duration) -> Result<Value, String> {
        LspClient::request(self, method, params, stop, wait)
    }
    fn sync(&self, file: &Path, language: &str, text: &str) -> Result<i64, String> {
        LspClient::sync(self, file, language, text)
    }
    fn diagnostics(&self, file: &Path, version: i64, wait: Duration) -> Option<Vec<Value>> {
        LspClient::diagnostics(self, file, version, wait)
    }
    fn is_alive(&self) -> bool {
        LspClient::is_alive(self)
    }
}
