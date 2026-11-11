//! A scripted language server for tests: answers in order, records what it
//! was asked and sent.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use domain::lsp::ServerSpec;

use super::{LanguageServer, LanguageServers, Lsp, Opened};

#[derive(Debug, Default)]
pub struct ScriptedServer {
    pub answers: Mutex<VecDeque<Result<Value, String>>>,
    pub asked: Mutex<Vec<(String, Value)>>,
    pub synced: Mutex<Vec<(PathBuf, String, String)>>,
    pub diagnostics: Mutex<Option<Vec<Value>>>,
}

impl ScriptedServer {
    pub fn answering(answers: impl IntoIterator<Item = Result<Value, String>>) -> Arc<Self> {
        Arc::new(Self { answers: Mutex::new(answers.into_iter().collect()), ..Default::default() })
    }

    pub fn asked(&self) -> Vec<(String, Value)> {
        self.asked.lock().map(|asked| asked.clone()).unwrap_or_default()
    }

    /// The workspace handle a run's tools would get, serving every default.
    pub fn lsp(self: &Arc<Self>) -> Lsp {
        Lsp { servers: Arc::new(ScriptedServers(Arc::clone(self))), specs: Arc::new(domain::lsp::defaults()) }
    }
}

impl LanguageServer for ScriptedServer {
    fn request(&self, method: &str, params: Value, _: &AtomicBool, _: Duration) -> Result<Value, String> {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push((method.to_owned(), params));
        }
        self.answers.lock().ok().and_then(|mut answers| answers.pop_front()).unwrap_or(Ok(Value::Null))
    }

    fn sync(&self, file: &Path, language: &str, text: &str) -> Result<i64, String> {
        let mut synced = self.synced.lock().map_err(|_| "synced")?;
        synced.push((file.to_path_buf(), language.to_owned(), text.to_owned()));
        Ok(i64::try_from(synced.len()).unwrap_or(i64::MAX))
    }

    fn diagnostics(&self, _: &Path, _: i64, _: Duration) -> Option<Vec<Value>> {
        self.diagnostics.lock().ok()?.clone()
    }

    fn is_alive(&self) -> bool {
        true
    }
}

#[derive(Debug)]
struct ScriptedServers(Arc<ScriptedServer>);

impl LanguageServers for ScriptedServers {
    fn open(&self, file: &Path, specs: &[ServerSpec], cwd: &Path, _: &AtomicBool) -> Result<Opened, String> {
        let (_, language) = domain::lsp::server_for(file, specs).ok_or("No language server handles it.")?;
        let server: Arc<dyn LanguageServer> = self.0.clone();
        Ok(Opened { server, language: language.to_owned(), root: cwd.to_path_buf() })
    }

    fn any_installed(&self, specs: &[ServerSpec]) -> bool {
        !specs.is_empty()
    }
}
