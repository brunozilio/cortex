use domain::web_reading::WebReading;

use super::*;

const URL: &str = "https://docs.example.dev/guide/install";

fn with(change: impl Fn(&mut WebReading), body: &str, content_type: Option<&str>) -> String {
    let mut reading = WebReading::default();
    change(&mut reading);
    read(body, content_type, URL, reading).unwrap_or_else(|e| e.0)
}

fn prose(words: usize) -> String {
    (0..words).map(|n| format!("word{n}")).collect::<Vec<_>>().join(" ")
}

#[test]
fn without_main_content_the_page_reads_in_its_own_order() {
    let page = format!("<body><nav><p>Menu first</p></nav><main><h1>Install</h1><p>{}</p></main></body>", prose(700));
    let text = with(|r| r.main_content = false, &page, Some("text/html"));
    assert!(text.starts_with("Menu first\n\n# Install"), "{text}");
    assert!(!text.contains("around the main content"), "{text}");
}

#[test]
fn without_cleanup_hidden_parts_buttons_and_repeats_stay() {
    let rows: String = (0..6).map(|n| format!("<li>Row {n} <a href='/s/{n}'>Share</a></li>")).collect();
    let page = format!("<body><div hidden>secret</div><button>Like</button><ul>{rows}</ul></body>");
    let text = with(|r| r.cleanup = false, &page, Some("text/html"));
    for kept in ["secret", "Like", "Row 5 Share"] {
        assert!(text.contains(kept), "{kept} missing from {text}");
    }
}

#[test]
fn without_the_link_list_addresses_stay_beside_their_text() {
    let page = "<body><p>See <a href='/guide/setup'>setup</a> and <a href='/guide/setup'>again</a>.</p></body>";
    assert_eq!(
        with(|r| r.links = false, page, Some("text/html")),
        "See setup [https://docs.example.dev/guide/setup] and again [https://docs.example.dev/guide/setup]."
    );
}

#[test]
fn without_formats_json_and_feeds_come_back_as_sent() {
    let json = "{\n  \"name\": \"serde\",\n  \"homepage\": null\n}";
    assert_eq!(with(|r| r.formats = false, json, Some("application/json")), json);
    let rss = "<rss><channel><title>Blog</title></channel></rss>";
    assert_eq!(with(|r| r.formats = false, rss, Some("application/rss+xml")), rss);
}

#[test]
fn without_the_empty_page_error_a_bare_shell_is_read_as_it_is() {
    let shell =
        format!("<html><head>{}</head><body><div id='root'></div></body></html>", "<script>x</script>".repeat(800));
    assert_eq!(with(|r| r.empty_page_error = false, &shell, Some("text/html")), "");
    assert!(read(&shell, Some("text/html"), URL, WebReading::default()).is_err());
}
