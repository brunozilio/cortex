use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

use crate::model::http::{Http, NetHttp, Response};
use crate::model::{CancellationToken, ModelError};

const STILL_SENDING: std::time::Duration = std::time::Duration::from_secs(120);

fn one_hop(url: &str) -> Result<Response, ModelError> {
    platform::runtime::block_on(NetHttp.one_hop(url, &[], None, &CancellationToken::new())).expect("runtime")
}

fn serve(answer: impl Fn(&mut TcpStream) + Send + 'static) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                line.clear();
            }
            answer(&mut stream);
        }
    });
    port
}

fn text(status: &str, extra: &str, body: &str) -> String {
    format!("HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n{extra}\r\n{body}", body.len())
}

#[test]
fn a_redirect_is_handed_back_instead_of_followed() {
    let inner = serve(|stream| {
        let _ = stream.write_all(text("200 OK", "", "inner").as_bytes());
    });
    let outer = serve(move |stream| {
        let location = format!("location: http://127.0.0.1:{inner}/\r\n");
        let _ = stream.write_all(text("302 Found", &location, "moved").as_bytes());
    });
    let response = one_hop(&format!("http://127.0.0.1:{outer}/")).expect("answer");
    assert_eq!(response.status, 302, "the caller decides whether the next address may be read");
}

#[test]
fn a_name_that_resolves_to_this_computer_is_not_connected_to() {
    let port = serve(|stream| {
        let _ = stream.write_all(text("200 OK", "", "local").as_bytes());
    });
    let error = one_hop(&format!("http://localhost:{port}/")).expect_err("refused").to_string();
    assert!(error.contains("private"), "{error}");
}

#[test]
fn a_body_without_end_is_read_only_up_to_the_cap() {
    let (hung_up, hangs_up) = crossbeam_channel::unbounded();
    let port = serve(move |stream| {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nconnection: close\r\n\r\n");
        let chunk = [b'a'; 16 * 1024];
        while stream.write_all(&chunk).is_ok() {}
        let _ = hung_up.send(());
    });
    let response = one_hop(&format!("http://127.0.0.1:{port}/")).expect("answer");
    let read = platform::runtime::block_on(response.body.capped(&CancellationToken::new(), 64 * 1024))
        .expect("runtime")
        .expect("read");
    assert_eq!(read.len(), 64 * 1024);
    hangs_up.recv_timeout(STILL_SENDING).expect("the endless body was let go of once the cap was read");
}

#[test]
fn through_a_proxy_a_name_is_still_resolved_and_a_private_target_refused_before_the_proxy_is_asked() {
    let proxy = TcpListener::bind("127.0.0.1:0").expect("proxy");
    proxy.set_nonblocking(true).expect("nonblocking");
    let to_this_computer = |_: &str| Ok(vec!["127.0.0.1".parse().expect("address")]);
    let found = super::addresses_for("intranet.dev", true, &to_this_computer).expect("resolved");
    let why = super::private_among("intranet.dev", &found).expect("refused");
    assert!(why.contains("127.0.0.1"), "{why}");

    for literal in ["10.0.0.2", "[::1]", "169.254.169.254"] {
        let found = super::addresses_for(literal, true, &|_| Err(std::io::Error::other("no dns"))).expect("literal");
        assert!(super::private_among(literal, &found).is_some(), "{literal}");
    }
    let unresolvable = super::addresses_for("public.dev", true, &|_| Err(std::io::Error::other("no dns")));
    assert_eq!(unresolvable, Ok(Vec::new()), "behind a proxy without DNS the proxy resolves the name");
    assert!(super::addresses_for("public.dev", false, &|_| Err(std::io::Error::other("no dns"))).is_err());
    assert!(proxy.accept().is_err(), "the proxy was never asked");
    let found = super::addresses_for("localhost", true, &|host| {
        (host, 443).to_socket_addrs().map(|found| found.map(|address| address.ip()).collect())
    });
    assert!(super::private_among("localhost", &found.expect("localhost")).is_some());
}

#[test]
fn an_address_list_with_one_private_address_is_private() {
    let addresses = ["93.184.216.34".parse().expect("address"), "10.0.0.7".parse().expect("address")];
    let why = super::private_among("mixed.dev", &addresses).expect("private");
    assert!(why.contains("mixed.dev") && why.contains("10.0.0.7"), "{why}");
    assert_eq!(super::private_among("fine.dev", &addresses[..1]), None);
}
