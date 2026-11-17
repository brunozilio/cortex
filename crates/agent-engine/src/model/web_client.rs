use std::io::{self, BufRead};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use domain::permission_rules::is_private_address;

const PROXY_VARIABLES: [&str; 6] = ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy", "HTTP_PROXY", "http_proxy"];

pub fn proxied() -> bool {
    PROXY_VARIABLES.iter().any(|key| std::env::var_os(key).is_some_and(|value| !value.is_empty()))
}

pub fn client() -> reqwest::Result<reqwest::Client> {
    let builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
    if proxied() { builder.build() } else { builder.dns_resolver(Arc::new(PublicOnly)).build() }
}

pub fn private_among(host: &str, addresses: &[IpAddr]) -> Option<String> {
    let private = addresses.iter().find(|address| is_private_address(**address))?;
    Some(format!("{host} resolves to {private}, which is this computer or its private network"))
}

pub fn system_addresses(host: &str) -> Result<Vec<IpAddr>, String> {
    if proxied() {
        return Ok(Vec::new());
    }
    let found = (host, 443).to_socket_addrs().map_err(|error| format!("{host} could not be resolved: {error}"))?;
    Ok(found.map(|address| address.ip()).collect())
}

struct PublicOnly;

impl reqwest::dns::Resolve for PublicOnly {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let found: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            let addresses: Vec<IpAddr> = found.iter().map(SocketAddr::ip).collect();
            if let Some(why) = private_among(&host, &addresses) {
                return Err(why.into());
            }
            Ok(Box::new(found.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

pub fn read_capped(body: &mut dyn BufRead, cap: u64) -> io::Result<Vec<u8>> {
    let cap = usize::try_from(cap).unwrap_or(usize::MAX);
    let mut bytes = Vec::new();
    while bytes.len() < cap {
        let taken = match body.fill_buf() {
            Ok([]) => break,
            Ok(data) => {
                let taken = data.len().min(cap - bytes.len());
                bytes.extend_from_slice(&data[..taken]);
                taken
            }
            Err(error) if matches!(error.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => continue,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        body.consume(taken);
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "web_client_tests.rs"]
mod tests;
