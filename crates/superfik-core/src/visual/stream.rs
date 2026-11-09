//! A visual still streaming in, drawn as far as it has arrived: the
//! document cut after its last whole tag, with the elements still open
//! closed, so each shape appears as soon as its tag is complete.

const UNSEEN: [&str; 5] = ["defs", "style", "title", "desc", "metadata"];

pub fn drawn_so_far(svg: &str) -> Option<String> {
    let start = svg.find("<svg")?;
    let doc = &svg[start..];
    let mut open: Vec<&str> = Vec::new();
    let mut cut = 0;
    let mut drawn = false;
    let mut at = 0;
    while let Some(lt) = doc[at..].find('<').map(|i| at + i) {
        let rest = &doc[lt..];
        let end = if rest.starts_with("<!--") {
            rest.find("-->").map(|i| lt + i + 3)
        } else if rest.starts_with("<![CDATA[") {
            rest.find("]]>").map(|i| lt + i + 3)
        } else if rest.starts_with("<!") || rest.starts_with("<?") {
            rest.find('>').map(|i| lt + i + 1)
        } else {
            let Some(gt) = tag_end(rest) else { break };
            let tag = &rest[1..gt];
            let name = tag.trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/').next()?;
            let hidden = UNSEEN.contains(&name) || open.iter().any(|o| UNSEEN.contains(o));
            if tag.starts_with('/') {
                if let Some(depth) = open.iter().rposition(|o| *o == name) {
                    open.truncate(depth);
                }
                drawn |= !hidden && !open.is_empty();
            } else if tag.ends_with('/') {
                drawn |= !hidden && !open.is_empty();
            } else {
                open.push(name);
            }
            Some(lt + gt + 1)
        };
        let Some(end) = end else { break };
        cut = end;
        at = end;
    }
    if !drawn {
        return None;
    }
    let mut out = doc[..cut].to_owned();
    for name in open.iter().rev() {
        out.push_str(&format!("</{name}>"));
    }
    Some(out)
}

fn tag_end(tag: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in tag.char_indices().skip(1) {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOW: &str = r#"<svg viewBox="0 0 440 200"><defs><marker id="arrow"><path d="M0 0L10 5"/></marker></defs><g class="node c-purple"><rect x="20" y="20" width="160" height="44"/><text class="th" x="100" y="42">Start</text></g><line x1="100" y1="64" x2="100" y2="110"/></svg>"#;

    #[test]
    fn nothing_is_drawn_until_a_visible_element_is_whole() {
        for end in [
            "",
            "<svg viewBox",
            r#"<svg viewBox="0 0 440 200">"#,
            r#"<svg viewBox="0 0 440 200"><defs><marker id="arrow"><path d="M0 0L10 5"/>"#,
        ] {
            assert_eq!(drawn_so_far(&FLOW[..end.len()]), None, "{end}");
        }
    }

    #[test]
    fn whole_tags_are_kept_and_open_elements_closed() {
        let rect = FLOW.find("<text").expect("text");
        assert_eq!(
            drawn_so_far(&FLOW[..rect + 10]).as_deref(),
            Some(&*format!("{}</g></svg>", &FLOW[..rect])),
            "a tag still arriving is left out"
        );
        let label = FLOW.find("Start").expect("label") + 3;
        let partial = drawn_so_far(&FLOW[..label]).expect("drawn");
        assert!(partial.ends_with(r#"y="42"></text></g></svg>"#), "{partial}");
    }

    #[test]
    fn quoted_angle_brackets_do_not_end_a_tag() {
        let svg = r#"<svg viewBox="0 0 10 10"><rect onclick="sendPrompt('a > b')" width="4" height="4"/><circle r="#;
        let drawn = drawn_so_far(svg).expect("drawn");
        assert!(drawn.ends_with(r#"height="4"/></svg>"#), "{drawn}");
    }

    #[test]
    fn a_whole_document_comes_back_as_it_was() {
        assert_eq!(drawn_so_far(FLOW).as_deref(), Some(FLOW));
    }
}
