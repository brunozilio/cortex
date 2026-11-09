//! A server's address as typed, made into its web address and socket.
//!
//! - A scheme written is the one used: `https://` or `http://`, nothing
//!   else.
//! - A bare address (`superfik.acme.io`, `192.168.1.5:7420`) is HTTPS, but
//!   for this computer (`localhost`, `127.0.0.1`, `::1`), which is HTTP.
//! - HTTP to another computer would send the password and the sign-in
//!   token in clear, so it needs the user's say-so for that address.

use std::net::IpAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressProblem {
    Invalid,
    Insecure,
}

pub fn server_urls(address: &str, insecure: bool) -> Result<(String, String), AddressProblem> {
    let address = address.trim().trim_end_matches('/');
    if address.is_empty() || address.contains(char::is_whitespace) {
        return Err(AddressProblem::Invalid);
    }
    let (scheme, rest) = match address.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("https") => (Some(Scheme::Https), rest),
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("http") => (Some(Scheme::Http), rest),
        Some(_) => return Err(AddressProblem::Invalid),
        None => (None, address),
    };
    let local = is_loopback(host(rest)?);
    let scheme = scheme.unwrap_or(if local { Scheme::Http } else { Scheme::Https });
    if scheme == Scheme::Http && !local && !insecure {
        return Err(AddressProblem::Insecure);
    }
    let (web, socket) = match scheme {
        Scheme::Https => ("https", "wss"),
        Scheme::Http => ("http", "ws"),
    };
    Ok((format!("{web}://{rest}"), format!("{socket}://{rest}/ws")))
}

pub fn bare_remote(address: &str) -> bool {
    let address = address.trim().trim_end_matches('/');
    !address.contains("://") && host(address).is_ok_and(|host| !is_loopback(host))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scheme {
    Https,
    Http,
}

fn host(rest: &str) -> Result<&str, AddressProblem> {
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return Err(AddressProblem::Invalid);
    }
    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, after) = bracketed.split_once(']').ok_or(AddressProblem::Invalid)?;
        host.parse::<std::net::Ipv6Addr>().map_err(|_| AddressProblem::Invalid)?;
        let port = if after.is_empty() { None } else { Some(after.strip_prefix(':').ok_or(AddressProblem::Invalid)?) };
        (host, port)
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    let port_ok = port.is_none_or(|port| port.parse::<u16>().is_ok());
    if host.is_empty() || !port_ok {
        return Err(AddressProblem::Invalid);
    }
    Ok(host)
}

fn is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}
