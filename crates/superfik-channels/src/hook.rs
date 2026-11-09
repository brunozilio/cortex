//! A provider's call to an account's webhook on the server
//! (`/hooks/channels/<connection>`): Meta's and LinkedIn's verification
//! handshakes, X's CRC, and the signed events. The server hands the request
//! to the account's [`Channel::hook`](crate::Channel::hook), which checks
//! the provider's signature and reports what it carried as events.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookMethod {
    Get,
    Post,
}

#[derive(Clone, PartialEq, Eq)]
pub struct HookRequest {
    pub method: HookMethod,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl HookRequest {
    pub fn get(query: &[(&str, &str)]) -> Self {
        let query = query.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        Self { method: HookMethod::Get, query, headers: BTreeMap::new(), body: Vec::new() }
    }

    pub fn post(headers: &[(&str, &str)], body: impl Into<Vec<u8>>) -> Self {
        let headers = headers.iter().map(|(k, v)| (k.to_ascii_lowercase(), (*v).to_owned())).collect();
        Self { method: HookMethod::Post, query: BTreeMap::new(), headers, body: body.into() }
    }

    pub fn query(&self, key: &str) -> Option<&str> {
        self.query.get(key).map(String::as_str)
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_ascii_lowercase()).map(String::as_str)
    }
}

impl std::fmt::Debug for HookRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookRequest")
            .field("method", &self.method)
            .field("query", &self.query.keys().collect::<Vec<_>>())
            .field("body", &self.body.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookAnswer {
    Accepted,
    Text(String),
    Json(String),
    Refused,
}
