use domain::web_reading::WebReading;

use super::tree::{Element, Syntax, parse};

const SUMMARY: usize = 300;

pub fn is_feed(body: &str) -> bool {
    let head = body.trim_start().chars().take(512).collect::<String>().to_ascii_lowercase();
    head.contains("<rss") || head.contains("<feed") || head.contains("<rdf:rdf")
}

pub fn read(body: &str) -> String {
    let root = parse(body, Syntax::Xml);
    let channel = root.first("channel").or(root.first("feed")).unwrap_or(&root);
    let title = channel.elements().find(|element| element.name == "title").map(Element::text);
    let entries: Vec<String> =
        root.find(&|element| element.name == "item" || element.name == "entry").into_iter().map(entry).collect();
    let mut out = title.map(|title| format!("# {title}\n\n")).unwrap_or_default();
    out.push_str(&entries.join("\n"));
    out.trim_end().to_owned()
}

fn entry(item: &Element) -> String {
    let field = |names: &[&str]| item.elements().find(|element| names.contains(&element.name.as_str()));
    let title = field(&["title"]).map(Element::text).unwrap_or_default();
    let date = field(&["pubdate", "published", "updated", "date"]).map(Element::text);
    let link = item
        .elements()
        .filter(|element| element.name == "link")
        .find(|link| link.attr("rel").is_none_or(|rel| rel == "alternate"))
        .map(|link| link.attr("href").map(str::to_owned).unwrap_or_else(|| link.text()));
    let mut line = format!("- {title}");
    if let Some(date) = date.filter(|date| !date.is_empty()) {
        line.push_str(&format!(" ({date})"));
    }
    if let Some(link) = link.filter(|link| !link.is_empty()) {
        line.push_str(&format!(" {link}"));
    }
    if let Some(summary) = field(&["description", "summary", "encoded", "content"]).map(summary) {
        line.push_str(&format!("\n  {summary}"));
    }
    line
}

fn summary(element: &Element) -> String {
    let text = element.text();
    let inline = WebReading { main_content: false, links: false, ..WebReading::default() };
    let text = if text.contains('<') { super::distill::distill(&text, "", inline) } else { text };
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(SUMMARY) {
        Some((at, _)) => format!("{}…", &flat[..at]),
        None => flat,
    }
}
