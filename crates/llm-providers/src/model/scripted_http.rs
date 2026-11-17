use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Mutex;

use super::http::{Http, Response};

const SOMEWHERE_PUBLIC: IpAddr = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));

type Headers = Vec<(String, String)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub url: String,
    pub headers: Headers,
    pub body: String,
}

#[derive(Debug, Default)]
pub struct ScriptedHttp {
    answers: Mutex<VecDeque<(u16, Headers, String)>>,
    sent: Mutex<Vec<Sent>>,
    resolved: Mutex<HashMap<String, IpAddr>>,
}

impl ScriptedHttp {
    pub fn new(answers: impl IntoIterator<Item = (u16, String)>) -> Self {
        Self::with_headers(answers.into_iter().map(|(status, text)| (status, Vec::new(), text)))
    }

    pub fn with_headers(answers: impl IntoIterator<Item = (u16, Headers, String)>) -> Self {
        Self { answers: Mutex::new(answers.into_iter().collect()), ..Self::default() }
    }

    pub fn redirect(to: &str) -> (u16, Headers, String) {
        (302, vec![("location".to_owned(), to.to_owned())], String::new())
    }

    pub fn page(text: &str) -> (u16, Headers, String) {
        (200, Vec::new(), text.to_owned())
    }

    #[must_use]
    pub fn resolving(self, host: &str, address: IpAddr) -> Self {
        if let Ok(mut resolved) = self.resolved.lock() {
            resolved.insert(host.to_owned(), address);
        }
        self
    }

    pub fn sent(&self) -> Vec<Sent> {
        self.sent.lock().map(|sent| sent.clone()).unwrap_or_default()
    }
}

impl Http for ScriptedHttp {
    fn get(&self, url: &str, headers: &[(String, String)]) -> Result<Response, String> {
        self.post(url, headers, "")
    }

    fn addresses(&self, host: &str) -> Result<Vec<IpAddr>, String> {
        let scripted = self.resolved.lock().ok().and_then(|resolved| resolved.get(host).copied());
        Ok(vec![scripted.unwrap_or(SOMEWHERE_PUBLIC)])
    }

    fn post(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<Response, String> {
        if let Ok(mut sent) = self.sent.lock() {
            sent.push(Sent { url: url.to_owned(), headers: headers.to_vec(), body: body.to_owned() });
        }
        let (status, headers, text) = self
            .answers
            .lock()
            .ok()
            .and_then(|mut answers| answers.pop_front())
            .ok_or_else(|| "no scripted answer left".to_owned())?;
        Ok(Response { status, headers, body: text.into() })
    }
}
