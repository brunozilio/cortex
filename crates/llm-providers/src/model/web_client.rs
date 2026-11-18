use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use domain::permission_rules::is_private_address;

const PROXY_VARIABLES: [&str; 6] = ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy", "HTTP_PROXY", "http_proxy"];

pub fn proxied() -> bool {
    PROXY_VARIABLES.iter().any(|key| std::env::var_os(key).is_some_and(|value| !value.is_empty()))
}

pub fn builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
    if proxied() { builder } else { builder.dns_resolver(Arc::new(PublicOnly)) }
}

pub fn private_among(host: &str, addresses: &[IpAddr]) -> Option<String> {
    let private = addresses.iter().find(|address| is_private_address(**address))?;
    Some(format!("{host} resolves to {private}, which is this computer or its private network"))
}

pub fn system_addresses(host: &str) -> Result<Vec<IpAddr>, String> {
    addresses_for(host, proxied(), &|host| {
        (host, 443).to_socket_addrs().map(|found| found.map(|address| address.ip()).collect())
    })
}

pub fn addresses_for(
    host: &str,
    proxied: bool,
    resolve: &dyn Fn(&str) -> std::io::Result<Vec<IpAddr>>,
) -> Result<Vec<IpAddr>, String> {
    let literal = host.trim_start_matches('[').trim_end_matches(']').parse::<IpAddr>();
    if let Ok(address) = literal {
        return Ok(vec![address]);
    }
    match resolve(host) {
        Ok(found) => Ok(found),
        Err(_) if proxied => Ok(Vec::new()),
        Err(error) => Err(format!("{host} could not be resolved: {error}")),
    }
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

#[cfg(test)]
#[path = "web_client_tests.rs"]
mod tests;
