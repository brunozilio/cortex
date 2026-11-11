use domain::{Access, ModelOption};
use serde_json::Value;

use super::{Call, Config, Page, Provider, ProviderError};

#[derive(Debug)]
pub struct Stand {
    pub id: &'static str,
    pub name: &'static str,
    pub vendor: &'static str,
    pub access: Access,
    pub rank: u8,
}

pub static KEYED: Stand = Stand { id: "keyed", name: "Keyed", vendor: "keyed", access: Access::Key, rank: 200 };
pub static KEYED_TOO: Stand = Stand { id: "keyed-too", name: "Keyed Too", vendor: "keyed-too", access: Access::Key, rank: 201 };
pub static BROWSER: Stand = Stand { id: "browser-sub", name: "Browser Subscription", vendor: "browser-sub", access: Access::Browser, rank: 202 };
pub static DEVICE: Stand =
    Stand { id: "device-sub", name: "Device Subscription", vendor: "device-sub", access: Access::Code, rank: 203 };
pub static FAMILY: Stand = Stand { id: "family-key", name: "Family Key", vendor: "anthropic", access: Access::Key, rank: 204 };

pub fn install() {
    crate::declared::install(&[&KEYED, &KEYED_TOO, &BROWSER, &DEVICE, &FAMILY]);
}

impl Provider for Stand {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn vendor(&self) -> &'static str {
        self.vendor
    }
    fn access(&self) -> Access {
        self.access
    }
    fn rank(&self) -> u8 {
        self.rank
    }
    fn slot(&self) -> u128 {
        0x7465_7374_0000_0000_0000_0000_0000_0000 + u128::from(self.rank)
    }
    fn window(&self) -> u64 {
        1000
    }
    fn wire(&self, _model: Option<&ModelOption>) -> &'static str {
        "chat"
    }
    fn base(&self, _config: &Config) -> String {
        format!("https://{}.test", self.id)
    }
    fn headers(&self, config: &Config) -> Vec<(String, String)> {
        vec![("authorization".into(), format!("Bearer {}", config.api_key))]
    }
    fn path(&self, _model: Option<&ModelOption>) -> &'static str {
        "/chat"
    }
    fn shape(&self, _model: Option<&ModelOption>, _body: &mut Value, _effort: &str) -> Result<(), ProviderError> {
        Ok(())
    }
    fn catalog(&self, config: &Config, after: &str) -> Call {
        Call::get(self, config, &format!("/models?after={after}"))
    }
    fn page(&self, value: &Value) -> Result<Page, ProviderError> {
        let models = value["data"].as_array().ok_or_else(|| ProviderError::Invalid("no model array".into()))?;
        let models = models
            .iter()
            .filter_map(|item| item["id"].as_str())
            .map(|id| crate::catalog::option(self, id, id.to_owned()))
            .collect();
        Ok(Page { models, next: value["next"].as_str().map(str::to_owned) })
    }
}

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::sign_in::{Answer, Host, Listener};

#[derive(Debug, Default)]
pub struct Log {
    pub opened: Vec<(String, Option<String>)>,
    pub sent: Vec<(String, String)>,
    pub answered: Vec<String>,
    pub waits: Vec<std::time::Duration>,
}

#[derive(Debug, Default)]
pub struct ScriptedHost {
    replies: RefCell<VecDeque<(u16, String)>>,
    visits: Rc<RefCell<VecDeque<(String, String)>>>,
    pub log: Rc<RefCell<Log>>,
    count: RefCell<usize>,
}

impl ScriptedHost {
    pub fn new(
        replies: impl IntoIterator<Item = (u16, String)>,
        visits: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> Self {
        Self {
            replies: RefCell::new(replies.into_iter().collect()),
            visits: Rc::new(RefCell::new(visits.into_iter().map(|(p, q)| (p.to_owned(), q.to_owned())).collect())),
            ..Self::default()
        }
    }

    fn reply(&self, url: &str, body: &str) -> Result<(u16, String), crate::ProviderError> {
        self.log.borrow_mut().sent.push((url.to_owned(), body.to_owned()));
        self.replies.borrow_mut().pop_front().ok_or_else(|| crate::ProviderError::Invalid("no reply scripted".into()))
    }
}

impl Host for ScriptedHost {
    fn post(
        &self,
        url: &str,
        _headers: &[(String, String)],
        body: &str,
    ) -> Result<(u16, String), crate::ProviderError> {
        self.reply(url, body)
    }
    fn get(&self, url: &str, _headers: &[(String, String)]) -> Result<(u16, String), crate::ProviderError> {
        self.reply(url, "")
    }
    fn open(&self, url: String, code: Option<String>) {
        self.log.borrow_mut().opened.push((url, code));
    }
    fn listen(&self, port: u16, path: &str) -> Result<Box<dyn Listener>, crate::ProviderError> {
        Ok(Box::new(ScriptedListener {
            port: if port == 0 { 7 } else { port },
            path: path.to_owned(),
            visits: self.visits.clone(),
            log: self.log.clone(),
        }))
    }
    fn random(&self, bytes: usize) -> Result<String, crate::ProviderError> {
        let mut count = self.count.borrow_mut();
        *count += 1;
        Ok(format!("r{}x{bytes}", *count))
    }
    fn sleep(&self, duration: std::time::Duration) {
        self.log.borrow_mut().waits.push(duration);
    }
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::UNIX_EPOCH
    }
}

struct ScriptedListener {
    port: u16,
    path: String,
    visits: Rc<RefCell<VecDeque<(String, String)>>>,
    log: Rc<RefCell<Log>>,
}

impl Listener for ScriptedListener {
    fn redirect_uri(&self) -> String {
        format!("http://localhost:{}{}", self.port, self.path)
    }
    fn port(&self) -> u16 {
        self.port
    }
    fn next(&self, _state: &str, _others: &[&str]) -> Result<Box<dyn Answer>, crate::ProviderError> {
        let (path, query) = self
            .visits
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| crate::ProviderError::Invalid("the sign-in timed out".into()))?;
        Ok(Box::new(ScriptedAnswer { path, query, log: self.log.clone() }))
    }
}

struct ScriptedAnswer {
    path: String,
    query: String,
    log: Rc<RefCell<Log>>,
}

impl Answer for ScriptedAnswer {
    fn path(&self) -> &str {
        &self.path
    }
    fn query(&self) -> &str {
        &self.query
    }
    fn code(&self) -> Result<String, crate::ProviderError> {
        url::form_urlencoded::parse(self.query.as_bytes())
            .find(|(key, _)| key == "code")
            .map(|(_, value)| value.into_owned())
            .ok_or_else(|| crate::ProviderError::Unauthorized("the sign-in returned no code".into()))
    }
    fn redirect(self: Box<Self>, to: &str) {
        self.log.borrow_mut().answered.push(format!("redirect {to}"));
    }
    fn finish(self: Box<Self>, result: Result<(), String>) {
        self.log.borrow_mut().answered.push(match result {
            Ok(()) => "done".to_owned(),
            Err(error) => format!("failed {error}"),
        });
    }
}
