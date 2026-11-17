use console::{Assets, Options};

fn started() -> console::Running {
    let options = Options { assets: Assets::Missing, port: 0, folder: std::env::temp_dir() };
    let Ok(running) = console::start(options) else {
        panic!("the console did not start");
    };
    running
}

fn entry(url: &str) -> &str {
    url.split_once("#enter=").map_or("", |(_, code)| code)
}

#[test]
fn the_link_carries_a_one_time_entry_code_in_its_fragment_never_the_token() {
    let running = started();
    let url = running.url();
    assert!(url.starts_with(&format!("http://{}/#enter=", running.address())), "{url}");
    assert!(!url.contains(running.token()), "{url}");
    assert!(entry(&url).len() >= 32);
    assert_ne!(entry(&url), entry(&running.url()), "each link has its own code");
    assert!(running.token().len() >= 32);
}

#[test]
fn a_link_can_point_at_one_page() {
    let running = started();
    let url = running.url_at("/docs");
    assert!(url.starts_with(&format!("http://{}/docs#enter=", running.address())), "{url}");
    let searching = running.url_at("/docs?q=hooks");
    assert!(searching.starts_with(&format!("http://{}/docs?q=hooks#enter=", running.address())), "{searching}");
}

#[test]
fn the_link_s_code_enters_once() {
    use std::io::{Read, Write};
    let running = started();
    let code = entry(&running.url()).to_owned();
    let enter = || {
        let mut stream = std::net::TcpStream::connect(running.address()).expect("connect");
        let request = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\n{}: {code}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            console::ENTER_PATH,
            running.address(),
            console::ENTRY_HEADER
        );
        stream.write_all(request.as_bytes()).expect("request");
        let mut answer = String::new();
        stream.read_to_string(&mut answer).expect("answer");
        answer
    };
    let first = enter();
    assert!(first.starts_with("HTTP/1.1 200") && first.contains(running.token()), "{first}");
    assert!(enter().starts_with("HTTP/1.1 401"));
}
