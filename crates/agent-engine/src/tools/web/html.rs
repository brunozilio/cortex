//! A web page's HTML read as plain text: scripts, styles and tags dropped,
//! blocks on their own lines, links kept as `text (url)`, entities decoded.

/// The page's text.
pub fn text(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        out.push_str(&decode(&rest[..at]));
        rest = &rest[at..];
        let Some(end) = rest.find('>') else { break };
        let tag = &rest[1..end];
        let name = tag.trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("");
        let name = name.to_ascii_lowercase();
        rest = &rest[end + 1..];
        if matches!(name.as_str(), "script" | "style" | "noscript" | "svg" | "head") && !tag.starts_with('/') {
            let close = format!("</{name}");
            match rest.to_ascii_lowercase().find(&close) {
                Some(at) => rest = &rest[at..],
                None => rest = "",
            }
            continue;
        }
        if name == "a"
            && !tag.starts_with('/')
            && let Some(href) = attribute(tag, "href").filter(|href| href.starts_with("http"))
        {
            out.push_str(&format!(" [{href}] "));
        }
        if !tag.starts_with('/')
            && matches!(
                name.as_str(),
                "p" | "div"
                    | "br"
                    | "li"
                    | "tr"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
                    | "pre"
                    | "section"
                    | "article"
                    | "header"
                    | "footer"
                    | "table"
                    | "ul"
                    | "ol"
                    | "blockquote"
            )
        {
            out.push('\n');
            if name.starts_with('h') && !tag.starts_with('/') {
                out.push_str("# ");
            }
            if name == "li" && !tag.starts_with('/') {
                out.push_str("- ");
            }
        }
    }
    out.push_str(&decode(rest));
    tidy(&out)
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let at = tag.find(&format!("{name}="))? + name.len() + 1;
    let value = &tag[at..];
    let quote = value.chars().next()?;
    if quote == '"' || quote == '\'' { value[1..].split(quote).next() } else { value.split(char::is_whitespace).next() }
}

/// The common entities, and numeric ones.
pub fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Runs of spaces as one, and no more than one blank line in a row.
fn tidy(text: &str) -> String {
    let lines: Vec<String> = text.lines().map(|line| line.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
    let mut out = String::new();
    let mut blank = false;
    for line in lines {
        if line.is_empty() {
            if !blank && !out.is_empty() {
                out.push('\n');
            }
            blank = true;
        } else {
            out.push_str(&line);
            out.push('\n');
            blank = false;
        }
    }
    out.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_reads_as_text_with_its_links() {
        let html = r#"<html><head><title>x</title><style>p{}</style></head><body>
            <h1>Install</h1><script>alert(1)</script>
            <p>Run <code>cargo add serde</code> &amp; see <a href="https://docs.rs/serde">the docs</a>.</p>
            <ul><li>one</li><li>two&nbsp;&#x2764;</li></ul></body></html>"#;
        assert_eq!(
            text(html),
            "# Install\n\nRun cargo add serde & see [https://docs.rs/serde] the docs.\n\n- one\n- two ❤"
        );
    }
}
