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

pub fn parse(source: &str, syntax: Syntax) -> Element {
    let mut stack = vec![Element { name: "#document".into(), ..Element::default() }];
    let mut rest = source;
    while !rest.is_empty() {
        let Some(at) = rest.find('<') else {
            push_text(&mut stack, rest);
            break;
        };
        push_text(&mut stack, &rest[..at]);
        rest = &rest[at..];
        if let Some(after) = rest.strip_prefix("<![CDATA[") {
            let end = after.find("]]>").unwrap_or(after.len());
            push_node(&mut stack, Node::Text(after[..end].to_owned()));
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
            push_text(&mut stack, rest);
            break;
        };
        let tag = &rest[1..end];
        rest = &rest[end + 1..];
        if let Some(closing) = tag.strip_prefix('/') {
            close(&mut stack, &name_of(closing, syntax));
            continue;
        }
        let Some(element) = open(tag, syntax) else {
            push_text(&mut stack, "<");
            continue;
        };
        let leaf = tag.ends_with('/') || (syntax == Syntax::Html && VOID.contains(&element.name.as_str()));
        if syntax == Syntax::Html && RAW.contains(&element.name.as_str()) {
            let lower = rest.to_ascii_lowercase();
            let end = lower.find(&format!("</{}", element.name)).unwrap_or(rest.len());
            let mut raw = element;
            if raw.name != "script" && raw.name != "style" {
                raw.children.push(Node::Text(decode(&rest[..end])));
            }
            push_node(&mut stack, Node::Element(raw));
            rest = &rest[end..];
            rest = rest.find('>').map_or("", |close| &rest[close + 1..]);
            continue;
        }
        if leaf {
            push_node(&mut stack, Node::Element(element));
            continue;
        }
        if syntax == Syntax::Html && CLOSED_BY_SELF.contains(&element.name.as_str()) {
            close_sibling(&mut stack, &element.name);
        }
        stack.push(element);
    }
    while stack.len() > 1 {
        fold(&mut stack);
    }
    stack.pop().unwrap_or_default()
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

fn open(tag: &str, syntax: Syntax) -> Option<Element> {
    let name = name_of(tag, syntax);
    if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    Some(Element { attrs: super::attrs::parse(&tag[name.len()..]), name, children: Vec::new() })
}

fn push_text(stack: &mut [Element], text: &str) {
    if !text.is_empty() {
        push_node(stack, Node::Text(decode(text)));
    }
}

fn push_node(stack: &mut [Element], node: Node) {
    if let Some(top) = stack.last_mut() {
        top.children.push(node);
    }
}

fn fold(stack: &mut Vec<Element>) {
    if let Some(done) = stack.pop() {
        push_node(stack, Node::Element(done));
    }
}

fn close(stack: &mut Vec<Element>, name: &str) {
    if let Some(at) = stack.iter().rposition(|open| open.name == name).filter(|at| *at > 0) {
        while stack.len() > at {
            fold(stack);
        }
    }
}

fn close_sibling(stack: &mut Vec<Element>, name: &str) {
    let scope: &[&str] = match name {
        "li" => &["ul", "ol"],
        "td" | "th" | "tr" => &["table", "tbody", "thead"],
        "option" => &["select", "datalist"],
        _ => &["div", "section", "article", "main", "body", "td", "li", "blockquote"],
    };
    let open = stack.iter().rposition(|element| element.name == name);
    let fence = stack.iter().rposition(|element| scope.contains(&element.name.as_str()));
    if let Some(at) = open.filter(|at| fence.is_none_or(|fence| *at > fence)) {
        while stack.len() > at {
            fold(stack);
        }
    }
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

    #[test]
    fn xml_keeps_link_text_and_cdata() {
        let root = parse(
            "<rss><item><link>https://a.dev/1</link><description><![CDATA[<p>Hi</p>]]></description></item></rss>",
            Syntax::Xml,
        );
        assert_eq!(root.first("link").map(Element::text).as_deref(), Some("https://a.dev/1"));
        assert_eq!(root.first("description").map(Element::text).as_deref(), Some("<p>Hi</p>"));
    }
}
