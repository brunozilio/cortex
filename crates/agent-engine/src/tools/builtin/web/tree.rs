use std::collections::HashMap;

use super::html::decode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    Html,
    Xml,
}

const VOID: &[&str] =
    &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];
const RAW: &[&str] = &["script", "style", "textarea", "title", "noscript"];
const CLOSED_BY_SELF: &[&str] = &["p", "li", "option", "tr", "td", "th", "dt", "dd"];

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }

    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|child| match child {
            Node::Element(element) => Some(element),
            Node::Text(_) => None,
        })
    }

    pub fn find(&self, matches: &dyn Fn(&Element) -> bool) -> Vec<&Element> {
        let mut found = Vec::new();
        for child in self.elements() {
            if matches(child) {
                found.push(child);
            }
            found.extend(child.find(matches));
        }
        found
    }

    pub fn first(&self, name: &str) -> Option<&Element> {
        self.find(&|element| element.name == name).into_iter().next()
    }

    pub fn text(&self) -> String {
        let mut out = String::new();
        self.gather(&mut out);
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn gather(&self, out: &mut String) {
        for child in &self.children {
            match child {
                Node::Text(text) => {
                    out.push_str(text);
                    out.push(' ');
                }
                Node::Element(element) if !matches!(element.name.as_str(), "script" | "style") => element.gather(out),
                Node::Element(_) => {}
            }
        }
    }
}

pub const DEEPEST: usize = 256;

struct Open {
    stack: Vec<Element>,
    named: HashMap<String, usize>,
}

impl Open {
    fn new() -> Self {
        Self { stack: vec![Element { name: "#document".into(), ..Element::default() }], named: HashMap::new() }
    }

    fn full(&self) -> bool {
        self.stack.len() > DEEPEST
    }

    fn push(&mut self, element: Element) {
        *self.named.entry(element.name.clone()).or_default() += 1;
        self.stack.push(element);
    }

    fn holds(&self, name: &str) -> bool {
        self.named.get(name).is_some_and(|count| *count > 0)
    }

    fn text(&mut self, text: &str) {
        if !text.is_empty() {
            self.node(Node::Text(decode(text)));
        }
    }

    fn node(&mut self, node: Node) {
        if let Some(top) = self.stack.last_mut() {
            top.children.push(node);
        }
    }

    fn fold(&mut self) {
        if self.stack.len() < 2 {
            return;
        }
        if let Some(done) = self.stack.pop() {
            if let Some(count) = self.named.get_mut(&done.name) {
                *count = count.saturating_sub(1);
            }
            self.node(Node::Element(done));
        }
    }

    fn fold_to(&mut self, at: usize) {
        while self.stack.len() > at.max(1) {
            self.fold();
        }
    }

    fn close(&mut self, name: &str) {
        if !self.holds(name) {
            return;
        }
        if let Some(at) = self.stack.iter().rposition(|open| open.name == name).filter(|at| *at > 0) {
            self.fold_to(at);
        }
    }

    fn close_sibling(&mut self, name: &str) {
        if !self.holds(name) {
            return;
        }
        let scope: &[&str] = match name {
            "li" => &["ul", "ol"],
            "td" | "th" | "tr" => &["table", "tbody", "thead"],
            "option" => &["select", "datalist"],
            _ => &["div", "section", "article", "main", "body", "td", "li", "blockquote"],
        };
        let open = self.stack.iter().rposition(|element| element.name == name);
        let fence = self.stack.iter().rposition(|element| scope.contains(&element.name.as_str()));
        if let Some(at) = open.filter(|at| fence.is_none_or(|fence| *at > fence)) {
            self.fold_to(at);
        }
    }

    fn finished(mut self) -> Element {
        self.fold_to(1);
        self.stack.pop().unwrap_or_default()
    }
}

fn closing_tag(rest: &str, name: &str) -> usize {
    let bytes = rest.as_bytes();
    let mut from = 0;
    while let Some(found) = rest[from..].find("</") {
        let at = from + found;
        let after = at + 2;
        if bytes.get(after..after + name.len()).is_some_and(|written| written.eq_ignore_ascii_case(name.as_bytes())) {
            return at;
        }
        from = after;
    }
    rest.len()
}

pub fn parse(source: &str, syntax: Syntax) -> Element {
    let mut open = Open::new();
    let mut rest = source;
    while !rest.is_empty() {
        let Some(at) = rest.find('<') else {
            open.text(rest);
            break;
        };
        open.text(&rest[..at]);
        rest = &rest[at..];
        if let Some(after) = rest.strip_prefix("<![CDATA[") {
            let end = after.find("]]>").unwrap_or(after.len());
            open.node(Node::Text(after[..end].to_owned()));
            rest = after.get(end + 3..).unwrap_or("");
            continue;
        }
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.find("-->").map_or("", |end| &after[end + 3..]);
            continue;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            rest = rest.find('>').map_or("", |end| &rest[end + 1..]);
            continue;
        }
        let Some(end) = tag_end(rest) else {
            open.text(rest);
            break;
        };
        let tag = &rest[1..end];
        rest = &rest[end + 1..];
        if let Some(closing) = tag.strip_prefix('/') {
            open.close(&name_of(closing, syntax));
            continue;
        }
        let Some(element) = opened(tag, syntax) else {
            open.text("<");
            continue;
        };
        let leaf = tag.ends_with('/') || (syntax == Syntax::Html && VOID.contains(&element.name.as_str()));
        if syntax == Syntax::Html && RAW.contains(&element.name.as_str()) {
            let end = closing_tag(rest, &element.name);
            let mut raw = element;
            if raw.name != "script" && raw.name != "style" {
                raw.children.push(Node::Text(decode(&rest[..end])));
            }
            open.node(Node::Element(raw));
            rest = &rest[end..];
            rest = rest.find('>').map_or("", |close| &rest[close + 1..]);
            continue;
        }
        if leaf {
            open.node(Node::Element(element));
            continue;
        }
        if syntax == Syntax::Html && CLOSED_BY_SELF.contains(&element.name.as_str()) {
            open.close_sibling(&element.name);
        }
        if !open.full() {
            open.push(element);
        }
    }
    open.finished()
}

fn tag_end(rest: &str) -> Option<usize> {
    let mut quote = None;
    for (at, c) in rest.char_indices().skip(1) {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), _) if c == open => quote = None,
            (None, '>') => return Some(at),
            _ => {}
        }
    }
    None
}

fn name_of(tag: &str, syntax: Syntax) -> String {
    let name: String = tag.chars().take_while(|c| !c.is_whitespace() && *c != '/' && *c != '>').collect();
    let name = if syntax == Syntax::Xml { name.rsplit(':').next().unwrap_or("").to_owned() } else { name };
    name.to_ascii_lowercase()
}

fn opened(tag: &str, syntax: Syntax) -> Option<Element> {
    let name = name_of(tag, syntax);
    if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let written = tag.find(|c: char| c.is_whitespace() || c == '/' || c == '>').unwrap_or(tag.len());
    Some(Element { attrs: super::attrs::parse(&tag[written..]), name, children: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unclosed_items_and_raw_text_still_make_a_tree() {
        let root = parse("<ul><li>one<li>two <b>bold</b></ul><script>if (a < b) {}</script><p>x &amp; y", Syntax::Html);
        let items = root.find(&|element| element.name == "li");
        assert_eq!(items.iter().map(|li| li.text()).collect::<Vec<_>>(), ["one", "two bold"]);
        assert_eq!(root.first("p").map(Element::text).as_deref(), Some("x & y"));
        assert!(root.text().contains("x & y") && !root.text().contains("if (a"));
    }

    fn on_a_small_stack<T: Send>(work: impl FnOnce() -> T + Send) -> T {
        std::thread::scope(|scope| {
            std::thread::Builder::new().stack_size(2 << 20).spawn_scoped(scope, work).expect("thread").join()
        })
        .expect("the work ran to its end without overflowing its stack")
    }

    #[test]
    fn a_page_nested_without_end_is_read_flat_past_a_depth_and_never_overflows_the_stack() {
        let deep = format!("{}deepest{}", "<b>".repeat(200_000), "</b>".repeat(200_000));
        for syntax in [Syntax::Html, Syntax::Xml] {
            let text = on_a_small_stack(|| {
                let root = parse(&deep, syntax);
                let found = root.find(&|element| element.name == "b").len();
                (root.text(), found)
            });
            assert_eq!(text.0, "deepest");
            assert!(text.1 > 0 && text.1 <= DEEPEST, "{}", text.1);
        }
        let read = on_a_small_stack(|| {
            super::super::distill::distill(&deep, "https://a.dev", domain::web_reading::WebReading::default())
        });
        assert!(read.contains("deepest"), "{read}");
    }

    #[test]
    fn five_mebibytes_of_tags_are_read_in_one_pass() {
        let five = 5usize << 20;
        let titles = "<title></title>".repeat(five / 15);
        let open = "<i>".repeat(15_000);
        let closes = format!("{open}{}", "</x>".repeat((five - open.len()) / 4));
        let raw = "<SCRIPT>a</SCRIPT><Style>b</sTyle>".repeat(five / 34);
        for page in [titles, closes, raw] {
            let started = std::time::Instant::now();
            on_a_small_stack(|| parse(&page, Syntax::Html));
            assert!(started.elapsed() < std::time::Duration::from_secs(5), "{:?}", started.elapsed());
        }
    }

    #[test]
    fn a_raw_element_ends_at_its_closing_tag_in_any_case() {
        let root = parse("<TITLE>Hi <b>there</b></TiTlE><p>after</p>", Syntax::Html);
        assert_eq!(root.first("title").map(Element::text).as_deref(), Some("Hi <b>there</b>"));
        assert_eq!(root.first("p").map(Element::text).as_deref(), Some("after"));
    }

    #[test]
    fn xml_keeps_link_text_and_cdata() {
        let root = parse(
            "<rss><item><link>https://a.dev/1</link><description><![CDATA[<p>Hi</p>]]></description></item></rss>",
            Syntax::Xml,
        );
        assert_eq!(root.first("link").map(Element::text).as_deref(), Some("https://a.dev/1"));
        assert_eq!(root.first("description").map(Element::text).as_deref(), Some("<p>Hi</p>"));
    }

    #[test]
    fn a_prefixed_tag_in_any_script_keeps_its_attributes_and_never_panics() {
        let root = parse("<日本:item kind=\"a\">x</日本:item><dc:creator id='7'>y</dc:creator>", Syntax::Xml);
        assert_eq!(root.first("item").and_then(|item| item.attr("kind")), Some("a"));
        assert_eq!(root.first("creator").and_then(|creator| creator.attr("id")), Some("7"));

        const PIECES: [&str; 14] = ["a", "日本", "é", ":", "x", "-", "/", " ", "=", "\"", "'", ">", "<", "ü"];
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        for _ in 0..5_000 {
            let mut name = String::new();
            for _ in 0..1 + seed % 8 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                name.push_str(PIECES[usize::try_from(seed % PIECES.len() as u64).unwrap_or(0)]);
            }
            let text = format!("<{name} k=\"v\">t</{name}><{name}/>");
            let _ = parse(&text, Syntax::Xml);
            let _ = parse(&text, Syntax::Html);
        }
    }
}
