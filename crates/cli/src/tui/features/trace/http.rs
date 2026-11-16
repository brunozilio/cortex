use std::io::{BufRead, BufReader};
use std::net::TcpStream;

const HEADERS_AT_MOST: usize = 64;
pub const BODY_AT_MOST: usize = 256 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub token: Option<String>,
    pub origin: Option<String>,
    pub body: Vec<u8>,
}

pub fn read(stream: &TcpStream) -> Option<Request> {
    parse(BufReader::new(stream.try_clone().ok()?))
}

pub fn parse(mut reader: impl BufRead) -> Option<Request> {
    let mut first = String::new();
    reader.read_line(&mut first).ok()?;
    let mut words = first.split_whitespace();
    let method = words.next()?.to_owned();
    let target = words.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let token = query.split('&').find_map(|pair| pair.strip_prefix("token=")).map(str::to_owned);
    let (mut length, mut origin) = (0, None);
    for _ in 0..HEADERS_AT_MOST {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? <= 2 {
            break;
        }
        let Some((name, value)) = line.split_once(':') else { continue };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => length = value.parse().ok()?,
            "origin" => origin = Some(value.to_owned()),
            _ => {}
        }
    }
    if length > BODY_AT_MOST {
        return None;
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Request { method, path: path.to_owned(), token, origin, body })
}

pub fn reply(status: &str, kind: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> Option<Request> {
        parse(text.as_bytes())
    }

    #[test]
    fn a_request_names_its_method_path_token_origin_and_body() {
        let request = parsed(
            "POST /prompt?token=ab HTTP/1.1\r\nHost: x\r\nOrigin: http://127.0.0.1:9\r\nContent-Length: 4\r\n\r\nhi!!",
        )
        .expect("request");
        assert_eq!(
            request,
            Request {
                method: "POST".into(),
                path: "/prompt".into(),
                token: Some("ab".into()),
                origin: Some("http://127.0.0.1:9".into()),
                body: b"hi!!".to_vec(),
            }
        );
    }

    #[test]
    fn a_request_without_a_token_or_a_body_is_still_read() {
        let request = parsed("GET / HTTP/1.1\r\n\r\n").expect("request");
        assert_eq!((request.token, request.body.len()), (None, 0));
    }

    #[test]
    fn a_body_too_large_or_cut_short_is_refused() {
        assert!(parsed(&format!("POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n", BODY_AT_MOST + 1)).is_none());
        assert!(parsed("POST / HTTP/1.1\r\nContent-Length: 10\r\n\r\nshort").is_none());
    }
}
