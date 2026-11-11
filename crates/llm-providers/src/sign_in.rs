use chrono::{DateTime, Utc};
use std::time::Duration;

use super::{ProviderError, TokenRequest};

pub trait Host {
    fn post(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<(u16, String), ProviderError>;
    fn get(&self, url: &str, headers: &[(String, String)]) -> Result<(u16, String), ProviderError>;
    fn open(&self, url: String, code: Option<String>);
    fn listen(&self, port: u16, path: &str) -> Result<Box<dyn Listener>, ProviderError>;
    fn random(&self, bytes: usize) -> Result<String, ProviderError>;
    fn sleep(&self, duration: Duration);
    fn now(&self) -> DateTime<Utc>;
}

pub trait Listener {
    fn redirect_uri(&self) -> String;
    fn port(&self) -> u16;
    fn next(&self, state: &str, others: &[&str]) -> Result<Box<dyn Answer>, ProviderError>;
}

pub trait Answer {
    fn path(&self) -> &str;
    fn query(&self) -> &str;
    fn code(&self) -> Result<String, ProviderError>;
    fn redirect(self: Box<Self>, to: &str);
    fn finish(self: Box<Self>, result: Result<(), String>);
}

pub fn post(host: &dyn Host, request: &TokenRequest) -> Result<(u16, String), ProviderError> {
    host.post(&request.url, &[("content-type".to_owned(), request.content_type.to_owned())], &request.body)
}
