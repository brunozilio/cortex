use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use crate::model::http::{Http, NetHttp};

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
    let response = NetHttp.one_hop(&format!("http://127.0.0.1:{outer}/"), &[], None).expect("answer");
    assert_eq!(response.status, 302, "the caller decides whether the next address may be read");
}

#[test]
fn a_name_that_resolves_to_this_computer_is_not_connected_to() {
    let port = serve(|stream| {
        let _ = stream.write_all(text("200 OK", "", "local").as_bytes());
    });
    let error = NetHttp.one_hop(&format!("http://localhost:{port}/"), &[], None).expect_err("refused");
    assert!(error.contains("private"), "{error}");
}

#[test]
fn a_body_without_end_is_read_only_up_to_the_cap() {
    let port = serve(|stream| {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nconnection: close\r\n\r\n");
        let chunk = [b'a'; 16 * 1024];
        while stream.write_all(&chunk).is_ok() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
    let started = std::time::Instant::now();
    let mut response = NetHttp.one_hop(&format!("http://127.0.0.1:{port}/"), &[], None).expect("answer");
    let read = super::read_capped(response.body.as_mut(), 64 * 1024).expect("read");
    drop(response);
    assert_eq!(read.len(), 64 * 1024);
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "the whole body was waited for");
}

#[test]
fn a_short_body_is_read_whole_and_a_cap_of_nothing_reads_nothing() {
    let mut body = std::io::Cursor::new(b"hello".to_vec());
    assert_eq!(super::read_capped(&mut body, 1024).expect("read"), b"hello");
    let mut body = std::io::Cursor::new(b"hello".to_vec());
    assert_eq!(super::read_capped(&mut body, 3).expect("read"), b"hel");
    assert!(super::read_capped(&mut body, 0).expect("read").is_empty());
}

#[test]
fn an_address_list_with_one_private_address_is_private() {
    let addresses = ["93.184.216.34".parse().expect("address"), "10.0.0.7".parse().expect("address")];
    let why = super::private_among("mixed.dev", &addresses).expect("private");
    assert!(why.contains("mixed.dev") && why.contains("10.0.0.7"), "{why}");
    assert_eq!(super::private_among("fine.dev", &addresses[..1]), None);
}
