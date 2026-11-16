use console::{Assets, Options};

#[test]
fn the_link_carries_the_token_and_points_at_localhost() {
    let Ok(running) = console::start(Options { assets: Assets::Missing, port: 0 }) else {
        panic!("the console did not start");
    };
    let url = running.url();
    assert!(url.starts_with("http://127.0.0.1:"), "{url}");
    assert!(url.ends_with(&format!("/?token={}", running.token())), "{url}");
    assert!(running.token().len() >= 32);
}
