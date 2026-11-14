use std::collections::HashSet;

use domain::web_reading::WebReading;

use super::links::Links;
use super::main_content::{DROPPED, FURNITURE, NEVER_TEXT, hidden, main_of};
use super::runs::{SHOWN_OF_A_RUN, link_run, repeated_labels};
use super::tree::{Element, Node, Syntax, parse};

pub fn distill(html: &str, url: &str, reading: WebReading) -> String {
    let root = parse(html, Syntax::Html);
    let body = root.first("body").unwrap_or(&root);
    let mut writer = Writer {
        out: String::new(),
        links: Links::new(url),
        repeated: if reading.cleanup { repeated_labels(body) } else { HashSet::new() },
        compact: false,
        addresses: true,
        reading,
    };
    let main = if reading.main_content { main_of(body) } else { None };
    let text = match main {
        Some(main) => {
            writer.element(main, None);
            let lead = tidy(&std::mem::take(&mut writer.out));
            writer.compact = reading.cleanup;
            writer.addresses = false;
            writer.element(body, Some(main));
            let rest = tidy(&writer.out);
            if rest.is_empty() { lead } else { format!("{lead}\n\n--- around the main content ---\n{rest}") }
        }
        None => {
            writer.element(body, None);
            tidy(&writer.out)
        }
    };
    let title = root.first("title").map(Element::text).filter(|title| !title.is_empty() && !text.starts_with("# "));
    let text = match title {
        Some(title) if text.is_empty() => format!("# {title}"),
        Some(title) => format!("# {title}\n\n{text}"),
        None => text,
    };
    let links = writer.links.list();
    if links.is_empty() { text } else { format!("{text}\n\n{links}") }
}

struct Writer {
    out: String,
    links: Links,
    repeated: HashSet<String>,
    compact: bool,
    addresses: bool,
    reading: WebReading,
}

impl Writer {
    fn element(&mut self, element: &Element, skip: Option<&Element>) {
        let name = element.name.as_str();
        let cleanup = self.reading.cleanup;
        let dropped = if cleanup { DROPPED.contains(&name) || hidden(element) } else { NEVER_TEXT.contains(&name) };
        if skip.is_some_and(|skip| std::ptr::eq(skip, element)) || dropped {
            return;
        }
        if self.compact
            && let Some(run) = link_run(element)
        {
            return self.run(&run);
        }
        match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = name[1..].parse().unwrap_or(1);
                self.out.push_str(&format!("\n\n{} {}\n\n", "#".repeat(level), element.text()));
            }
            "pre" => self.out.push_str(&format!("\n\n```\n{}\n```\n\n", verbatim(element).trim_matches('\n'))),
            "a" => self.link(element),
            "br" => self.out.push('\n'),
            "li" => self.wrapped(element, skip, "\n- ", "\n"),
            "td" | "th" => self.cell(element, skip),
            "tr" => self.wrapped(element, skip, "\n", ""),
            _ if FURNITURE.contains(&name) => {
                let outer = std::mem::replace(&mut self.compact, cleanup);
                self.wrapped(element, skip, "\n", "\n");
                self.compact = outer;
            }
            _ if block(name) => self.wrapped(element, skip, "\n", "\n"),
            _ => self.children(element, skip),
        }
    }

    fn wrapped(&mut self, element: &Element, skip: Option<&Element>, before: &str, after: &str) {
        self.out.push_str(before);
        self.children(element, skip);
        self.out.push_str(after);
    }

    fn cell(&mut self, cell: &Element, skip: Option<&Element>) {
        let start = self.out.len();
        self.children(cell, skip);
        if self.out[start..].trim().is_empty() {
            self.out.truncate(start);
        } else {
            self.out.push_str(" | ");
        }
    }

    fn children(&mut self, element: &Element, skip: Option<&Element>) {
        for child in &element.children {
            match child {
                Node::Text(text) => self.out.push_str(&text.replace(['\n', '\r', '\t'], " ")),
                Node::Element(inner) => self.element(inner, skip),
            }
        }
    }

    fn link(&mut self, link: &Element) {
        let text = label(link);
        let footnote = text.starts_with('[') && link.attr("href").is_some_and(|href| href.starts_with('#'));
        if text.is_empty() || (footnote && self.reading.cleanup) || self.repeated.contains(&text.to_lowercase()) {
            return;
        }
        let numbered = self.numbered(link, text);
        self.out.push_str(&numbered);
    }

    fn numbered(&mut self, link: &Element, text: String) -> String {
        let href = link.attr("href").filter(|_| self.addresses);
        if !self.reading.links {
            return match href.and_then(|href| self.links.absolute(href)) {
                Some(address) => format!("{text} [{address}]"),
                None => text,
            };
        }
        match href.and_then(|href| self.links.number(href)) {
            Some(number) => format!("{text} [{number}]"),
            None => text,
        }
    }

    fn run(&mut self, run: &[&Element]) {
        let shown: Vec<String> = run.iter().take(SHOWN_OF_A_RUN).map(|link| self.numbered(link, label(link))).collect();
        let more = run.len().saturating_sub(SHOWN_OF_A_RUN);
        self.out.push_str(&format!("\n{} (+{more} more links)\n", shown.join(" · ")));
    }
}

fn label(link: &Element) -> String {
    let text = link.text();
    if !text.is_empty() {
        return text;
    }
    let image = link.first("img").and_then(|image| image.attr("alt"));
    link.attr("aria-label").or(link.attr("title")).or(image).unwrap_or("").trim().to_owned()
}

fn block(name: &str) -> bool {
    matches!(
        name,
        "p" | "div" | "section" | "article" | "main" | "table" | "ul" | "ol" | "dl" | "dt" | "dd" | "blockquote"
    ) || matches!(name, "figure" | "figcaption" | "form" | "details" | "summary" | "hr" | "address" | "fieldset")
}

fn verbatim(element: &Element) -> String {
    let mut out = String::new();
    for child in &element.children {
        match child {
            Node::Text(text) => out.push_str(text),
            Node::Element(inner) if inner.name == "br" => out.push('\n'),
            Node::Element(inner) if !DROPPED.contains(&inner.name.as_str()) => out.push_str(&verbatim(inner)),
            Node::Element(_) => {}
        }
    }
    out
}

fn tidy(text: &str) -> String {
    let mut out = String::new();
    let mut blank = false;
    let mut code = false;
    for line in text.lines() {
        let fence = line.trim_start().starts_with("```");
        let line = if code || fence {
            line.trim_end().to_owned()
        } else {
            let words = line.split_whitespace().collect::<Vec<_>>().join(" ");
            words.trim_end_matches(" |").trim_end_matches('|').trim_end().to_owned()
        };
        if fence {
            code = !code;
        }
        if line.is_empty() && !code {
            blank = !out.is_empty();
            continue;
        }
        if out.ends_with("\n-\n") || out == "-\n" {
            out.truncate(out.len() - 1);
            out.push(' ');
            out.push_str(&line);
            out.push('\n');
            blank = false;
            continue;
        }
        let in_list =
            (line == "-" || line.starts_with("- ")) && out.lines().last().is_some_and(|last| last.starts_with("- "));
        if blank && !in_list {
            out.push('\n');
        }
        blank = false;
        out.push_str(&line);
        out.push('\n');
    }
    out.trim_end().to_owned()
}
