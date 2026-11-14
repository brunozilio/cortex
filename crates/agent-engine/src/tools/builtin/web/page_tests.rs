use super::*;

fn read(body: &str, content_type: Option<&str>, url: &str) -> Result<String, Unreadable> {
    super::read(body, content_type, url, WebReading::default())
}

fn prose(words: usize) -> String {
    (0..words).map(|n| format!("word{n}")).collect::<Vec<_>>().join(" ")
}

fn html(page: &str) -> String {
    read(page, Some("text/html; charset=utf-8"), "https://docs.example.dev/guide/install").unwrap_or_else(|e| e.0)
}

#[test]
fn the_main_content_comes_first_and_the_menus_after_it() {
    let page = format!(
        "<html><body><header><a href='/'>Home</a> <a href='/blog'>Blog</a></header>\
         <nav><ul><li><a href='/a'>Menu A</a></li><li><a href='/b'>Menu B</a></li></ul></nav>\
         <main><h1>Install</h1><p>{}</p></main><footer>Copyright footer</footer></body></html>",
        prose(700)
    );
    let text = html(&page);
    assert!(text.starts_with("# Install\n"), "{text}");
    let main = text.find("word699").unwrap_or(usize::MAX);
    assert!(main < text.find("Menu A").unwrap_or(0), "menus follow the content: {text}");
    assert!(main < text.find("Copyright footer").unwrap_or(0), "{text}");
}

#[test]
fn links_are_numbered_once_and_listed_at_the_end() {
    let page = "<body><p>See <a href='/guide/setup?utm_source=x'>setup</a>, \
        <a href='https://docs.example.dev/guide/setup'>the same setup</a>, \
        <a href='https://crates.io/serde'>serde</a>, <a href='#top'>top</a> and \
        <a href='javascript:void(0)'>menu</a>.</p></body>";
    assert_eq!(
        html(page),
        "See setup [1], the same setup [1], serde [2], top and menu.\n\n\
         Links (paths are on https://docs.example.dev):\n[1] /guide/setup\n[2] https://crates.io/serde"
    );
}

#[test]
fn hidden_parts_and_controls_repeated_on_every_item_are_dropped() {
    let rows: String = (0..6)
        .map(|n| format!("<li>Result {n} explained <a href='/share/{n}'>Share</a> <button>Like</button></li>"))
        .collect();
    let page = format!(
        "<body><div hidden>secret</div><p style='display: none'>gone</p><span aria-hidden='true'>icon</span>\
         <select><option>Brazil</option></select><ul>{rows}</ul></body>"
    );
    let text = html(&page);
    for absent in ["secret", "gone", "icon", "Brazil", "Like", "Share", "[1]"] {
        assert!(!text.contains(absent), "{absent} in {text}");
    }
    assert!(text.contains("- Result 5 explained"), "{text}");
}

#[test]
fn long_runs_of_menu_links_outside_the_content_shrink_to_one_line() {
    let menu: String = (0..30).map(|n| format!("<li><a href='/topic/{n}'>Topic {n}</a></li>")).collect();
    let page = format!("<body><nav><ul>{menu}</ul></nav><main><h1>Guide</h1><p>{}</p></main></body>", prose(700));
    let text = html(&page);
    assert!(text.contains("\nTopic 0 · Topic 1 · Topic 2"), "{text}");
    assert!(text.contains("(+22 more links)") && !text.contains("Topic 29"), "{text}");
    assert!(!text.contains("Links"), "links around the main content carry no address: {text}");
}

#[test]
fn a_cut_page_lists_only_the_links_it_kept_and_long_addresses_are_left_out() {
    let items: String = (0..300).map(|n| format!("<p>Item <a href='/item/{n}'>number {n}</a></p>")).collect();
    let page = format!("<body><p><a href='https://play.dev/?code={}'>Run</a></p>{items}</body>", "x".repeat(300));
    let text = read(&page, Some("text/html"), "https://x.dev/").unwrap_or_else(|e| e.0);
    let short = clipped(&text, 2_000);
    assert!(short.chars().count() <= 2_100, "{}", short.chars().count());
    assert!(short.starts_with("Run\n\nItem number 0 [1]"), "{short}");
    assert!(short.contains("more characters cut") && short.contains("\n[1] /item/0\n"), "{short}");
    let last = short.lines().last().unwrap_or("");
    assert!(last.starts_with('[') && !short.contains("[300]"), "{last}");
}

#[test]
fn a_main_tag_holding_its_own_menus_narrows_to_the_article_column() {
    let tools: String = (0..12).map(|n| format!("<li><a href='/tool/{n}'>Tool {n}</a></li>")).collect();
    let page = format!(
        "<body><main><div class='tools'><ul>{tools}</ul></div>\
         <div class='body'><h1>Rust</h1><p>{}</p></div></main></body>",
        prose(700)
    );
    assert!(html(&page).starts_with("# Rust\n"), "{}", html(&page));
}

#[test]
fn empty_table_cells_and_source_line_breaks_leave_no_trace() {
    let page = "<body><table><tr><td></td><td class='v'> </td><td>1.</td><td><a href='/x'>Story</a>\n(site)</td></tr>\
        <tr><td></td><td>42 points</td></tr></table><p>One sentence\nbroken in the source.</p></body>";
    assert_eq!(
        html(page),
        "1. | Story [1] (site)\n42 points\n\nOne sentence broken in the source.\n\n\
         Links (paths are on https://docs.example.dev):\n[1] /x"
    );
}

#[test]
fn the_title_leads_a_page_that_starts_without_a_heading_and_footnote_marks_go() {
    let page = "<html><head><title>Rust - Wikipedia</title></head><body>\
        <p>Rust is fast.<sup><a href='#cite_note-1'>[1]</a></sup></p></body></html>";
    assert_eq!(html(page), "# Rust - Wikipedia\n\nRust is fast.");
    let headed = "<html><head><title>Site</title></head><body><h1>Install</h1></body></html>";
    assert_eq!(html(headed), "# Install");
}

#[test]
fn a_list_item_holding_a_block_stays_on_one_line() {
    assert_eq!(html("<body><ul><li><p>Try it</p></li><li><div>Syntax</div></li></ul></body>"), "- Try it\n- Syntax");
}

#[test]
fn code_keeps_its_lines_and_indentation() {
    let page = "<body><pre><code><span>fn</span> main() {\n    <span>run</span>();\n}</code></pre></body>";
    assert_eq!(html(page), "```\nfn main() {\n    run();\n}\n```");
}

#[test]
fn a_json_list_is_one_line_per_record_with_shared_fields_said_once() {
    let body = r#"{"total": 2, "items": [
        {"name": "serde", "version": "1.0.2", "license": "MIT", "owner": {"login": "dtolnay"}, "yanked": null, "tags": ["a", "b"]},
        {"name": "toml", "version": "0.8.0", "license": "MIT", "owner": {"login": "epage"}, "yanked": null, "tags": []}
    ]}"#;
    assert_eq!(
        read(body, Some("application/json"), "https://x.dev/api").unwrap_or_else(|e| e.0),
        "total: 2\nevery item: license=MIT\n\
         [1] name=serde; owner.login=dtolnay; tags=a, b; version=1.0.2\n\
         [2] name=toml; owner.login=epage; version=0.8.0"
    );
}

#[test]
fn a_json_object_loses_its_indentation_and_nulls() {
    let body = "{\n  \"name\": \"serde\",\n  \"homepage\": null,\n  \"deps\": {\n    \"std\": \"1\"\n  }\n}";
    assert_eq!(read(body, None, "https://x.dev/api").unwrap_or_else(|e| e.0), r#"{"deps":{"std":"1"},"name":"serde"}"#);
}

#[test]
fn a_feed_reads_as_its_entries() {
    let rss = "<?xml version='1.0'?><rss><channel><title>Blog</title><item><title>Release 2.0</title>\
        <link>https://blog.dev/2</link><pubDate>Tue, 10 Nov 2026 10:00:00 GMT</pubDate>\
        <description><![CDATA[<p>Faster <b>builds</b>.</p>]]></description></item></channel></rss>";
    assert_eq!(
        read(rss, Some("application/rss+xml"), "https://blog.dev/feed").unwrap_or_else(|e| e.0),
        "# Blog\n\n- Release 2.0 (Tue, 10 Nov 2026 10:00:00 GMT) https://blog.dev/2\n  Faster builds."
    );
    let atom = "<feed xmlns='http://www.w3.org/2005/Atom'><title>Log</title><entry><title>One</title>\
        <link href='https://log.dev/1'/><updated>2026-11-01</updated><summary>Short.</summary></entry></feed>";
    assert_eq!(
        read(atom, None, "https://log.dev/atom").unwrap_or_else(|e| e.0),
        "# Log\n\n- One (2026-11-01) https://log.dev/1\n  Short."
    );
}

#[test]
fn without_a_main_tag_the_column_holding_the_prose_leads() {
    let links: String = (0..40).map(|n| format!("<li><a href='/t/{n}'>Topic {n}</a></li>")).collect();
    let page = format!(
        "<body><div id='layout'><div class='side'><ul>{links}</ul></div>\
         <div class='content'><div class='inner'><h2>Usage</h2><p>{}</p></div></div></div></body>",
        prose(700)
    );
    let text = html(&page);
    assert!(text.starts_with("## Usage\n"), "{text}");
}
