use std::collections::HashSet;

use super::tree::{Element, Node};

pub const SHOWN_OF_A_RUN: usize = 8;
const SHORT: usize = 30;
const REPEATED: usize = 5;

pub fn repeated_labels(body: &Element) -> HashSet<String> {
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for link in body.find(&|element| element.name == "a") {
        let label = link.text().to_lowercase();
        if !label.is_empty() && label.len() <= SHORT {
            *counts.entry(label).or_default() += 1;
        }
    }
    counts.into_iter().filter(|(_, count)| *count >= REPEATED).map(|(label, _)| label).collect()
}

pub fn link_run(list: &Element) -> Option<Vec<&Element>> {
    let mut links = Vec::new();
    for child in &list.children {
        match child {
            Node::Text(text) if text.trim().is_empty() => {}
            Node::Text(_) => return None,
            Node::Element(item) => links.push(lone_link(item)?),
        }
    }
    (links.len() > SHOWN_OF_A_RUN).then_some(links)
}

fn lone_link(item: &Element) -> Option<&Element> {
    let link = if item.name == "a" {
        item
    } else {
        let [link] = item.find(&|element| element.name == "a")[..] else { return None };
        if link.text() != item.text() {
            return None;
        }
        link
    };
    (link.attr("href").is_some() && link.text().len() <= SHORT).then_some(link)
}
