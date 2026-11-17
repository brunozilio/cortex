#![no_main]

use agent_engine::tools::builtin::web::read_page;
use domain::web_reading::WebReading;
use libfuzzer_sys::fuzz_target;

const CONTENT_TYPES: [Option<&str>; 8] = [
    None,
    Some("text/html; charset=utf-8"),
    Some("application/json"),
    Some("application/rss+xml"),
    Some("application/atom+xml"),
    Some("text/plain"),
    Some("text/markdown"),
    Some("application/xhtml+xml"),
];

fuzz_target!(|data: &[u8]| {
    let Some((&flags, rest)) = data.split_first() else { return };
    let Some((&kind, body)) = rest.split_first() else { return };
    let on = |bit: u8| flags & (1 << bit) != 0;
    let reading = WebReading {
        main_content: on(0),
        cleanup: on(1),
        links: on(2),
        small_pages_direct: on(3),
        formats: on(4),
        empty_page_error: on(5),
    };
    let content_type = CONTENT_TYPES[usize::from(kind) % CONTENT_TYPES.len()];
    let body = String::from_utf8_lossy(body);
    let _ = read_page(&body, content_type, "https://example.test/a/page.html", reading);
});
