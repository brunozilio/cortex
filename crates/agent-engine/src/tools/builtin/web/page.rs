use domain::web_reading::WebReading;

#[derive(Debug, PartialEq, Eq)]
pub struct Unreadable(pub String);

const THIN_TEXT: usize = 400;
const THIN_HTML: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Html,
    Json,
    Feed,
    Text,
}

pub fn read(body: &str, content_type: Option<&str>, url: &str, reading: WebReading) -> Result<String, Unreadable> {
    match kind(body, content_type, reading.formats) {
        Kind::Text => Ok(body.trim().to_owned()),
        Kind::Json => Ok(super::json::compact(body).unwrap_or_else(|| body.trim().to_owned())),
        Kind::Feed => Ok(super::feed::read(body)),
        Kind::Html => {
            let text = super::distill::distill(body, url, reading);
            let words = text.split("\n\nLinks").next().unwrap_or("").trim().len();
            let thin = words == 0 || (words < THIN_TEXT && body.len() > THIN_HTML);
            if thin && reading.empty_page_error {
                return Err(Unreadable(format!(
                    "{url} has no readable text in its HTML ({words} characters of text in {} of markup): it is \
                     probably built by JavaScript, or behind a consent wall, a login or a bot check.",
                    body.len()
                )));
            }
            Ok(text)
        }
    }
}

pub fn clipped(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let Some(split) = text.rfind("\n\nLinks") else { return crate::tools::clip(text, limit) };
    let (body, list) = (&text[..split], &text[split + 2..]);
    let mut budget = limit;
    loop {
        let cut = crate::tools::clip(body, budget);
        let kept = referenced(&cut, list);
        let overflow = (cut.chars().count() + kept.chars().count() + 2).saturating_sub(limit);
        if overflow == 0 || budget <= limit / 4 {
            return if kept.is_empty() { cut } else { format!("{cut}\n\n{kept}") };
        }
        budget = budget.saturating_sub(overflow).max(limit / 4);
    }
}

fn referenced(body: &str, list: &str) -> String {
    let highest = regex::Regex::new(r"\[(\d+)\]")
        .ok()
        .and_then(|pattern| pattern.captures_iter(body).filter_map(|found| found[1].parse::<usize>().ok()).max())
        .unwrap_or(0);
    if highest == 0 {
        return String::new();
    }
    list.lines().take(highest + 1).collect::<Vec<_>>().join("\n")
}

fn kind(body: &str, content_type: Option<&str>, formats: bool) -> Kind {
    let declared = content_type.unwrap_or("").to_ascii_lowercase();
    let start = body.trim_start();
    let json = declared.contains("json") || (declared.is_empty() && (start.starts_with('{') || start.starts_with('[')));
    if formats && json {
        return Kind::Json;
    }
    if formats && super::feed::is_feed(body) && !declared.contains("html") {
        return Kind::Feed;
    }
    if json {
        return Kind::Text;
    }
    if declared.contains("html") {
        return Kind::Html;
    }
    if !declared.is_empty() && (!formats || !declared.contains("xml")) {
        return Kind::Text;
    }
    let head = start.chars().take(1024).collect::<String>().to_ascii_lowercase();
    let marks = ["<!doctype html", "<html", "<head", "<body", "<div", "<p>", "<p ", "<br", "<a href", "<h1", "<title"];
    if marks.iter().any(|mark| head.contains(mark)) { Kind::Html } else { Kind::Text }
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "page_flag_tests.rs"]
mod flag_tests;
