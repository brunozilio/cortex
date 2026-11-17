#[cfg(test)]
mod callers_tests;
mod client;
mod documents;
mod download;
#[cfg(any(test, feature = "test-support"))]
pub mod fake;
pub mod framing;
pub mod install;
mod installer;
mod installs;
mod pool;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use domain::lsp::ServerSpec;
use domain::lsp::answer::RequestError;
use serde_json::Value;

pub use client::{LspClient, StartError};
pub use installer::Downloads;
pub use installs::Installer;
pub use pool::{IDLE, LspPool, Processes, RETRY, Starter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    ForInstall,
    Never,
}

pub trait Fetch: Send + Sync {
    fn get(&self, url: &str) -> Result<(u16, Box<dyn std::io::Read + Send>), String>;
}

pub trait LanguageServer: Send + Sync {
    fn request(&self, method: &str, params: Value, stop: &AtomicBool, wait: Duration) -> Result<Value, RequestError>;
    fn sync(&self, file: &Path, language: &str, text: &str) -> Result<i64, String>;
    fn diagnostics(&self, file: &Path, version: i64, wait: Duration) -> Option<Vec<Value>>;
    fn published(&self) -> Vec<(String, Vec<Value>)>;
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
    pub fn available(&self) -> Vec<ServerSpec> {
        self.servers.available(&self.specs)
    }

    pub fn pending_install(&self, file: &Path) -> Option<String> {
        self.servers.pending_install(file, &self.specs)
    }

    pub fn open(
        &self,
        file: &Path,
        cwd: &Path,
        stop: &AtomicBool,
        installing: Installing,
    ) -> Result<(Opened, Option<String>), String> {
        match self.servers.open(file, &self.specs, cwd, stop, Wait::Never) {
            Ok(opened) => Ok((opened, None)),
            Err(not_yet) => {
                if let (Installing::Refused(why), Some(pending)) = (installing, self.pending_install(file)) {
                    return Err(format!("Would {pending}, but {why}. {not_yet}"));
                }
                let opened = self.servers.open(file, &self.specs, cwd, stop, Wait::ForInstall)?;
                let installed = domain::lsp::server_for(file, &self.specs).map(|(spec, _)| spec.name.clone());
                Ok((opened, installed))
            }
        }
    }

    pub fn problems(&self, file: &Path, text: &str, cwd: &Path, stop: &AtomicBool) -> Option<String> {
        domain::lsp::server_for(file, &self.specs)?;
        let opened = match self.servers.open(file, &self.specs, cwd, stop, Wait::Never) {
            Ok(opened) => opened,
            Err(_) => {
                return self
                    .servers
                    .installing(file, &self.specs)
                    .map(|name| domain::lsp::diagnostics::installing(&name));
            }
        };
        let version = opened.server.sync(file, &opened.language, text).ok()?;
        let found = opened.server.diagnostics(file, version, DIAGNOSTICS_WAIT)?;
        let shown = file.strip_prefix(cwd).unwrap_or(file).display().to_string();
        domain::lsp::diagnostics::report(&shown, &found)
    }

    pub fn callers(&self, file: &Path, text: &str, lines: (u32, u32), cwd: &Path, stop: &AtomicBool) -> Option<String> {
        use domain::lsp::blast;
        domain::lsp::server_for(file, &self.specs)?;
        let opened = self.servers.open(file, &self.specs, cwd, stop, Wait::Never).ok()?;
        opened.server.sync(file, &opened.language, text).ok()?;
        let uri = domain::lsp::uri::uri_of(file);
        let (method, params) = blast::symbols_request(&uri);
        let symbols = opened.server.request(method, params, stop, CALLERS_WAIT).ok()?;
        let answers: Vec<(blast::Changed, Value)> = blast::changed_symbols(&symbols, lines)
            .into_iter()
            .filter_map(|symbol| {
                let (method, params) = blast::references_request(&uri, &symbol);
                let found = opened.server.request(method, params, stop, CALLERS_WAIT).ok()?;
                Some((symbol, found))
            })
            .collect();
        blast::report(file, cwd, &answers)
    }
}

pub const CALLERS_WAIT: Duration = Duration::from_secs(2);

pub trait LanguageServers: Send + Sync + std::fmt::Debug {
    fn open(
        &self,
        file: &Path,
        specs: &[ServerSpec],
        cwd: &Path,
        stop: &AtomicBool,
        wait: Wait,
    ) -> Result<Opened, String>;
    fn available(&self, specs: &[ServerSpec]) -> Vec<ServerSpec>;
    fn installing(&self, file: &Path, specs: &[ServerSpec]) -> Option<String>;
    fn pending_install(&self, _file: &Path, _specs: &[ServerSpec]) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installing {
    Allowed,
    Refused(&'static str),
}

impl LanguageServer for LspClient {
    fn request(&self, method: &str, params: Value, stop: &AtomicBool, wait: Duration) -> Result<Value, RequestError> {
        LspClient::request(self, method, params, stop, wait)
    }
    fn sync(&self, file: &Path, language: &str, text: &str) -> Result<i64, String> {
        LspClient::sync(self, file, language, text)
    }
    fn diagnostics(&self, file: &Path, version: i64, wait: Duration) -> Option<Vec<Value>> {
        LspClient::diagnostics(self, file, version, wait)
    }
    fn published(&self) -> Vec<(String, Vec<Value>)> {
        LspClient::published(self)
    }
    fn is_alive(&self) -> bool {
        LspClient::is_alive(self)
    }
}
