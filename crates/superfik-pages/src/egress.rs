//! The only way out for pages on the server: a SOCKS5 proxy on loopback
//! that the server's browser sends every connection through (navigations,
//! redirects, subresources, sockets). It resolves each name itself, checks
//! every address the name resolves to ([`address::is_public`]), and
//! connects to the addresses it checked, so neither a redirect nor a DNS
//! answer that changes (rebinding) reaches the server's own machine or
//! network: loopback, private ranges, link-local cloud metadata.

use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use superfik_browser::address;

pub type Resolve = Arc<dyn Fn(&str, u16) -> io::Result<Vec<SocketAddr>> + Send + Sync>;

const PATIENCE: Duration = Duration::from_secs(10);
const MOST: usize = 512;

const SUCCEEDED: u8 = 0;
const NOT_ALLOWED: u8 = 2;
const UNREACHABLE: u8 = 4;
const REFUSED: u8 = 5;
const UNSUPPORTED: u8 = 7;
const ADDRESS_UNSUPPORTED: u8 = 8;

#[derive(Debug, Clone, Copy)]
pub struct Egress {
    address: SocketAddr,
}

impl Egress {
    pub fn address(&self) -> SocketAddr {
        self.address
    }
}

pub fn start() -> io::Result<Egress> {
    start_with(Arc::new(|name: &str, port: u16| Ok((name, port).to_socket_addrs()?.collect())), address::is_public)
}

pub fn start_with(resolve: Resolve, allow: fn(IpAddr) -> bool) -> io::Result<Egress> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let address = listener.local_addr()?;
    let open = Arc::new(AtomicUsize::new(0));
    std::thread::Builder::new().name("page-egress".into()).spawn(move || {
        for client in listener.incoming().flatten() {
            if open.fetch_add(1, Ordering::SeqCst) >= MOST {
                open.fetch_sub(1, Ordering::SeqCst);
                continue;
            }
            let (resolve, open) = (Arc::clone(&resolve), Arc::clone(&open));
            let spawned = std::thread::Builder::new().name("page-egress-link".into()).spawn(move || {
                serve(client, &resolve, allow);
                open.fetch_sub(1, Ordering::SeqCst);
            });
            if spawned.is_err() {
                tracing::warn!("no thread for a page's connection");
            }
        }
    })?;
    Ok(Egress { address })
}

enum Target {
    Address(SocketAddr),
    Name(String, u16),
}

fn serve(mut client: TcpStream, resolve: &Resolve, allow: fn(IpAddr) -> bool) {
    let _ = client.set_read_timeout(Some(PATIENCE));
    let _ = client.set_write_timeout(Some(PATIENCE));
    let target = match handshake(&mut client) {
        Ok(Ok(target)) => target,
        Ok(Err(code)) => return reply(&mut client, code),
        Err(_) => return,
    };
    let addresses = match target {
        Target::Address(address) => vec![address],
        Target::Name(name, port) => match resolve(&name, port) {
            Ok(addresses) => addresses,
            Err(_) => return reply(&mut client, UNREACHABLE),
        },
    };
    if addresses.is_empty() || !addresses.iter().all(|address| allow(address.ip())) {
        tracing::warn!("a page's connection to the server's own network was refused");
        return reply(&mut client, NOT_ALLOWED);
    }
    let Some(upstream) = addresses.iter().find_map(|address| TcpStream::connect_timeout(address, PATIENCE).ok()) else {
        return reply(&mut client, REFUSED);
    };
    reply(&mut client, SUCCEEDED);
    let _ = client.set_read_timeout(None);
    let _ = client.set_write_timeout(None);
    relay(client, upstream);
}

fn handshake(client: &mut TcpStream) -> io::Result<Result<Target, u8>> {
    let [version, methods] = read::<2>(client)?;
    let mut offered = vec![0; usize::from(methods)];
    client.read_exact(&mut offered)?;
    if version != 5 || !offered.contains(&0) {
        client.write_all(&[5, 0xff])?;
        return Err(io::ErrorKind::InvalidData.into());
    }
    client.write_all(&[5, 0])?;
    let [version, command, _, kind] = read::<4>(client)?;
    if version != 5 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let target = match kind {
        1 => Target::Address(SocketAddr::new(Ipv4Addr::from(read::<4>(client)?).into(), port(client)?)),
        4 => Target::Address(SocketAddr::new(Ipv6Addr::from(read::<16>(client)?).into(), port(client)?)),
        3 => {
            let [length] = read::<1>(client)?;
            let mut name = vec![0; usize::from(length)];
            client.read_exact(&mut name)?;
            let Ok(name) = String::from_utf8(name) else { return Ok(Err(ADDRESS_UNSUPPORTED)) };
            Target::Name(name, port(client)?)
        }
        _ => return Ok(Err(ADDRESS_UNSUPPORTED)),
    };
    Ok(if command == 1 { Ok(target) } else { Err(UNSUPPORTED) })
}

fn read<const N: usize>(stream: &mut TcpStream) -> io::Result<[u8; N]> {
    let mut bytes = [0; N];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn port(stream: &mut TcpStream) -> io::Result<u16> {
    Ok(u16::from_be_bytes(read::<2>(stream)?))
}

fn reply(client: &mut TcpStream, code: u8) {
    let _ = client.write_all(&[5, code, 0, 1, 0, 0, 0, 0, 0, 0]);
}

fn relay(client: TcpStream, upstream: TcpStream) {
    let (Ok(mut client_in), Ok(mut upstream_out)) = (client.try_clone(), upstream.try_clone()) else { return };
    let outgoing = std::thread::Builder::new().name("page-egress-out".into()).spawn(move || {
        let _ = io::copy(&mut client_in, &mut upstream_out);
        let _ = upstream_out.shutdown(Shutdown::Write);
    });
    let (mut upstream_in, mut client_out) = (upstream, client);
    let _ = io::copy(&mut upstream_in, &mut client_out);
    let _ = client_out.shutdown(Shutdown::Write);
    if let Ok(outgoing) = outgoing {
        let _ = outgoing.join();
    }
}

#[cfg(test)]
mod tests;
