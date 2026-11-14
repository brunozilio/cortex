use super::tree::{Element, Node};

const MIN_PAGE: usize = 3000;
const MIN_SHARE: f64 = 0.25;
const DESCEND_SHARE: f64 = 0.6;
const KEEP_SHARE: f64 = 0.5;
const MAX_DEPTH: usize = 12;

const COLUMNS: &[&str] = &["div", "section", "article", "main", "td", "center"];
pub const FURNITURE: &[&str] = &["nav", "header", "footer", "aside"];
pub const NEVER_TEXT: &[&str] = &["script", "style", "noscript", "svg", "head", "template"];
pub const DROPPED: &[&str] = &[
    "script", "style", "noscript", "svg", "head", "template", "iframe", "button", "select", "input", "textarea",
    "canvas", "object", "embed",
];

pub fn hidden(element: &Element) -> bool {
    element.attr("hidden").is_some()
        || element.attr("aria-hidden") == Some("true")
        || element.attr("style").is_some_and(|style| {
            let style: String = style.chars().filter(|c| !c.is_whitespace()).collect();
            style.to_ascii_lowercase().contains("display:none")
        })
}

pub fn main_of(body: &Element) -> Option<&Element> {
    let total = prose(body);
    if body.text().len() < MIN_PAGE {
        return None;
    }
    let landmarks: [&dyn Fn(&Element) -> bool; 3] =
        [&|element| element.name == "main", &|element| element.attr("role") == Some("main"), &|element| {
            element.name == "article"
        }];
    for landmark in landmarks {
        let found = body.find(landmark);
        if let [only] = found.as_slice()
            && prose(only) as f64 >= total as f64 * MIN_SHARE
        {
            return Some(densest(only, prose(only)).unwrap_or(only));
        }
    }
    densest(body, total)
}

fn densest(body: &Element, total: usize) -> Option<&Element> {
    let mut node = body;
    for _ in 0..MAX_DEPTH {
        let best = node
            .elements()
            .filter(|child| COLUMNS.contains(&child.name.as_str()))
            .map(|child| (prose(child), child))
            .max_by_key(|(len, _)| *len);
        let Some((len, child)) = best else { break };
        if (len as f64) < prose(node) as f64 * DESCEND_SHARE || (len as f64) < total as f64 * KEEP_SHARE {
            break;
        }
        node = child;
    }
    (!std::ptr::eq(node, body)).then_some(node)
}

pub fn prose(element: &Element) -> usize {
    let mut length = 0;
    for child in &element.children {
        match child {
            Node::Text(text) => length += text.split_whitespace().map(|word| word.len() + 1).sum::<usize>(),
            Node::Element(inner) if matches!(inner.name.as_str(), "a" | "select" | "button") => {}
            Node::Element(inner) if DROPPED.contains(&inner.name.as_str()) || hidden(inner) => {}
            Node::Element(inner) => length += prose(inner),
        }
    }
    length
}
