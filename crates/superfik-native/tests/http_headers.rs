//! What `UreqHttp` puts on the wire, read by a one-shot local server.
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use superfik_native::model::http::{Http, UreqHttp};

fn sent_headers(headers: &[(String, String)]) -> Vec<String> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}/token", listener.local_addr().expect("addr"));
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("line");
            if line.trim().is_empty() {
                break;
            }
            lines.push(line.trim().to_lowercase());
        }
        let mut body = [0u8; 3];
        std::io::Read::read_exact(&mut reader, &mut body).expect("body");
        let mut stream = stream;
        stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}").expect("answer");
        lines
    });
    UreqHttp.post(&url, headers, "a=b").expect("posted");
    server.join().expect("server")
}

fn content_types(lines: &[String]) -> Vec<&String> {
    lines.iter().filter(|line| line.starts_with("content-type:")).collect()
}

#[test]
fn a_given_content_type_replaces_the_json_default() {
    let form = [("content-type".to_owned(), "application/x-www-form-urlencoded".to_owned())];
    let lines = sent_headers(&form);
    assert_eq!(content_types(&lines), ["content-type: application/x-www-form-urlencoded"]);
}

#[test]
fn json_is_the_default() {
    let lines = sent_headers(&[]);
    assert_eq!(content_types(&lines), ["content-type: application/json"]);
}
