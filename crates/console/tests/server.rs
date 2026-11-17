use console::{Assets, Options};

fn started() -> console::Running {
    let options = Options { assets: Assets::Missing, port: 0, folder: std::env::temp_dir() };
    let Ok(running) = console::start(options) else {
        panic!("the console did not start");
    };
    running
}

#[test]
fn the_link_carries_the_token_and_points_at_localhost() {
    let running = started();
    let url = running.url();
    assert!(url.starts_with("http://127.0.0.1:"), "{url}");
    assert!(url.ends_with(&format!("/?token={}", running.token())), "{url}");
    assert!(running.token().len() >= 32);
}

#[test]
fn a_link_can_point_at_one_page() {
    let running = started();
    let url = running.url_at("/docs");
    assert_eq!(url, format!("http://{}/docs?token={}", running.address(), running.token()));
    let searching = running.url_at("/docs?q=hooks");
    assert_eq!(searching, format!("http://{}/docs?q=hooks&token={}", running.address(), running.token()));
}
