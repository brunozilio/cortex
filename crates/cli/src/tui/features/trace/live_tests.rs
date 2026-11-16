use std::io::Read;

use super::*;

fn address(live: &Live) -> String {
    live.url.trim_start_matches("http://").split('/').next().unwrap_or_default().to_owned()
}

fn get(live: &Live, target: &str) -> TcpStream {
    let mut stream = TcpStream::connect(address(live)).expect("connect");
    write!(stream, "GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").expect("request");
    stream
}

fn post(live: &Live, target: &str, origin: &str, body: &str) -> String {
    let mut stream = TcpStream::connect(address(live)).expect("connect");
    write!(
        stream,
        "POST {target} HTTP/1.1\r\nHost: localhost\r\nOrigin: {origin}\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .expect("request");
    read_until(&mut stream, "\r\n\r\n")
}

fn own(live: &Live) -> String {
    format!("http://{}", address(live))
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
fn an_open_stream_gets_the_latest_update_of_each_session_and_every_one_after() {
    let live = Live::start().expect("start");
    live.send("s", "{\"id\":\"s\",\"runs\":[]}");
    live.send("t", "{\"id\":\"t\",\"runs\":[]}");
    let mut events = get(&live, &live.events());
    read_until(&mut events, "data: {\"id\":\"t\",\"runs\":[]}\n\n");
    live.send("s", "{\"id\":\"s\",\"runs\":[1]}");
    read_until(&mut events, "data: {\"id\":\"s\",\"runs\":[1]}\n\n");
}

#[test]
fn the_sessions_listed_are_read_back_by_the_page() {
    let live = Live::start().expect("start");
    let mut empty = get(&live, &format!("/sessions?token={}", live.token));
    assert!(read_until(&mut empty, "[]").starts_with("HTTP/1.1 200"));
    live.list("[{\"session\":\"s\"}]".into());
    let mut listed = get(&live, &format!("/sessions?token={}", live.token));
    read_until(&mut listed, "[{\"session\":\"s\"}]");
}

#[test]
fn a_prompt_from_the_page_waits_to_be_taken_once() {
    let live = Live::start().expect("start");
    let sent =
        post(&live, &format!("/prompt?token={}", live.token), &own(&live), "{\"session\":\"s\",\"text\":\" hi \"}");
    assert!(sent.starts_with("HTTP/1.1 202"), "{sent}");
    assert_eq!(live.take_prompts(), [Prompt { session: "s".into(), text: "hi".into() }]);
    assert!(live.take_prompts().is_empty());
}

#[test]
fn a_prompt_needs_the_token_the_page_origin_and_some_text() {
    let live = Live::start().expect("start");
    let body = "{\"session\":\"s\",\"text\":\"hi\"}";
    assert!(post(&live, "/prompt?token=guess", &own(&live), body).starts_with("HTTP/1.1 404"));
    let foreign = post(&live, &format!("/prompt?token={}", live.token), "http://evil.test", body);
    assert!(foreign.starts_with("HTTP/1.1 404"));
    let blank =
        post(&live, &format!("/prompt?token={}", live.token), &own(&live), "{\"session\":\"s\",\"text\":\"  \"}");
    assert!(blank.starts_with("HTTP/1.1 404"));
    assert!(live.take_prompts().is_empty());
}

#[test]
fn the_page_names_the_session_it_watches() {
    let live = Live::start().expect("start");
    assert_eq!(live.watched(), None);
    post(&live, &format!("/watch?token={}", live.token), &own(&live), "{\"session\":\"s\"}");
    assert_eq!(live.watched().as_deref(), Some("s"));
}
