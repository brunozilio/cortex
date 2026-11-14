use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use platform::sync::Locked;

const HEADERS_AT_MOST: usize = 64;

#[derive(Default)]
struct Shared {
    page: String,
    latest: Option<String>,
    clients: Vec<TcpStream>,
}

pub struct Live {
    pub url: String,
    token: String,
    shared: Arc<Mutex<Shared>>,
}

fn token() -> String {
    let half = || {
        let mut hasher = RandomState::new().build_hasher();
        hasher
            .write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        hasher.finish()
    };
    format!("{:016x}{:016x}", half(), half())
}

impl Live {
    pub fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let token = token();
        let url = format!("http://127.0.0.1:{}/?token={token}", listener.local_addr()?.port());
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (serving, expected) = (Arc::clone(&shared), token.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (shared, expected) = (Arc::clone(&serving), expected.clone());
                std::thread::spawn(move || answer(stream, &expected, &shared));
            }
        });
        Ok(Self { url, token, shared })
    }

    pub fn events(&self) -> String {
        format!("/events?token={}", self.token)
    }

    pub fn show(&self, page: String) {
        self.shared.locked().page = page;
    }

    pub fn send(&self, data: &str) {
        let mut shared = self.shared.locked();
        let message = format!("data: {}\n\n", data.replace('\n', " "));
        shared.clients.retain_mut(|client| client.write_all(message.as_bytes()).and_then(|()| client.flush()).is_ok());
        shared.latest = Some(message);
    }
}

fn requested(stream: &TcpStream) -> Option<(String, String)> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut first = String::new();
    reader.read_line(&mut first).ok()?;
    for _ in 0..HEADERS_AT_MOST {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? <= 2 {
            break;
        }
    }
    let target = first.strip_prefix("GET ")?.split_whitespace().next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let token = query.split('&').find_map(|pair| pair.strip_prefix("token="))?;
    Some((path.to_owned(), token.to_owned()))
}

fn answer(mut stream: TcpStream, expected: &str, shared: &Mutex<Shared>) {
    let found = requested(&stream).filter(|(_, token)| token == expected).map(|(path, _)| path);
    let _ = match found.as_deref() {
        Some("/") => {
            let page = shared.locked().page.clone();
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
                page.len()
            );
            stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(page.as_bytes()))
        }
        Some("/events") => {
            let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n";
            let mut shared = shared.locked();
            let latest = shared.latest.clone().unwrap_or_default();
            let opened = stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(latest.as_bytes()));
            if opened.is_ok() && stream.flush().is_ok() {
                shared.clients.push(stream);
            }
            Ok(())
        }
        _ => stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
    };
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    fn get(live: &Live, target: &str) -> TcpStream {
        let address = live.url.trim_start_matches("http://").split('/').next().unwrap_or_default().to_owned();
        let mut stream = TcpStream::connect(address).expect("connect");
        write!(stream, "GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").expect("request");
        stream
    }

    fn read_until(stream: &mut TcpStream, needle: &str) -> String {
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).expect("timeout");
        let mut seen = String::new();
        let mut chunk = [0u8; 1024];
        while !seen.contains(needle) {
            let read = stream.read(&mut chunk).expect("read");
            assert!(read > 0, "closed before {needle:?}: {seen}");
            seen.push_str(&String::from_utf8_lossy(&chunk[..read]));
        }
        seen
    }

    #[test]
    fn the_page_is_served_only_with_the_token_and_only_on_loopback() {
        let live = Live::start().expect("start");
        assert!(live.url.starts_with("http://127.0.0.1:"));
        live.show("<p>trace</p>".into());
        assert!(live.url.ends_with(&format!("/?token={}", live.token)));
        let mut page = get(&live, &format!("/?token={}", live.token));
        assert!(read_until(&mut page, "<p>trace</p>").starts_with("HTTP/1.1 200"));
        let mut stranger = get(&live, "/?token=guess");
        assert!(read_until(&mut stranger, "\r\n\r\n").starts_with("HTTP/1.1 404"));
    }

    #[test]
    fn an_open_stream_gets_the_latest_update_and_every_one_after() {
        let live = Live::start().expect("start");
        live.send("{\"id\":\"s\",\"runs\":[]}");
        let mut events = get(&live, &live.events());
        read_until(&mut events, "data: {\"id\":\"s\",\"runs\":[]}\n\n");
        live.send("{\"id\":\"s\",\"runs\":[1]}");
        read_until(&mut events, "data: {\"id\":\"s\",\"runs\":[1]}\n\n");
    }
}
