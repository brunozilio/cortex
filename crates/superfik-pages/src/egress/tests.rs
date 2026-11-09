//! The proxy over real loopback sockets, with names resolved by a table:
//! what it lets through, and what it refuses.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;

use super::*;

fn echo() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut buffer = [0; 64];
                while let Ok(n @ 1..) = stream.read(&mut buffer) {
                    if stream.write_all(&buffer[..n]).is_err() {
                        return;
                    }
                }
            });
        }
    });
    address
}

fn proxy(table: Vec<(&'static str, Vec<SocketAddr>)>, allow: fn(std::net::IpAddr) -> bool) -> SocketAddr {
    let resolve: Resolve = Arc::new(move |name: &str, _port: u16| {
        table
            .iter()
            .find(|(known, _)| *known == name)
            .map(|(_, addresses)| addresses.clone())
            .ok_or_else(|| std::io::Error::other("unknown name"))
    });
    start_with(resolve, allow).expect("proxy").address()
}

fn connect(proxy: SocketAddr, command: u8, target: &[u8]) -> (TcpStream, u8) {
    let mut stream = TcpStream::connect(proxy).expect("connect");
    stream.write_all(&[5, 1, 0]).expect("greet");
    let mut chosen = [0; 2];
    stream.read_exact(&mut chosen).expect("method");
    assert_eq!(chosen, [5, 0], "no authentication");
    stream.write_all(&[&[5, command, 0][..], target].concat()).expect("request");
    let mut reply = [0; 10];
    stream.read_exact(&mut reply).expect("reply");
    (stream, reply[1])
}

fn name(name: &str, port: u16) -> Vec<u8> {
    let mut target = vec![3, u8::try_from(name.len()).expect("short")];
    target.extend(name.as_bytes());
    target.extend(port.to_be_bytes());
    target
}

#[test]
fn a_public_destination_is_reached() {
    let server = echo();
    let proxy = proxy(vec![("public.test", vec![server])], |_| true);
    let (mut stream, reply) = connect(proxy, 1, &name("public.test", server.port()));
    assert_eq!(reply, 0, "succeeded");
    stream.write_all(b"hello").expect("write");
    let mut heard = [0; 5];
    stream.read_exact(&mut heard).expect("read");
    assert_eq!(&heard, b"hello");
}

#[test]
fn a_name_with_any_address_on_the_servers_network_is_refused() {
    let server = echo();
    let public: SocketAddr = "93.184.216.34:443".parse().expect("address");
    let proxy = proxy(vec![("rebind.test", vec![public, server])], superfik_browser::address::is_public);
    assert_eq!(connect(proxy, 1, &name("rebind.test", 443)).1, NOT_ALLOWED);
}

#[test]
fn a_literal_metadata_address_is_refused() {
    let proxy = proxy(Vec::new(), superfik_browser::address::is_public);
    let metadata = [1, 169, 254, 169, 254, 0, 80];
    assert_eq!(connect(proxy, 1, &metadata).1, NOT_ALLOWED);
    let loopback_v6 = [&[4][..], &std::net::Ipv6Addr::LOCALHOST.octets(), &[0, 80]].concat();
    assert_eq!(connect(proxy, 1, &loopback_v6).1, NOT_ALLOWED);
}

#[test]
fn only_connect_is_offered() {
    let proxy = proxy(vec![("public.test", vec![echo()])], |_| true);
    assert_eq!(connect(proxy, 2, &name("public.test", 80)).1, UNSUPPORTED, "no BIND");
    assert_eq!(connect(proxy, 3, &name("public.test", 80)).1, UNSUPPORTED, "no UDP");
}
